use super::*;
use crate::{
    activity_import_pipeline::ActivityProcessingArtifact,
    activity_parser::ParsedActivityData,
    activity_reprocessing::{revision, RefreshScope},
    bike_tasks::{ImportExecution, ImportTarget},
    entities::{activity_event_history, activity_imports, heatmap_projections},
    training_profile::TrainingProfile,
};
use sea_orm::TransactionTrait;

impl Model {
    pub(crate) async fn apply_reprocessing(
        self,
        db: &DatabaseConnection,
        execution: &ImportExecution,
        artifact: &ActivityProcessingArtifact,
        parsed: &ParsedActivityData,
        profile: &TrainingProfile,
    ) -> Result<RefreshScope, WorkflowError> {
        let txn = db.begin().await?;
        let current = Self::lock_owned(&txn, self.id, self.user_id).await?;
        // A retry may have missed a commit before waiting for this row lock.
        if let Some(scope) = activity_event_history::Model::import_refresh(&txn, execution).await? {
            txn.commit().await?;
            return Ok(scope);
        }
        let before = current.ok_or_else(|| WorkflowError::not_found("Activity was deleted"))?;
        before.check_reprocess_target(execution)?;
        let saved = before
            .clone()
            .update_parsed(
                &txn,
                artifact,
                parsed,
                parsed::build_save_data(parsed, &artifact.original_filename, profile)?,
            )
            .await?;
        let scope = saved.refresh_after_reprocessing(&txn, &before, parsed).await?;
        activity_imports::Model::complete_native_reprocess(&txn, execution).await?;
        let mut event = activity_event_history::Event::import(execution, Some(&before), &saved)?;
        event.context["refresh_scope"] =
            serde_json::to_value(&scope).map_err(|error| WorkflowError::internal(error.to_string()))?;
        activity_event_history::Model::append_import(&txn, event).await?;
        txn.commit().await?;
        Ok(scope)
    }

    async fn refresh_after_reprocessing(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        before: &Self,
        parsed: &ParsedActivityData,
    ) -> Result<RefreshScope, WorkflowError> {
        let route = if self.is_bike_activity() {
            parsed.derived_data.route_points.as_slice()
        } else {
            &[]
        };
        let segment_ids = crate::activity_lifecycle::refresh_activity_derived_state_without_cache_rebuilds(
            txn,
            self.user_id,
            self.id,
            route,
        )
        .await?;
        crate::analytics::rebuild_activity_analytics_cache(txn, &[self.id]).await?;
        if self.is_bike_activity() {
            crate::activity_training_analysis::rebuild_activity_training_analysis_cache(txn, &[self.id]).await?;
        } else {
            crate::entities::activity_training_analyses::Model::clear_activity(txn, self.id).await?;
        }
        let dirty_from = before.started_at.date_naive().min(self.started_at.date_naive());
        crate::analytics::mark_user_fitness_dirty(txn, self.user_id, dirty_from, self.updated_at).await?;
        crate::analytics::mark_segment_activity_changes(txn, &segment_ids, self.updated_at).await?;
        // The activity-update trigger invalidates its existing heatmap generation.
        Ok(RefreshScope {
            revision: revision(self.updated_at)?,
            dirty_from,
            segment_ids,
            heatmap_generation: heatmap_projections::Model::for_activity(txn, self.id)
                .await?
                .map(|projection| {
                    u64::try_from(projection.generation)
                        .map_err(|_| WorkflowError::internal("Invalid heatmap generation"))
                })
                .transpose()?,
        })
    }

    fn check_reprocess_target(&self, execution: &ImportExecution) -> Result<(), WorkflowError> {
        let ImportTarget::ExistingActivity {
            activity_id,
            expected_revision,
        } = execution.command.target
        else {
            return Err(WorkflowError::bad_request("Expected a reprocess command"));
        };
        let target_matches = self.id == activity_id && self.user_id == execution.command.source.user_id;
        let source_matches = self.activity_import_id == Some(execution.command.source.activity_import_id)
            && self.source != "strava_sync";
        if !target_matches || !source_matches {
            return Err(WorkflowError::conflict(
                "Reprocess target does not match its retained source",
            ));
        }
        if revision(self.updated_at)? != expected_revision {
            return Err(WorkflowError::conflict(
                "Activity changed since reprocessing was requested",
            ));
        }
        Ok(())
    }
}
