use super::*;
use crate::{
    activity_import_pipeline::ActivityProcessingArtifact,
    activity_parser::ParsedActivityData,
    bike_tasks::{ImportExecution, ImportTarget},
    entities::{activity_event_history, activity_imports},
    training_profile::TrainingProfile,
    workflow_error::WorkflowError,
};
use sea_orm::TransactionTrait;

impl Model {
    pub(crate) async fn apply_import(
        db: &DatabaseConnection,
        objects: &crate::object_store::ObjectStore,
        execution: &ImportExecution,
        artifact: &ActivityProcessingArtifact,
        parsed: &ParsedActivityData,
        profile: &TrainingProfile,
    ) -> Result<Self, WorkflowError> {
        let txn = db.begin().await?;
        let source = &execution.command.source;
        // New imports serialize creation on their import row. Reprocessing locks
        // the activity first, matching edit/deletion lock order.
        let import = match execution.command.target {
            ImportTarget::NewActivity => {
                activity_imports::Entity::lock_owned(&txn, source.user_id, source.activity_import_id).await?
            }
            ImportTarget::ExistingActivity { .. } => {
                activity_imports::Entity::find_owned(&txn, source.user_id, source.activity_import_id).await?
            }
        }
        .ok_or_else(|| WorkflowError::not_found("Activity import was deleted"))?;
        import.require_retained_file_processing()?;
        let before = Self::import_target(&txn, execution).await?;
        if before.is_none() {
            if let Some(duplicate) = Self::duplicate_import(&txn, &import, parsed).await? {
                crate::entities::activity_recordings::Model::store(&txn, objects, &duplicate).await?;
                txn.commit().await?;
                return Ok(duplicate);
            }
        }
        let data = parsed::build_save_data(parsed, &artifact.original_filename, profile)?;
        let saved = match &before {
            Some(activity) => activity.clone().update_parsed(&txn, artifact, parsed, data).await?,
            None => Self::insert_parsed(&txn, &import, artifact, parsed, data, None).await?,
        };
        crate::entities::activity_recordings::Model::store(&txn, objects, &saved).await?;
        let dirty_from = before.as_ref().map_or(saved.started_at.date_naive(), |activity| {
            activity.started_at.date_naive().min(saved.started_at.date_naive())
        });
        crate::entities::analytics_user_states::Model::mark_dirty(&txn, saved.user_id, dirty_from, saved.updated_at)
            .await?;
        activity_imports::Model::complete_native_import(&txn, execution, saved.id).await?;
        activity_event_history::Model::append(
            &txn,
            activity_event_history::Event::import(execution, before.as_ref(), &saved)?,
        )
        .await?;
        txn.commit().await?;
        Ok(saved)
    }

    async fn duplicate_import(
        db: &impl ConnectionTrait,
        import: &activity_imports::Model,
        parsed: &ParsedActivityData,
    ) -> Result<Option<Self>, WorkflowError> {
        let sport_values = crate::activity_sport::activity_sport_from_stored(&parsed.draft.sport)
            .map(|sport| sport.stored_values().to_vec())
            .unwrap_or_else(|| vec![parsed.draft.sport.as_str()]);
        let candidates = Self::at_start_for_sports(db, import.user_id, parsed.draft.started_at, &sport_values).await?;
        let Some(activity) = candidates.into_iter().find(|activity| {
            crate::dedupe::activity_dedupe_matches_model(activity, &parsed.draft, &parsed.derived_data.route_points)
        }) else {
            return Ok(None);
        };
        let expected_updated_at = activity.updated_at;
        let Some(activity) = Self::lock_owned(db, activity.id, import.user_id).await? else {
            return Err(WorkflowError::not_found("Duplicate activity was deleted"));
        };
        if activity.updated_at != expected_updated_at {
            return Err(WorkflowError::internal(
                "Activity changed while resolving duplicate recording; retry import",
            ));
        }
        let activity = activity
            .merge_import_recording(db, &parsed.derived_data.recording)
            .await?;
        import.mark_duplicate(db, activity.id).await?;
        Ok(Some(activity))
    }

    async fn merge_import_recording(
        mut self,
        db: &impl ConnectionTrait,
        recording: &crate::activity_recording::RecordingContext,
    ) -> Result<Self, WorkflowError> {
        let mut derived = crate::activity_data::deserialize_derived_activity_data(self.derived_data_json.as_ref());
        let previous = derived.recording.clone();
        derived.recording.merge(recording.clone());
        if derived.recording != previous
            && !Self::store_recording(db, self.id, self.user_id, self.updated_at, &derived).await?
        {
            return Err(WorkflowError::internal(
                "Activity changed while merging recording evidence; retry import",
            ));
        }
        if derived.recording != previous {
            let stored = crate::activity_data::StoredActivityDerivedData::from(&derived);
            self.derived_data_json = Some(
                match self.derived_data_json.as_ref().and_then(|data| data.source_metadata()) {
                    Some(metadata) => stored.with_source_metadata(metadata.clone()),
                    None => stored,
                },
            );
        }
        Ok(self)
    }

    async fn import_target(
        txn: &sea_orm::DatabaseTransaction,
        execution: &ImportExecution,
    ) -> Result<Option<Self>, WorkflowError> {
        let source = &execution.command.source;
        match execution.command.target {
            ImportTarget::NewActivity => {
                Ok(Self::owned_import_activity(txn, source.user_id, source.activity_import_id).await?)
            }
            ImportTarget::ExistingActivity { activity_id } => {
                let activity = Self::lock_owned(txn, activity_id, source.user_id)
                    .await?
                    .ok_or_else(|| WorkflowError::not_found("Activity was deleted"))?;
                if activity.activity_import_id != Some(source.activity_import_id) {
                    return Err(WorkflowError::conflict("Activity no longer belongs to this import"));
                }
                Ok(Some(activity))
            }
        }
    }
}
