//! Transactional domain writes and direct derived-data publication.
use super::*;
use crate::{
    activity_parser::ParsedActivityData,
    entities::{activity_imports, strava_activity_states, strava_events},
};
use sea_orm::{ActiveModelTrait, EntityTrait, Set, TransactionTrait};

#[cfg(test)]
mod strava_native_unit_deletion;

impl StravaOperations {
    pub async fn persist_activity(
        &self,
        input: NormalizedActivity,
    ) -> Result<ActivityChange, WorkflowError> {
        self.connection(&input.event).await?;
        if let Some(change) = strava_events::Model::stored_mutation(&self.db, &input.event).await? {
            return Ok(change);
        }
        let bytes = self.store.read(&input.source).await?;
        let summary: super::super::provider_payload::StoredStravaProviderPayload =
            serde_json::from_slice(&bytes)
                .map_err(|error| WorkflowError::bad_request(error.to_string()))?;
        if !super::super::provider_payload::strava_activity_is_bike(&summary.activity) {
            return self
                .delete_activity(self.capture_deletion(input.event).await?)
                .await;
        }
        let parsed_ref = input
            .parsed
            .as_ref()
            .ok_or_else(|| WorkflowError::bad_request("Cycling activity has no normalized data"))?;
        let parsed: ParsedActivityData = self.store.read_json(parsed_ref).await?;
        let profile =
            crate::training_profile::load_training_profile(&self.db, input.event.bike_user_id)
                .await?;
        self.persist_bike_activity(input, parsed, summary, profile)
            .await
    }

    async fn persist_bike_activity(
        &self,
        input: NormalizedActivity,
        parsed: ParsedActivityData,
        summary: super::super::provider_payload::StoredStravaProviderPayload,
        profile: crate::training_profile::TrainingProfile,
    ) -> Result<ActivityChange, WorkflowError> {
        let txn = self.db.begin().await?;
        strava_connections::Model::lock_event(&txn, &input.event).await?;
        if strava_activity_states::Model::supersedes(&txn, &input.event, false).await? {
            let change = ActivityChange::unchanged(input.event, ActivityStatus::Superseded);
            strava_events::Model::record_mutation(&txn, &change).await?;
            txn.commit().await?;
            return Ok(change);
        }
        let existing = activities::Model::strava_activity(
            &txn,
            input.event.bike_user_id,
            input.event.activity_id()?,
        )
        .await?;
        if let Some(activity) = &existing {
            if strava_activity_states::Model::matches_content(
                &txn,
                &input.event,
                &input.source.sha256,
            )
            .await?
            {
                let change = Self::persist_unchanged(&txn, &input, activity).await?;
                txn.commit().await?;
                return Ok(change);
            }
        }
        let upload = super::super::build_activity_upload(&summary.activity, &summary.streams)?;
        if let Some(change) = strava_events::Model::stored_mutation(&txn, &input.event).await? {
            return Ok(change);
        }
        let import = activity_imports::Entity::native_strava_source(
            &txn,
            &input.event,
            existing
                .as_ref()
                .and_then(|activity| activity.activity_import_id),
            &input.source,
            &upload,
        )
        .await?;
        let saved =
            activities::Model::save_parsed_strava(&txn, &input.event, &import, &parsed, &profile)
                .await?;
        strava_activity_states::Model::apply_event(
            &txn,
            &input.event,
            Some(input.source.sha256),
            false,
        )
        .await?;
        let dirty_from =
            existing
                .as_ref()
                .map_or(parsed.draft.started_at.date_naive(), |activity| {
                    activity
                        .started_at
                        .date_naive()
                        .min(parsed.draft.started_at.date_naive())
                });
        crate::analytics::mark_user_fitness_dirty(
            &txn,
            input.event.bike_user_id,
            dirty_from,
            chrono::Utc::now(),
        )
        .await?;
        let change = ActivityChange::applied(input.event, saved.id, import.id, dirty_from);
        strava_events::Model::record_mutation(&txn, &change).await?;
        txn.commit().await?;
        Ok(change)
    }

