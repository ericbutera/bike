use crate::app_error::AppError;
use crate::controllers::activity_imports::ActivityImportResponse;
use bike_core::entities::activity_imports;
use sea_orm::DatabaseConnection;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Default)]
pub struct HistoryFilter {
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

pub async fn history(
    db: &DatabaseConnection,
    user_id: i32,
    filter: HistoryFilter,
) -> Result<ImportHistoryResponse, AppError> {
    let page = filter.page.unwrap_or(1).max(1);
    let (items, total) = activity_imports::Entity::history(
        db,
        user_id,
        page,
        filter.source.as_deref().filter(|value| !value.is_empty()),
        filter.status.as_deref().filter(|value| !value.is_empty()),
        filter.archive_job_id,
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

pub async fn detail(db: &DatabaseConnection, import_id: i32) -> Result<ActivityImportResponse, AppError> {
    let import = activity_imports::Entity::find_for_administration(db, import_id)
        .await?
        .ok_or_else(|| AppError::not_found("Activity import not found"))?;
    Ok(ActivityImportResponse::from_model(import, None))
}
