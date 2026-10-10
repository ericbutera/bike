use crate::activity_import_history::{
    self as service, HistoryQuery, ImportHistoryResponse, ReplayRequest, ReplayResponse,
};
use crate::app_error::{ApiErrorResponse, AppError};
use crate::storage::AppStorage;
use axum::extract::{Path, Query, State};
use axum::Json;
use bike_core::auth::AdminUserContext;
use std::sync::Arc;

#[utoipa::path(get, path = "/admin/activity-imports/history", params(HistoryQuery),
    responses((status = 200, body = ImportHistoryResponse), (status = 400, body = ApiErrorResponse), (status = 401), (status = 403), (status = 500, body = ApiErrorResponse)),
    tag = "admin", security(("bearer_auth" = [])))]
pub async fn history(
    AdminUserContext { user, .. }: AdminUserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<ImportHistoryResponse>, AppError> {
    let owner_id = query.user_id.unwrap_or(user.id);
    if owner_id <= 0 {
        return Err(AppError::bad_request("User ID must be positive"));
    }
    Ok(Json(service::history(&state.db, owner_id, query).await?))
}

#[utoipa::path(get, path = "/admin/activity-imports/{id}/replay", params(("id" = i32, Path), ReplayRequest),
    responses((status = 200, body = ReplayResponse), (status = 400, body = ApiErrorResponse), (status = 401), (status = 403), (status = 404, body = ApiErrorResponse)),
    tag = "admin", security(("bearer_auth" = [])))]
pub async fn replay_plan(
    _admin: AdminUserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Path(id): Path<i32>,
    Query(request): Query<ReplayRequest>,
) -> Result<Json<ReplayResponse>, AppError> {
    Ok(Json(
        service::admin_replay(&state.db, &state.uploads_dir, id, request, false).await?,
    ))
}

#[utoipa::path(post, path = "/admin/activity-imports/{id}/replay", params(("id" = i32, Path)), request_body = ReplayRequest,
    responses((status = 202, body = ReplayResponse), (status = 400, body = ApiErrorResponse), (status = 401), (status = 403), (status = 404, body = ApiErrorResponse), (status = 409, body = ApiErrorResponse)),
    tag = "admin", security(("bearer_auth" = [])))]
pub async fn replay(
    _admin: AdminUserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Path(id): Path<i32>,
    Json(request): Json<ReplayRequest>,
) -> Result<(axum::http::StatusCode, Json<ReplayResponse>), AppError> {
    Ok((
        axum::http::StatusCode::ACCEPTED,
        Json(service::admin_replay(&state.db, &state.uploads_dir, id, request, true).await?),
    ))
}

#[cfg(test)]
#[path = "activity_import_history_tests.rs"]
mod tests;
