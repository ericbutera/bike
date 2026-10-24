//! Provider deletion and replay adapters; ingestion belongs to the shared activity pipeline.
use super::*;
use bike_core::entities::{activity_event_history, activity_imports, strava_activity_states, strava_events};
use sea_orm::{DatabaseTransaction, TransactionTrait};

#[cfg(test)]
mod tests;

impl StravaOperations {
    /// Old histories resume through the same parser/save operation as new imports.
    pub async fn persist_activity(&self, input: NormalizedActivity) -> Result<ActivityChange, WorkflowError> {
        bike_core::activity_reprocessing::process_strava_source(
            &self.db,
            &self.objects,
            RetainedActivity {
                event: input.event,
                source: input.source,
            },
        )
        .await
    }

    async fn resolve_source_conflict(
        db: &DatabaseTransaction,
        event: &StravaEvent,
    ) -> Result<Option<ActivityChange>, WorkflowError> {
        if let Some(change) = strava_events::Model::stored_mutation(db, event).await? {
            return Ok(Some(change));
        }
        if !strava_activity_states::Model::matches_version(db, event).await? {
            return Ok(Some(ActivityChange::unchanged(
                event.clone(),
                ActivityStatus::SourceChanged,
            )));
        }
        Ok(None)
    }

    pub async fn capture_deletion(&self, mut event: StravaEvent) -> Result<ActivityChange, WorkflowError> {
        self.connection(&event).await?;
        if event.source_version.is_some() {
            event.source_version = Some(strava_activity_states::Model::capture_version(&self.db, &event).await?);
        }
        let Some(activity) =
            activities::Model::strava_activity(&self.db, event.bike_user_id, event.activity_id()?).await?
        else {
            return Ok(ActivityChange::unchanged(event, ActivityStatus::AlreadyAbsent));
        };
        let segment_ids = bike_core::activity_lifecycle::load_segment_ids_for_activity(&self.db, activity.id).await?;
        let mut event = event;
        event.bike_activity_id = Some(activity.id);
        Ok(ActivityChange {
            event,
            status: ActivityStatus::Captured,
            bike_activity_id: Some(activity.id),
            import_id: activity.activity_import_id,
            segment_ids,
            dirty_from: activity.is_bike_activity().then_some(activity.started_at.date_naive()),
            removed_source: None,
        })
    }

    pub async fn delete_activity(&self, change: ActivityChange) -> Result<ActivityChange, WorkflowError> {
        if let Some(saved) = strava_events::Model::stored_mutation(&self.db, &change.event).await? {
            return Ok(saved);
        }
        let txn = self.db.begin().await?;
        let change = self.delete_in_transaction(&txn, change).await?;
        txn.commit().await?;
        Ok(change)
    }

    async fn delete_in_transaction(
        &self,
        txn: &DatabaseTransaction,
        mut change: ActivityChange,
    ) -> Result<ActivityChange, WorkflowError> {
        let connection = strava_connections::Model::lock_event(txn, &change.event).await?;
        if let Some(saved) = Self::resolve_source_conflict(txn, &change.event).await? {
            return Ok(saved);
        }
        if Self::deletion_superseded(txn, &change.event, &connection).await? {
            let skipped = ActivityChange::unchanged(change.event, ActivityStatus::Superseded);
            strava_events::Model::record_mutation(txn, &skipped).await?;
            return Ok(skipped);
        }
        let previous = Self::delete_owned_activity(txn, &mut change).await?;
        Self::record_deletion(txn, &change, previous.as_ref()).await?;
        Ok(change)
    }

    async fn deletion_superseded(
        db: &impl sea_orm::ConnectionTrait,
        event: &StravaEvent,
        connection: &strava_connections::Model,
    ) -> Result<bool, WorkflowError> {
        let disconnect_cleanup = event.parent_operation == Some(super::super::events::StravaOperation::Disconnect)
            && connection.disconnect_requested_at.is_some();
        if disconnect_cleanup {
            return Ok(false);
        }
        Ok(strava_activity_states::Model::supersedes(db, event, true).await?)
    }

    async fn delete_owned_activity(
        db: &impl sea_orm::ConnectionTrait,
        change: &mut ActivityChange,
    ) -> Result<Option<activities::Model>, WorkflowError> {
        let Some(activity) =
            activities::Model::lock_strava_activity(db, change.event.bike_user_id, change.event.activity_id()?).await?
        else {
            change.status = ActivityStatus::AlreadyAbsent;
            return Ok(None);
        };
        let started_on = activity.started_at.date_naive();
        if activity.is_bike_activity() {
            change.dirty_from = Some(change.dirty_from.map_or(started_on, |day| day.min(started_on)));
        }
        change.bike_activity_id = Some(activity.id);
        change.event.bike_activity_id = Some(activity.id);
        let deleted =
            bike_core::activity_lifecycle::delete_activity_records(db, change.event.bike_user_id, activity.clone())
                .await?;
        change.segment_ids = deleted.segment_ids;
        change.removed_source = deleted.source;
        change.status = ActivityStatus::Deleted;
        Ok(Some(activity))
    }