    async fn persist_unchanged(
        db: &impl sea_orm::ConnectionTrait,
        input: &NormalizedActivity,
        activity: &activities::Model,
    ) -> Result<ActivityChange, WorkflowError> {
        let mut change = ActivityChange::unchanged(input.event.clone(), ActivityStatus::Unchanged);
        change.event.bike_activity_id = Some(activity.id);
        change.bike_activity_id = Some(activity.id);
        change.import_id = activity.activity_import_id;
        change.dirty_from = Some(activity.started_at.date_naive());
        strava_activity_states::Model::apply_event(
            db,
            &input.event,
            Some(input.source.sha256.clone()),
            false,
        )
        .await?;
        strava_events::Model::record_mutation(db, &change).await?;
        Ok(change)
    }

    pub async fn capture_deletion(
        &self,
        event: StravaEvent,
    ) -> Result<ActivityChange, WorkflowError> {
        self.connection(&event).await?;
        let Some(activity) =
            activities::Model::strava_activity(&self.db, event.bike_user_id, event.activity_id()?)
                .await?
        else {
            return Ok(ActivityChange::unchanged(
                event,
                ActivityStatus::AlreadyAbsent,
            ));
        };
        let segment_ids =
            crate::activity_lifecycle::load_segment_ids_for_activity(&self.db, activity.id).await?;
        let mut event = event;
        event.bike_activity_id = Some(activity.id);
        Ok(ActivityChange {
            event,
            status: ActivityStatus::Captured,
            bike_activity_id: Some(activity.id),
            import_id: activity.activity_import_id,
            segment_ids,
            dirty_from: Some(activity.started_at.date_naive()),
            removed_source: None,
        })
    }

    pub async fn delete_activity(
        &self,
        change: ActivityChange,
    ) -> Result<ActivityChange, WorkflowError> {
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
        txn: &impl sea_orm::ConnectionTrait,
        mut change: ActivityChange,
    ) -> Result<ActivityChange, WorkflowError> {
        let connection = strava_connections::Model::lock_event(txn, &change.event).await?;
        if let Some(saved) = strava_events::Model::stored_mutation(txn, &change.event).await? {
            return Ok(saved);
        }
        if Self::deletion_superseded(txn, &change.event, &connection).await? {
            let skipped = ActivityChange::unchanged(change.event, ActivityStatus::Superseded);
            strava_events::Model::record_mutation(txn, &skipped).await?;
            return Ok(skipped);
        }
        Self::delete_owned_activity(txn, &mut change).await?;
        Self::record_deletion(txn, &change).await?;
        Ok(change)
    }

    async fn deletion_superseded(
        db: &impl sea_orm::ConnectionTrait,
        event: &StravaEvent,
        connection: &strava_connections::Model,
    ) -> Result<bool, WorkflowError> {
        let disconnect_cleanup = event.parent_operation
            == Some(super::super::events::StravaOperation::Disconnect)
            && connection.disconnect_requested_at.is_some();
        if disconnect_cleanup {
            return Ok(false);
        }
        Ok(strava_activity_states::Model::supersedes(db, event, true).await?)
    }

    async fn delete_owned_activity(
        db: &impl sea_orm::ConnectionTrait,
        change: &mut ActivityChange,
    ) -> Result<(), WorkflowError> {
        let Some(activity) = activities::Model::strava_activity(
            db,
            change.event.bike_user_id,
            change.event.activity_id()?,
        )
        .await?
        else {
            change.status = ActivityStatus::AlreadyAbsent;
            return Ok(());
        };
        let started_on = activity.started_at.date_naive();
        change.dirty_from = Some(
            change
                .dirty_from
                .map_or(started_on, |day| day.min(started_on)),
        );
        change.bike_activity_id = Some(activity.id);
        change.event.bike_activity_id = Some(activity.id);
        let deleted = crate::activity_lifecycle::delete_activity_records(
            db,
            change.event.bike_user_id,
            activity,
        )
        .await?;
        change.segment_ids = deleted.segment_ids;
        change.removed_source = deleted.source;
        change.status = ActivityStatus::Deleted;
        Ok(())
    }

