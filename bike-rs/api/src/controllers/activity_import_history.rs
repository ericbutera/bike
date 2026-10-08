use crate::activity_import_history::{
    self as service, HistoryQuery, ImportHistoryResponse, ReplayRequest, ReplayResponse,
};
use crate::app_error::{ApiErrorResponse, AppError};
use crate::storage::AppStorage;
use axum::extract::{Path, Query, State};
use axum::Json;
use bike_core::auth::UserContext;
use std::sync::Arc;

#[utoipa::path(get, path = "/activity-imports/history", params(HistoryQuery),
    responses((status = 200, body = ImportHistoryResponse), (status = 401), (status = 500, body = ApiErrorResponse)),
    tag = "activity-imports", security(("bearer_auth" = [])))]
pub async fn history(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<ImportHistoryResponse>, AppError> {
    Ok(Json(service::history(&state.db, user.id, query).await?))
}

#[utoipa::path(get, path = "/activity-imports/{id}/replay", params(("id" = i32, Path), ReplayRequest),
    responses((status = 200, body = ReplayResponse), (status = 400, body = ApiErrorResponse), (status = 404, body = ApiErrorResponse)),
    tag = "activity-imports", security(("bearer_auth" = [])))]
pub async fn replay_plan(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Path(id): Path<i32>,
    Query(request): Query<ReplayRequest>,
) -> Result<Json<ReplayResponse>, AppError> {
    Ok(Json(
        service::replay(&state.db, &state.uploads_dir, user.id, id, request, false).await?,
    ))
}

#[utoipa::path(post, path = "/activity-imports/{id}/replay", params(("id" = i32, Path)), request_body = ReplayRequest,
    responses((status = 202, body = ReplayResponse), (status = 400, body = ApiErrorResponse), (status = 404, body = ApiErrorResponse), (status = 409, body = ApiErrorResponse)),
    tag = "activity-imports", security(("bearer_auth" = [])))]
pub async fn replay(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Path(id): Path<i32>,
    Json(request): Json<ReplayRequest>,
) -> Result<(axum::http::StatusCode, Json<ReplayResponse>), AppError> {
    Ok((
        axum::http::StatusCode::ACCEPTED,
        Json(service::replay(&state.db, &state.uploads_dir, user.id, id, request, true).await?),
    ))
}
