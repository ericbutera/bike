use crate::{
    admin_task_history::{self, HistoryQuery},
    app_error::{ApiErrorResponse, AppError},
    storage::AppStorage,
};
use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use bike_core::{
    auth::AdminUserContext,
    jobs::history::{TaskExecutionInput, TaskExecutionPage},
};
use std::sync::Arc;

pub fn routes() -> Router<Arc<AppStorage>> {
    Router::new()
        .route("/tasks", get(list))
        .route("/tasks/:workflow_id/runs/:run_id/input", get(input))
}

#[utoipa::path(
    get, path = "/admin/tasks", params(HistoryQuery),
    responses(
        (status = 200, description = "Retained executions in native Temporal visibility order", body = TaskExecutionPage),
        (status = 400, description = "Invalid filter or page token", body = ApiErrorResponse),
        (status = 401, description = "Unauthenticated"), (status = 403, description = "Administrator required"),
        (status = 502, description = "Temporal history unavailable", body = ApiErrorResponse)
    ), tag = "admin", security(("bearer_auth" = []))
)]
pub async fn list(
    _admin: AdminUserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<TaskExecutionPage>, AppError> {
    admin_task_history::list(&state, query).await.map(Json)
}

#[utoipa::path(
    get, path = "/admin/tasks/{workflow_id}/runs/{run_id}/input",
    params(("workflow_id" = String, Path), ("run_id" = String, Path)),
    responses(
        (status = 200, description = "Original workflow input parameters", body = TaskExecutionInput),
        (status = 400, description = "Invalid workflow or run ID", body = ApiErrorResponse),
        (status = 401, description = "Unauthenticated"), (status = 403, description = "Administrator required"),
        (status = 502, description = "Workflow input unavailable", body = ApiErrorResponse)
    ), tag = "admin", security(("bearer_auth" = []))
)]
pub async fn input(
    _admin: AdminUserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Path((workflow_id, run_id)): Path<(String, String)>,
) -> Result<Json<TaskExecutionInput>, AppError> {
    admin_task_history::input(&state, workflow_id, run_id).await.map(Json)
}

#[cfg(test)]
mod tests;
