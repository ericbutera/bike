use crate::activity_import_pipeline::activity_processing_graph_nodes;
use crate::activity_parser::ParsedActivityData;
use crate::entities::{activity_import_attempts as attempts, activity_imports};
use crate::workflow_error::WorkflowError;
use chrono::{DateTime, Utc};
use sea_orm::{ActiveModelTrait, ConnectionTrait, Set};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StageRecord {
    pub stage: String,
    pub status: String,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub summary: Vec<String>,
    pub error: Option<String>,
    pub reused_attempt_id: Option<i32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessingCheckpoint {
    pub version: i32,
    pub parsed: ParsedActivityData,
    pub affected_segment_ids: Vec<i32>,
    pub activity_updated_at: Option<DateTime<Utc>>,
}

pub fn stages(attempt: &attempts::Model) -> Result<Vec<StageRecord>, WorkflowError> {
    serde_json::from_value(attempt.stages_json.clone())
        .map_err(|_| WorkflowError::internal("Stored ingestion stage records are invalid"))
}

pub fn checkpoint(attempt: &attempts::Model) -> Option<ProcessingCheckpoint> {
    attempt
        .checkpoint_json
        .clone()
        .and_then(|value| serde_json::from_value(value).ok())
}

pub async fn create_attempt(
    db: &impl ConnectionTrait,
    import: &activity_imports::Model,
    source: serde_json::Value,
    requested_stage: &str,
    start_stage: &str,
    previous: Option<&attempts::Model>,
) -> Result<attempts::Model, WorkflowError> {
    let records = initial_stages(start_stage, previous)?;
    let now = Utc::now();
    let attempt = attempts::ActiveModel {
        user_id: Set(import.user_id),
        activity_import_id: Set(import.id),
        activity_id: Set(import.activity_id),
        status: Set("queued".into()),
        requested_stage: Set(requested_stage.into()),
        start_stage: Set(start_stage.into()),
        current_stage: Set(start_stage.into()),
        reused_attempt_id: Set(previous.map(|attempt| attempt.id)),
        source_json: Set(source),
        stages_json: Set(serde_json::to_value(records).map_err(json_error)?),
        checkpoint_json: Set(previous.and_then(|attempt| attempt.checkpoint_json.clone())),
        error: Set(None),
        created_at: Set(now),
        started_at: Set(None),
        finished_at: Set(None),
        updated_at: Set(now),
        ..Default::default()
    };
    attempt.insert(db).await.map_err(|error| {
        if matches!(
            error.sql_err(),
            Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
        ) {
            WorkflowError::conflict(
                "This import or its activity already has a queued or running attempt",
            )
        } else {
            WorkflowError::from(error)
        }
    })
}

fn initial_stages(
    start_stage: &str,
    previous: Option<&attempts::Model>,
) -> Result<Vec<StageRecord>, WorkflowError> {
    let previous_records = previous.map(stages).transpose()?.unwrap_or_default();
    let mut reuse = true;
    Ok(activity_processing_graph_nodes()
        .iter()
        .map(|node| {
            if node.stage == start_stage {
                reuse = false;
            }
            if reuse {
                if let Some(record) = previous_records
                    .iter()
                    .find(|record| record.stage == node.stage)
                {
                    let mut record = record.clone();
                    record.status = "reused".into();
                    record.reused_attempt_id = previous.map(|attempt| attempt.id);
                    return record;
                }
            }
            StageRecord {
                stage: node.stage.into(),
                status: "pending".into(),
                started_at: None,
                completed_at: None,
                summary: Vec::new(),
                error: None,
                reused_attempt_id: None,
            }
        })
        .collect())
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
    if let Some(id) = activity_id {
        active.activity_id = Set(Some(id));
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
                record.summary =
                    vec!["Matched an existing activity; downstream work was skipped".into()];
            }
        }
    }
    let mut active: attempts::ActiveModel = attempt.into();
    active.status = Set(outcome.into());
    active.activity_id = Set(import.activity_id);
    active.error = Set(error.map(str::to_owned));
    active.stages_json = Set(serde_json::to_value(records).map_err(json_error)?);
    active.finished_at = Set(Some(Utc::now()));
    active.updated_at = Set(Utc::now());
    active.update(db).await?;
    Ok(())
}

fn json_error(_: serde_json::Error) -> WorkflowError {
    WorkflowError::internal("Could not serialize ingestion execution state")
}

pub async fn record_batch_stage(
    db: &impl ConnectionTrait,
    import: &activity_imports::Model,
    stage: &str,
) -> Result<(), WorkflowError> {
    let Some(attempt) = attempts::Entity::active(db, import.user_id, import.id).await? else {
        return Ok(());
    };
    let Some(mut record) = stages(&attempt)?
        .into_iter()
        .find(|record| record.stage == stage && record.status == "pending")
    else {
        return Ok(());
    };
    record.status = "completed".into();
    record.completed_at = Some(Utc::now());
    record.summary = vec!["Completed in the batch rebuild".into()];
    record_stage(db, attempt, record, None, import.activity_id).await?;
    Ok(())
}
