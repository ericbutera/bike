use super::*;
use crate::{
    activity_import_execution::{initial_stages, json_error},
    bike_tasks::{ActivityImportResult, ImportExecution, ImportTarget},
    entities::activity_imports,
    workflow_error::WorkflowError,
};
use sea_orm::sea_query::OnConflict;
#[cfg(test)]
mod tests;

impl Model {
    /// Idempotent domain history, written by the Activity after Temporal accepts work.
    pub async fn record_temporal(
        db: &impl ConnectionTrait,
        execution: &ImportExecution,
    ) -> Result<Self, WorkflowError> {
        let command = &execution.command;
        command.validate()?;
        if let Some(attempt) = Self::for_command(db, execution).await? {
            return attempt.check_execution(execution);
        }
        let import = activity_imports::Entity::find_owned(
            db,
            command.source.user_id,
            command.source.activity_import_id,
        )
        .await?
        .ok_or_else(|| WorkflowError::not_found("Activity import not found"))?;
        if let ImportTarget::ExistingActivity { activity_id, .. } = command.target {
            if import.activity_id != Some(activity_id) {
                return Err(WorkflowError::conflict(
                    "Import does not belong to the target activity",
                ));
            }
        }
        let now = chrono::Utc::now();
        Entity::insert(ActiveModel {
            user_id: Set(import.user_id),
            activity_import_id: Set(import.id),
            activity_id: Set(import.activity_id),
            command_id: Set(Some(command.source.command_id.clone())),
            temporal_context: Set(Some(serde_json::to_value(execution).map_err(json_error)?)),
            status: Set(TEMPORAL_STARTED.into()),
            requested_stage: Set("raw_stored".into()),
            start_stage: Set("raw_stored".into()),
            current_stage: Set("raw_stored".into()),
            source_json: Set(serde_json::json!({
                "filename": import.original_filename, "format": import.format,
            })),
            stages_json: Set(
                serde_json::to_value(initial_stages("raw_stored", None)?).map_err(json_error)?
            ),
            started_at: Set(Some(now)),
            ..Default::default()
        })
        .on_conflict(
            OnConflict::columns([Column::UserId, Column::CommandId])
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(db)
        .await?;
        Self::for_command(db, execution)
            .await?
            .ok_or_else(|| WorkflowError::internal("Import attempt was not recorded"))?
            .check_execution(execution)
    }

    async fn for_command(
        db: &impl ConnectionTrait,
        execution: &ImportExecution,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(Column::UserId.eq(execution.command.source.user_id))
            .filter(Column::CommandId.eq(&execution.command.source.command_id))
            .one(db)
            .await
    }

    fn check_execution(self, execution: &ImportExecution) -> Result<Self, WorkflowError> {
        let stored: ImportExecution = serde_json::from_value(
            self.temporal_context
                .clone()
                .ok_or_else(|| WorkflowError::conflict("Missing import execution context"))?,
        )
        .map_err(json_error)?;
        if !stored.matches_request(execution) {
            return Err(WorkflowError::conflict(
                "Import command already identifies different work",
            ));
        }
        Ok(self)
    }

    pub async fn complete_temporal(
        db: &impl ConnectionTrait,
        attempt_id: i32,
        result: &ActivityImportResult,
    ) -> Result<(), WorkflowError> {
        let now = chrono::Utc::now();
        let updated = Entity::update_many()
            .set(ActiveModel {
                activity_id: Set(Some(result.activity_id)),
                status: Set("completed".into()),
                current_stage: Set("complete".into()),
                finished_at: Set(Some(now)),
                updated_at: Set(now),
                ..Default::default()
            })
            .filter(Column::Id.eq(attempt_id))
            .filter(Column::ActivityImportId.eq(result.activity_import_id))
            .exec(db)
            .await?;
        if updated.rows_affected != 1 {
            return Err(WorkflowError::conflict(
                "Import attempt completion did not match",
            ));
        }
        Ok(())
    }
}
