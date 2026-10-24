//! Provider identity and ordering guards surround the shared activity save.
#[cfg(test)]
mod tests;
use super::Model;
use crate::{
    activity_import_pipeline::ActivityUploadPayload,
    activity_parser::ParsedActivityData,
    entities::{activity_event_history, activity_imports, strava_activity_states, strava_connections, strava_events},
    object_store::ObjectStore,
    training_profile::TrainingProfile,
    workflow_error::WorkflowError,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DatabaseTransaction, TransactionTrait};
use strava_events::{ActivityChange, ActivityStatus, RetainedActivity, StravaEvent};

impl Model {
    pub(crate) async fn apply_strava_source(
        db: &DatabaseConnection,
        objects: &ObjectStore,
        input: RetainedActivity,
        parsed: &ParsedActivityData,
        profile: &TrainingProfile,
        upload: &ActivityUploadPayload,
    ) -> Result<ActivityChange, WorkflowError> {
        let txn = db.begin().await?;
        strava_connections::Model::lock_event(&txn, &input.event).await?;
        if let Some(change) = Self::strava_source_conflict(&txn, &input.event).await? {
            txn.commit().await?;
            return Ok(change);
        }
        let existing = Self::lock_strava_activity(&txn, input.event.bike_user_id, input.event.activity_id()?).await?;
        let unchanged = existing.is_some()
            && strava_activity_states::Model::matches_content(&txn, &input.event, &input.source.sha256).await?;
        let previous_segments = match &existing {
            Some(activity) => crate::activity_lifecycle::load_segment_ids_for_activity(&txn, activity.id).await?,
            None => vec![],
        };
        let import = activity_imports::Entity::native_strava_source(
            &txn,
            &input.event,
            existing.as_ref().and_then(|activity| activity.activity_import_id),
            &input.source,
            upload,
        )
        .await?;
        let saved = Self::save_parsed_strava(&txn, objects, &input.event, &import, parsed, profile).await?;
        activity_imports::Model::complete_source(&txn, saved.user_id, import.id, saved.id).await?;
        let mut change = ActivityChange::applied(input.event, saved.id, import.id, saved.started_at.date_naive());
        if unchanged {
            change.status = ActivityStatus::Unchanged;
        }
        change.segment_ids = previous_segments;
        change.dirty_from = Self::mark_strava_fitness_change(&txn, &change.event, parsed, existing.as_ref()).await?;
        Self::record_strava_change(&txn, &change, input.source.sha256, existing.as_ref(), &saved).await?;
        txn.commit().await?;
        Ok(change)
    }

    async fn record_strava_change(
        db: &DatabaseTransaction,
        change: &ActivityChange,
        hash: String,
        previous: Option<&Self>,
        saved: &Self,
    ) -> Result<(), WorkflowError> {
        let state = strava_activity_states::Model::apply_event(db, &change.event, Some(hash), false).await?;
        activity_event_history::Model::record_strava(db, change, &state, previous, Some(saved)).await?;
        strava_events::Model::record_mutation(db, change).await?;
        Ok(())
    }

    async fn strava_source_conflict(
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
        if strava_activity_states::Model::supersedes(db, event, false).await? {
            let change = ActivityChange::unchanged(event.clone(), ActivityStatus::Superseded);
            strava_events::Model::record_mutation(db, &change).await?;
            return Ok(Some(change));
        }
        Ok(None)
    }

    async fn mark_strava_fitness_change(
        db: &impl ConnectionTrait,
        event: &StravaEvent,
        parsed: &ParsedActivityData,
        previous: Option<&Self>,
    ) -> Result<Option<chrono::NaiveDate>, WorkflowError> {
        let cycling_changed = crate::activity_sport::is_bike_activity_sport(&parsed.draft.sport)
            || previous.is_some_and(Self::is_bike_activity);
        if !cycling_changed {
            return Ok(None);
        }
        let started = parsed.draft.started_at.date_naive();
        let dirty_from = previous.map_or(started, |activity| activity.started_at.date_naive().min(started));
        crate::analytics::mark_user_fitness_dirty(db, event.bike_user_id, dirty_from, chrono::Utc::now()).await?;
        Ok(Some(dirty_from))
    }
}
