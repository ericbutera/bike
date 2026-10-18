use crate::{
    admin_tasks::{self, AcceptedTask, ActivityTaskRequest, RequestContext, UserTaskRequest},
    app_error::{ApiErrorResponse, AppError},
    storage::AppStorage,
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use bike_core::auth::AdminUserContext;
use std::sync::Arc;

pub fn routes() -> Router<Arc<AppStorage>> {
    Router::new()
        .route("/activities/:id/tasks", post(activity_task))
        .route("/users/:id/tasks", post(user_task))
}

#[utoipa::path(
    post, path = "/admin/activities/{id}/tasks",
    params(("id" = i32, Path, description = "Bike activity ID")),
    request_body = ActivityTaskRequest,
    responses(
        (status = 202, description = "Temporal acknowledged the activity action", body = AcceptedTask),
        (status = 400, description = "Invalid task parameters", body = ApiErrorResponse),
        (status = 401, description = "Unauthenticated"), (status = 403, description = "Administrator required"),
        (status = 404, description = "Activity or connection missing", body = ApiErrorResponse),
        (status = 422, description = "Unknown task or malformed task parameters"),
        (status = 500, description = "Submission not confirmed; retry the same command ID", body = ApiErrorResponse)
    ), tag = "admin", security(("bearer_auth" = []))
)]
pub async fn activity_task(
    admin: AdminUserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Path(id): Path<i32>,
    headers: HeaderMap,
    Json(request): Json<ActivityTaskRequest>,
) -> Result<(StatusCode, Json<AcceptedTask>), AppError> {
    let result = admin_tasks::activity(&state, context(admin.user.id, &headers), id, request).await?;
    Ok((StatusCode::ACCEPTED, Json(result)))
}

#[utoipa::path(
    post, path = "/admin/users/{id}/tasks",
    params(("id" = i32, Path, description = "Bike user ID")),
    request_body = UserTaskRequest,
    responses(
        (status = 202, description = "Temporal acknowledged the user action", body = AcceptedTask),
        (status = 400, description = "Invalid task parameters", body = ApiErrorResponse),
        (status = 401, description = "Unauthenticated"), (status = 403, description = "Administrator required"),
        (status = 404, description = "User or connection missing", body = ApiErrorResponse),
        (status = 422, description = "Unknown task or malformed task parameters"),
        (status = 500, description = "Submission not confirmed; retry the same command ID", body = ApiErrorResponse)
    ), tag = "admin", security(("bearer_auth" = []))
)]
pub async fn user_task(
    admin: AdminUserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Path(id): Path<i32>,
    headers: HeaderMap,
    Json(request): Json<UserTaskRequest>,
) -> Result<(StatusCode, Json<AcceptedTask>), AppError> {
    let result = admin_tasks::user(&state, context(admin.user.id, &headers), id, request).await?;
    Ok((StatusCode::ACCEPTED, Json(result)))
}

fn context(actor_id: i32, headers: &HeaderMap) -> RequestContext {
    RequestContext {
        actor_id,
        request_id: headers
            .get("x-request-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned),
    }
}

#[cfg(test)]
mod tests;
