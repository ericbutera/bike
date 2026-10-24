//! Transactional domain writes and direct derived-data publication.
use super::*;
use bike_core::{
    activity_parser::ParsedActivityData,
    entities::{activity_event_history, activity_imports, strava_activity_states, strava_events},
};
use sea_orm::{DatabaseTransaction, TransactionTrait};

#[cfg(test)]
mod tests;

impl StravaOperations {
    pub async fn persist_activity(&self, input: NormalizedActivity) -> Result<ActivityChange, WorkflowError> {
        self.connection(&input.event).await?;
        if let Some(change) = strava_events::Model::stored_mutation(&self.db, &input.event).await? {
            return Ok(change);
        }
        let bytes = self.store.read(&input.source).await?;
        let summary: super::super::provider_payload::StoredStravaProviderPayload =
            serde_json::from_slice(&bytes).map_err(|error| WorkflowError::bad_request(error.to_string()))?;
        let parsed_ref = input
            .parsed
            .as_ref()
            .ok_or_else(|| WorkflowError::bad_request("Strava activity has no normalized data"))?;
        let parsed: ParsedActivityData = self.store.read_json(parsed_ref).await?;
        let profile = bike_core::training_profile::load_training_profile(&self.db, input.event.bike_user_id).await?;
        self.persist_summary(input, parsed, summary, profile).await
    }

    async fn persist_summary(
        &self,
        input: NormalizedActivity,
        parsed: ParsedActivityData,
        summary: super::super::provider_payload::StoredStravaProviderPayload,
        profile: bike_core::training_profile::TrainingProfile,
    ) -> Result<ActivityChange, WorkflowError> {
        let txn = self.db.begin().await?;
        strava_connections::Model::lock_event(&txn, &input.event).await?;
        if let Some(change) = Self::resolve_source_conflict(&txn, &input.event).await? {
            txn.commit().await?;
            return Ok(change);
        }
        if strava_activity_states::Model::supersedes(&txn, &input.event, false).await? {
            let change = ActivityChange::unchanged(input.event, ActivityStatus::Superseded);
            strava_events::Model::record_mutation(&txn, &change).await?;
            txn.commit().await?;
            return Ok(change);
        }
        let existing =
            activities::Model::lock_strava_activity(&txn, input.event.bike_user_id, input.event.activity_id()?).await?;
        if let Some(change) = Self::persist_matching_content(&txn, &input, existing.as_ref()).await? {
            txn.commit().await?;
            return Ok(change);
        }
        let previous_segment_ids = match &existing {
            Some(activity) => bike_core::activity_lifecycle::load_segment_ids_for_activity(&txn, activity.id).await?,
            None => vec![],
        };
        let (saved, import) = self
            .save_activity_recording(
                &txn,
                &input,
                existing.as_ref().and_then(|activity| activity.activity_import_id),
                &parsed,
                &profile,
                &summary,
            )
            .await?;
        let mut change =
            ActivityChange::applied(input.event, saved.id, import.id, parsed.draft.started_at.date_naive());
        change.segment_ids = previous_segment_ids;
        let state =
            strava_activity_states::Model::apply_event(&txn, &change.event, Some(input.source.sha256), false).await?;
        change.dirty_from = Self::mark_fitness_change(&txn, &change.event, &parsed.draft, existing.as_ref()).await?;
        activity_event_history::Model::record_strava(&txn, &change, &state, existing.as_ref(), Some(&saved)).await?;
        strava_events::Model::record_mutation(&txn, &change).await?;
        txn.commit().await?;
        Ok(change)
    }

    async fn save_activity_recording(
        &self,
        txn: &DatabaseTransaction,
        input: &NormalizedActivity,
        import_id: Option<i32>,
        parsed: &ParsedActivityData,
        profile: &bike_core::training_profile::TrainingProfile,
        summary: &super::super::provider_payload::StoredStravaProviderPayload,
    ) -> Result<(activities::Model, activity_imports::Model), WorkflowError> {
        let upload = super::super::build_activity_upload(&summary.activity, &summary.streams)?;
        let import =
            activity_imports::Entity::native_strava_source(txn, &input.event, import_id, &input.source, &upload)
                .await?;
        let recording = self
            .store
            .transform(
                &format!("strava/recordings/{}", input.event.bike_user_id),
                &bike_core::entities::activity_recordings::Recording::from(parsed.derived_data.clone()),
            )
            .await?;
        let saved = activities::Model::save_parsed_strava(
            txn,
            &recording,
            &input.event,
            &import,
            parsed,
            profile,
            &summary.activity,
        )
        .await?;
        Ok((saved, import))
    }

    /// Called under the connection lock. A committed original result wins over
    /// classification of a retry's older fetched snapshot.
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

    async fn mark_fitness_change(
        db: &impl sea_orm::ConnectionTrait,
        event: &StravaEvent,
        draft: &bike_core::activity_summary::ActivityDraft,
        previous: Option<&activities::Model>,
    ) -> Result<Option<chrono::NaiveDate>, WorkflowError> {
        let cycling_changed = bike_core::activity_sport::is_bike_activity_sport(&draft.sport)
            || previous.is_some_and(activities::Model::is_bike_activity);
        if !cycling_changed {
            return Ok(None);
        }
        let started_on = draft.started_at.date_naive();
        let dirty_from = previous.map_or(started_on, |activity| activity.started_at.date_naive().min(started_on));
        bike_core::analytics::mark_user_fitness_dirty(db, event.bike_user_id, dirty_from, chrono::Utc::now()).await?;
        Ok(Some(dirty_from))
    }

    async fn persist_matching_content(
        db: &DatabaseTransaction,
        input: &NormalizedActivity,
        activity: Option<&activities::Model>,
    ) -> Result<Option<ActivityChange>, WorkflowError> {
        let Some(activity) = activity else {
            return Ok(None);
        };
        if !strava_activity_states::Model::matches_content(db, &input.event, &input.source.sha256).await? {
            return Ok(None);
        }
        Self::persist_unchanged(db, input, activity).await.map(Some)
    }

    async fn persist_unchanged(
        db: &DatabaseTransaction,
        input: &NormalizedActivity,
        activity: &activities::Model,
    ) -> Result<ActivityChange, WorkflowError> {
        let mut change = ActivityChange::unchanged(input.event.clone(), ActivityStatus::Unchanged);
        change.event.bike_activity_id = Some(activity.id);
        change.bike_activity_id = Some(activity.id);
        change.import_id = activity.activity_import_id;
        change.segment_ids = bike_core::activity_lifecycle::load_segment_ids_for_activity(db, activity.id).await?;
        change.dirty_from = activity.is_bike_activity().then_some(activity.started_at.date_naive());
        let state =
            strava_activity_states::Model::apply_event(db, &change.event, Some(input.source.sha256.clone()), false)
                .await?;
        activity_event_history::Model::record_strava(db, &change, &state, Some(activity), Some(activity)).await?;
        strava_events::Model::record_mutation(db, &change).await?;
        Ok(change)
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
