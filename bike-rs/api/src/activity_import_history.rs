use crate::app_error::AppError;
use crate::controllers::activity_imports::{
    ActivityImportResponse, ActivityImportTraceNodeResponse,
};
use bike_core::activity_import_execution;
use bike_core::activity_import_pipeline::{
    activity_processing_graph_nodes, plan_activity_import_replay,
    queue_expected_activity_import_replay,
};
use bike_core::entities::{activity_import_attempts, activity_imports};
use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[cfg(test)]
#[path = "activity_import_history_tests.rs"]
mod tests;

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct HistoryQuery {
    pub page: Option<u64>,
    pub source: Option<String>,
    pub status: Option<String>,
    pub archive_job_id: Option<i32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ImportHistoryResponse {
    pub items: Vec<ActivityImportResponse>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ReplayRequest {
    pub stage: String,
    pub expected_start_stage: Option<String>,
    pub expected_reused_attempt_id: Option<i32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ReplayResponse {
    pub requested_stage: String,
    pub start_stage: String,
    pub reused_attempt_id: Option<i32>,
    pub reason: Option<String>,
    pub attempt_id: Option<i32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ImportAttemptResponse {
    pub id: i32,
    pub status: String,
    pub requested_stage: String,
    pub start_stage: String,
    pub reused_attempt_id: Option<i32>,
    pub source: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub nodes: Vec<ActivityImportTraceNodeResponse>,
}

pub async fn history(
    db: &DatabaseConnection,
    user_id: i32,
    query: HistoryQuery,
) -> Result<ImportHistoryResponse, AppError> {
    recover_owned_imports(db, user_id).await?;
    let page = query.page.unwrap_or(1).max(1);
    let (items, total) = activity_imports::Entity::history(
        db,
        user_id,
        page,
        query.source.as_deref().filter(|value| !value.is_empty()),
        query.status.as_deref().filter(|value| !value.is_empty()),
        query.archive_job_id,
    )
    .await?;
    Ok(ImportHistoryResponse {
        items: items
            .into_iter()
            .map(|import| ActivityImportResponse::from_model(import, None))
            .collect(),
        total,
        page,
        per_page: 25,
    })
}

async fn recover_owned_imports(db: &DatabaseConnection, user_id: i32) -> Result<(), AppError> {
    bike_core::activity_import_recovery::recover_stale_activity_imports_for_user(
        db,
        &bike_core::jobs::JobQueue::new(db.clone()),
        user_id,
        Utc::now(),
    )
    .await
    .map_err(|error| AppError::internal(error.message))?;
    Ok(())
}

pub async fn owned_import_for_inspection(
    db: &DatabaseConnection,
    user_id: i32,
    import_id: i32,
) -> Result<activity_imports::Model, AppError> {
    recover_owned_imports(db, user_id).await?;
    activity_imports::Entity::find_owned(db, user_id, import_id)
        .await?
        .ok_or_else(|| AppError::not_found("Activity import not found"))
}

pub async fn replay(
    db: &DatabaseConnection,
    uploads_dir: &str,
    user_id: i32,
    import_id: i32,
    request: ReplayRequest,
    queue: bool,
) -> Result<ReplayResponse, AppError> {
    let import = activity_imports::Entity::find_owned(db, user_id, import_id)
        .await?
        .ok_or_else(|| AppError::not_found("Activity import not found"))?;
    let (plan, attempt_id) = if queue {
        let expected =
            plan_activity_import_replay(db, uploads_dir, &import, &request.stage).await?;
        if request.expected_start_stage.as_ref().is_some_and(|stage| {
            stage != &expected.start_stage
                || request.expected_reused_attempt_id != expected.reused_attempt_id
        }) {
            return Err(AppError::conflict(
                "Replay prerequisites changed; review the updated start stage and try again",
            ));
        }
        let (plan, id) =
            queue_expected_activity_import_replay(db, uploads_dir, &import, &expected).await?;
        (plan, Some(id))
    } else {
        (
            plan_activity_import_replay(db, uploads_dir, &import, &request.stage).await?,
            None,
        )
    };
    Ok(ReplayResponse {
        requested_stage: plan.requested_stage,
        start_stage: plan.start_stage,
        reused_attempt_id: plan.reused_attempt_id,
        reason: plan.reason,
        attempt_id,
    })
}

pub async fn attempts(
    db: &DatabaseConnection,
    import: &activity_imports::Model,
) -> Result<Vec<ImportAttemptResponse>, AppError> {
    activity_import_attempts::Entity::recent_metadata(db, import.user_id, import.id)
        .await?
        .into_iter()
        .map(attempt_response)
        .collect()
}

fn attempt_response(
    attempt: activity_import_attempts::Model,
) -> Result<ImportAttemptResponse, AppError> {
    let records = activity_import_execution::stages(&attempt)?;
    let nodes = activity_processing_graph_nodes()
        .iter()
        .map(|node| {
            let record = records.iter().find(|record| record.stage == node.stage);
            ActivityImportTraceNodeResponse {
                id: node.node.id().into(),
                label: node.node.label().into(),
                stage: node.stage.into(),
                status: record
                    .map_or("unknown", |record| record.status.as_str())
                    .into(),
                started_at: record.and_then(|record| record.started_at),
                completed_at: record.and_then(|record| record.completed_at),
                summary: record.map_or_else(Vec::new, |record| record.summary.clone()),
                error: record.and_then(|record| record.error.clone()),
                reused_attempt_id: record.and_then(|record| record.reused_attempt_id),
            }
        })
        .collect();
    let mut source = attempt.source_json;
    if let Some(source) = source.as_object_mut() {
        source.remove("storage_path");
    }
    Ok(ImportAttemptResponse {
        id: attempt.id,
        status: attempt.status,
        requested_stage: attempt.requested_stage,
        start_stage: attempt.start_stage,
        reused_attempt_id: attempt.reused_attempt_id,
        source,
        created_at: attempt.created_at,
        started_at: attempt.started_at,
        finished_at: attempt.finished_at,
        error: attempt.error,
        nodes,
    })
}
