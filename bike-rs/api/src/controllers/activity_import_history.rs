use crate::activity_import_history::{self as service, ImportHistoryResponse};
use crate::app_error::{ApiErrorResponse, AppError};
use crate::controllers::activity_imports::ActivityImportResponse;
use crate::storage::AppStorage;
use axum::extract::{Path, Query, State};
use axum::Json;
use bike_core::auth::AdminUserContext;
use serde::Deserialize;
use std::sync::Arc;
use utoipa::IntoParams;

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct HistoryQuery {
    pub user_id: Option<i32>,
    pub page: Option<u64>,
    pub source: Option<String>,
    pub status: Option<String>,
    pub archive_job_id: Option<i32>,
}

impl From<HistoryQuery> for service::HistoryFilter {
    fn from(query: HistoryQuery) -> Self {
        Self {
            page: query.page,
            source: query.source,
            status: query.status,
            archive_job_id: query.archive_job_id,
        }
    }
}

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
    let filter = service::HistoryFilter::from(query);
    Ok(Json(service::history(&state.db, owner_id, filter).await?))
}

#[utoipa::path(get, path = "/admin/activity-imports/{id}", params(("id" = i32, Path)),
    responses((status = 200, body = ActivityImportResponse), (status = 401), (status = 403), (status = 404, body = ApiErrorResponse), (status = 500, body = ApiErrorResponse)),
    tag = "admin", security(("bearer_auth" = [])))]
pub async fn detail(
    _admin: AdminUserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Path(id): Path<i32>,
) -> Result<Json<ActivityImportResponse>, AppError> {
    Ok(Json(service::detail(&state.db, id).await?))
}

#[cfg(test)]
#[path = "activity_import_history_tests.rs"]
mod tests;
