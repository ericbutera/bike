use super::*;
use crate::activity_import_execution::{
    initial_stages, json_error, record_activity_lineage, stages, ProcessingCheckpoint, StageRecord,
};
use crate::entities::{activity_import_attempts as attempts, activity_imports};
use crate::workflow_error::WorkflowError;
use chrono::Utc;
use sea_orm::ActiveModelTrait;

impl Model {
    pub async fn create_attempt(
        db: &impl ConnectionTrait,
        import: &activity_imports::Model,
        source: serde_json::Value,
        requested_stage: &str,
        start_stage: &str,
        previous: Option<&attempts::Model>,
    ) -> Result<attempts::Model, WorkflowError> {
        let records = initial_stages(start_stage, previous)?;
        let attempt = attempts::ActiveModel {
            user_id: Set(import.user_id),
            activity_import_id: Set(import.id),
            activity_id: Set(import.activity_id),
            requested_stage: Set(requested_stage.into()),
            start_stage: Set(start_stage.into()),
            current_stage: Set(start_stage.into()),
            reused_attempt_id: Set(previous.map(|attempt| attempt.id)),
            source_json: Set(source),
            stages_json: Set(serde_json::to_value(records).map_err(json_error)?),
            checkpoint_json: Set(previous.and_then(|attempt| attempt.checkpoint_json.clone())),
            ..Default::default()
        };
        let attempt = attempt.insert(db).await.map_err(|error| {
            if matches!(error.sql_err(), Some(sea_orm::SqlErr::UniqueConstraintViolation(_))) {
                WorkflowError::conflict("This import or its activity already has a queued or running attempt")
            } else {
                WorkflowError::from(error)
            }
        })?;
        crate::background_jobs::entities::pipeline_outputs::Model::require(
            db,
            "import",
            import.id,
            attempt.id.to_string(),
        )
        .await?;
        Ok(attempt)
    }

    pub async fn record_stage(
        db: &impl ConnectionTrait,
        attempt: attempts::Model,
        record: StageRecord,
        checkpoint: Option<&ProcessingCheckpoint>,
        activity_id: Option<i32>,
    ) -> Result<attempts::Model, WorkflowError> {
        let mut records = stages(&attempt)?;
        let current = records
            .iter_mut()
            .find(|item| item.stage == record.stage)
            .ok_or_else(|| WorkflowError::internal("Unknown ingestion stage"))?;
        *current = record.clone();
        let previous_checkpoint = attempt.checkpoint_json.clone();
        let mut active: attempts::ActiveModel = attempt.into();
        active.current_stage = Set(record.stage);
        active.stages_json = Set(serde_json::to_value(records).map_err(json_error)?);
        active.updated_at = Set(Utc::now());
        if let Some(context) = crate::background_jobs::execution::ExecutionContext::current() {
            active.worker_task_id = Set(Some(context.task_id));
            crate::background_jobs::entities::task_attempts::Model::progress(db, context.task_id, context.attempt)
                .await?;
        }
        if let Some(id) = activity_id {
            active.activity_id = Set(Some(id));
            record_activity_lineage(db, id).await?;
        }
        if let Some(checkpoint) = checkpoint {
            let value = serde_json::to_value(checkpoint).map_err(json_error)?;
            if previous_checkpoint.as_ref() != Some(&value) {
                active.checkpoint_json = Set(Some(value));
            }
        }
        Ok(active.update(db).await?)
    }

    pub async fn finish_active(
        db: &impl ConnectionTrait,
        import: &activity_imports::Model,
        outcome: &str,
        error: Option<&str>,
    ) -> Result<(), WorkflowError> {
        let Some(attempt) = attempts::Entity::active(db, import.user_id, import.id).await? else {
            return Ok(());
        };
        let mut records = stages(&attempt)?;
        if let Some(error) = error {
            if let Some(record) = records
                .iter_mut()
                .find(|record| record.stage == attempt.current_stage && record.status == "running")
            {
                record.status = "failed".into();
                record.error = Some(error.into());
                record.completed_at = Some(Utc::now());
            }
        } else if outcome == "duplicate" {
            for record in &mut records {
                if record.status == "pending" {
                    record.status = "skipped".into();
                    record.summary = vec!["Matched an existing activity; downstream work was skipped".into()];
                }
            }
        }
        let revision = attempt.id.to_string();
        let finished_at = Utc::now();
        let mut active: attempts::ActiveModel = attempt.into();
        active.status = Set(outcome.into());
        active.activity_id = Set(import.activity_id);
        active.error = Set(error.map(str::to_owned));
        active.stages_json = Set(serde_json::to_value(records).map_err(json_error)?);
        active.finished_at = Set(Some(finished_at));
        active.updated_at = Set(Utc::now());
        active.update(db).await?;
        if error.is_none() && ["completed", "duplicate"].contains(&outcome) {
            crate::background_jobs::entities::pipeline_outputs::Model::publish(
                db,
                "import",
                import.id,
                &revision,
                finished_at,
            )
            .await?;
        }
        Ok(())
    }
}
