use crate::activity_import_pipeline::activity_processing_stages;
use crate::activity_parser::ParsedActivityData;
use crate::entities::{activity_import_attempts as attempts, activity_imports};
use crate::workflow_error::WorkflowError;
use chrono::{DateTime, Utc};
use sea_orm::ConnectionTrait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct StageRecord {
    pub stage: String,
    pub status: String,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub started_at: Option<DateTime<Utc>>,
    #[schema(value_type = Option<String>, format = DateTime)]
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
    attempts::Model::create_attempt(db, import, source, requested_stage, start_stage, previous).await
}

pub(crate) fn initial_stages(
    start_stage: &str,
    previous: Option<&attempts::Model>,
) -> Result<Vec<StageRecord>, WorkflowError> {
    let previous_records = previous.map(stages).transpose()?.unwrap_or_default();
    let mut reuse = true;
    Ok(activity_processing_stages()
        .iter()
        .map(|node| {
            if node.id() == start_stage {
                reuse = false;
            }
            if reuse {
                if let Some(record) = previous_records.iter().find(|record| record.stage == node.id()) {
                    let mut record = record.clone();
                    record.status = "reused".into();
                    record.reused_attempt_id = previous.map(|attempt| attempt.id);
                    return record;
                }
            }
            StageRecord {
                stage: node.id().into(),
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
    attempts::Model::record_stage(db, attempt, record, checkpoint, activity_id).await
}

pub(crate) async fn record_activity_lineage(db: &impl ConnectionTrait, activity_id: i32) -> Result<(), WorkflowError> {
    use crate::background_jobs::{
        entities::{pipeline_outputs, pipeline_runs, pipeline_subjects},
        pipeline::PipelineContext,
    };

    let Some(context) = PipelineContext::current() else {
        return Ok(());
    };
    pipeline_runs::Model::record_origin(db, &context).await?;
    pipeline_subjects::Model::attach(db, &context.run_id, "activity", activity_id).await?;
    if let Some(activity) = crate::entities::activities::Model::find_by_id(db, activity_id).await? {
        let revision = activity.updated_at.timestamp_micros().to_string();
        pipeline_outputs::Model::require(db, "activity", activity_id, revision.clone()).await?;
        pipeline_outputs::Model::publish(db, "activity", activity_id, &revision, activity.updated_at).await?;
    }
    if crate::heatmaps::projection::Projection::enabled(db).await? {
        if let Some(projection) = crate::entities::heatmap_projections::Model::for_activity(db, activity_id).await? {
            let revision = crate::heatmaps::projection::Projection::revision(projection.generation);
            pipeline_outputs::Model::require(db, "heatmap", activity_id, revision.clone()).await?;
            if ["ready", "skipped"].contains(&projection.status.as_str()) {
                if let Some(published) = projection.published_at {
                    pipeline_outputs::Model::publish(db, "heatmap", activity_id, &revision, published).await?;
                }
            }
        }
    }
    Ok(())
}

pub async fn finish_active(
    db: &impl ConnectionTrait,
    import: &activity_imports::Model,
    outcome: &str,
    error: Option<&str>,
) -> Result<(), WorkflowError> {
    attempts::Model::finish_active(db, import, outcome, error).await
}

pub(crate) fn json_error(_: serde_json::Error) -> WorkflowError {
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
