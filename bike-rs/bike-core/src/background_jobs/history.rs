//! Compact admin history. Source payloads and checkpoints stay in their owners.
use super::entities::{pipeline_tasks, task_attempts};
use sea_orm::{DatabaseConnection, DbErr};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct TaskAttemptResponse {
    pub attempt: i32,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub heartbeat_at: String,
    /// Last recorded checkpoint; heartbeats do not update this value.
    pub progress_at: String,
    pub outcome: String,
    pub error: Option<String>,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub eligible_at: Option<String>,
    pub processing_version: Option<String>,
    pub workload_cohort: Option<String>,
}

impl From<task_attempts::Model> for TaskAttemptResponse {
    fn from(attempt: task_attempts::Model) -> Self {
        Self {
            attempt: attempt.attempt,
            started_at: attempt.started_at.to_rfc3339(),
            finished_at: attempt.finished_at.map(|time| time.to_rfc3339()),
            heartbeat_at: attempt.heartbeat_at.to_rfc3339(),
            progress_at: attempt.progress_at.to_rfc3339(),
            outcome: attempt.outcome,
            error: attempt.error,
            trace_id: attempt.trace_id,
            span_id: attempt.span_id,
            eligible_at: attempt.eligible_at.map(|time| time.to_rfc3339()),
            processing_version: attempt.processing_version,
            workload_cohort: attempt.workload_cohort,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TaskPipelineResponse {
    pub run_id: String,
    pub pipeline_started_at: String,
    pub accepted_at: String,
    pub entrypoint: String,
    pub request_id: Option<String>,
    pub trace_id: Option<String>,
    pub parent_task_id: Option<i32>,
    /// Output readiness is unknown until a publication barrier records it.
    pub available_at: Option<String>,
}

pub async fn load(
    db: &DatabaseConnection,
    task_id: i32,
) -> Result<(Vec<TaskAttemptResponse>, Vec<TaskPipelineResponse>), DbErr> {
    let attempts = task_attempts::Model::for_task(db, task_id)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();
    let pipelines = pipeline_tasks::Model::origins(db, task_id)
        .await?
        .into_iter()
        .filter_map(|(link, run)| {
            run.map(|run| TaskPipelineResponse {
                run_id: run.id,
                pipeline_started_at: run.pipeline_started_at.to_rfc3339(),
                accepted_at: run.accepted_at.to_rfc3339(),
                entrypoint: run.entrypoint,
                request_id: run.request_id,
                trace_id: run.trace_id,
                parent_task_id: link.parent_task_id,
                available_at: run.available_at.map(|time| time.to_rfc3339()),
            })
        })
        .collect();
    Ok((attempts, pipelines))
}