    async fn record_deletion(
        db: &impl sea_orm::ConnectionTrait,
        change: &ActivityChange,
    ) -> Result<(), WorkflowError> {
        strava_activity_states::Model::apply_event(db, &change.event, None, true).await?;
        if let Some(day) = change.dirty_from {
            crate::analytics::mark_user_fitness_dirty(
                db,
                change.event.bike_user_id,
                day,
                chrono::Utc::now(),
            )
            .await?;
        }
        strava_events::Model::record_mutation(db, change).await?;
        Ok(())
    }

    pub async fn delete_source(
        &self,
        change: ActivityChange,
    ) -> Result<ActivityChange, WorkflowError> {
        if let Some(source) = &change.removed_source {
            self.store.remove(source).await?;
        }
        Ok(change)
    }

    pub async fn rebuild_activity(
        &self,
        mut change: ActivityChange,
    ) -> Result<ActivityChange, WorkflowError> {
        let txn = self.db.begin().await?;
        strava_connections::Model::lock_event(&txn, &change.event).await?;
        if strava_activity_states::Model::supersedes(&txn, &change.event, false).await? {
            return Ok(change);
        }
        if let Some(id) = change.bike_activity_id {
            if let Some(activity) = activities::Entity::find_by_id(id).one(&txn).await? {
                let derived = crate::activity_data::deserialize_derived_activity_data(
                    activity.derived_data_json.as_ref(),
                );
                change.segment_ids = crate::activity_lifecycle::refresh_activity_derived_state_without_cache_rebuilds(&txn, change.event.bike_user_id, id, &derived.route_points).await?;
                crate::analytics::mark_segment_activity_changes(
                    &txn,
                    &change.segment_ids,
                    chrono::Utc::now(),
                )
                .await?;
                crate::analytics::rebuild_activity_analytics_cache(&txn, &[id]).await?;
                crate::activity_training_analysis::rebuild_activity_training_analysis_cache(
                    &txn,
                    &[id],
                )
                .await?;
            }
        }
        txn.commit().await?;
        Ok(change)
    }

    pub async fn rebuild_fitness(
        &self,
        change: ActivityChange,
    ) -> Result<ActivityChange, WorkflowError> {
        if change.dirty_from.is_some() {
            crate::analytics::rebuild_fitness_freshness_cache(&self.db, change.event.bike_user_id)
                .await?;
        }
        Ok(change)
    }

    pub async fn rebuild_segments(
        &self,
        change: ActivityChange,
    ) -> Result<ActivityChange, WorkflowError> {
        crate::analytics::rebuild_segment_analytics_cache(&self.db, &change.segment_ids).await?;
        Ok(change)
    }

    pub async fn publish_heatmap(
        &self,
        change: ActivityChange,
    ) -> Result<ActivityChange, WorkflowError> {
        if let Some(id) = change.bike_activity_id {
            if let Some(projection) = crate::entities::heatmap_projections::Entity::find_by_id(id)
                .one(&self.db)
                .await?
            {
                crate::heatmaps::preparation::prepare_activity(
                    &self.db,
                    crate::heatmaps::projection::PendingProjection {
                        activity_id: id,
                        generation: projection.generation,
                    },
                )
                .await?;
            }
        }
        Ok(change)
    }

    pub async fn complete_activity(
        &self,
        change: ActivityChange,
    ) -> Result<ActivityChange, WorkflowError> {
        if let Some(id) = change.import_id {
            if let Some(import) =
                activity_imports::Entity::find_owned(&self.db, change.event.bike_user_id, id)
                    .await?
            {
                let mut active: activity_imports::ActiveModel = import.into();
                active.status =
                    Set(crate::activity_import_lifecycle::ACTIVITY_IMPORT_STATUS_PROCESSED.into());
                active.processing_stage =
                    Set(crate::activity_import_lifecycle::ACTIVITY_IMPORT_STAGE_COMPLETE.into());
                active.processed_at = Set(Some(chrono::Utc::now()));
                active.processing_error = Set(None);
                active.update(&self.db).await?;
            }
        }
        Ok(change)
    }
}