    async fn record_deletion(
        db: &DatabaseTransaction,
        change: &ActivityChange,
        previous: Option<&activities::Model>,
    ) -> Result<(), WorkflowError> {
        let state = strava_activity_states::Model::apply_event(db, &change.event, None, true).await?;
        if let Some(day) = change.dirty_from {
            bike_core::analytics::mark_user_fitness_dirty(db, change.event.bike_user_id, day, chrono::Utc::now())
                .await?;
        }
        activity_event_history::Model::record_strava(db, change, &state, previous, None).await?;
        strava_events::Model::record_mutation(db, change).await?;
        Ok(())
    }

    pub async fn delete_source(&self, change: ActivityChange) -> Result<ActivityChange, WorkflowError> {
        if let Some(source) = &change.removed_source {
            self.store.remove(source).await?;
        }
        Ok(change)
    }

    pub async fn rebuild_activity(&self, mut change: ActivityChange) -> Result<ActivityChange, WorkflowError> {
        let txn = self.db.begin().await?;
        strava_connections::Model::lock_event(&txn, &change.event).await?;
        if strava_activity_states::Model::supersedes(&txn, &change.event, false).await? {
            return Ok(change);
        }
        if let Some(id) = change.bike_activity_id {
            if let Some(activity) = activities::Model::lock_owned(&txn, id, change.event.bike_user_id).await? {
                let derived =
                    bike_core::activity_data::deserialize_derived_activity_data(activity.derived_data_json.as_ref());
                let affected_segments =
                    bike_core::activity_lifecycle::refresh_activity_derived_state_without_cache_rebuilds(
                        &txn,
                        change.event.bike_user_id,
                        id,
                        &derived.route_points,
                    )
                    .await?;
                change.include_segments(affected_segments);
                bike_core::analytics::mark_segment_activity_changes(&txn, &change.segment_ids, chrono::Utc::now())
                    .await?;
                bike_core::analytics::rebuild_activity_analytics_cache(&txn, &[id]).await?;
                bike_core::entities::activity_training_analyses::Model::clear_for_non_cycling(&txn, &activity).await?;
                if activity.is_bike_activity() {
                    bike_core::activity_training_analysis::rebuild_activity_training_analysis_cache(&txn, &[id])
                        .await?;
                }
            }
        }
        txn.commit().await?;
        Ok(change)
    }

    pub async fn rebuild_fitness(&self, change: ActivityChange) -> Result<ActivityChange, WorkflowError> {
        if change.dirty_from.is_some() {
            bike_core::analytics::rebuild_fitness_freshness_cache(&self.db, change.event.bike_user_id).await?;
        }
        Ok(change)
    }

    pub async fn rebuild_segments(&self, change: ActivityChange) -> Result<ActivityChange, WorkflowError> {
        bike_core::analytics::rebuild_segment_analytics_cache(&self.db, &change.segment_ids).await?;
        if let Some(id) = change.bike_activity_id {
            if let Some(activity) = activities::Model::find_owned(&self.db, id, change.event.bike_user_id).await? {
                bike_core::activity_achievements::record(&self.db, &activity).await?;
                bike_core::analytics::rebuild_activity_analytics_cache(&self.db, &[id]).await?;
            }
        }
        Ok(change)
    }

    pub async fn publish_heatmap(&self, change: ActivityChange) -> Result<ActivityChange, WorkflowError> {
        if let Some(id) = change.bike_activity_id {
            if let Some(projection) =
                bike_core::entities::heatmap_projections::Model::for_activity(&self.db, id).await?
            {
                bike_core::heatmaps::preparation::prepare_activity(
                    &self.db,
                    bike_core::heatmaps::projection::PendingProjection {
                        activity_id: id,
                        generation: projection.generation,
                    },
                )
                .await?;
            }
        }
        Ok(change)
    }

    pub async fn complete_activity(&self, change: ActivityChange) -> Result<ActivityChange, WorkflowError> {
        if let Some(id) = change.import_id {
            if let Some(import) = activity_imports::Entity::find_owned(&self.db, change.event.bike_user_id, id).await? {
                import.mark_native_processed(&self.db).await?;
            }
        }
        Ok(change)
    }
}
