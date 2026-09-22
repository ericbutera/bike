use crate::app_error::{ApiErrorResponse, AppError};
use crate::storage::AppStorage;
use axum::extract::{Query, State};
use axum::Json;
use bike_core::services::cooldown::{CooldownService, CooldownType};
use bike_core::services::reports::*;
use chrono::Utc;
use kaleido::auth::UserContext;
use std::sync::Arc;

#[utoipa::path(
    get,
    path = "/api/training/reports/definitions",
    responses(
        (status = 200, description = "Training report definitions", body = TrainingReportDefinitionsResponse),
        (status = 401, description = "Not authenticated"),
    ),
    tag = "training",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_training_report_definitions(
    UserContext { .. }: UserContext<AppStorage>,
) -> Json<TrainingReportDefinitionsResponse> {
    Json(ReportsService::definitions())
}

#[utoipa::path(
    get,
    path = "/api/training/reports",
    params(TrainingReportsQuery),
    responses(
        (status = 200, description = "Training reports over a selected boundary for the authenticated user", body = TrainingReportsResponse),
        (status = 400, description = "Invalid query parameters", body = ApiErrorResponse),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "training",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_training_reports(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Query(query): Query<TrainingReportsQuery>,
) -> Result<Json<TrainingReportsResponse>, AppError> {
    let prepared_request =
        ReportsService::prepare_training_report_request(PreparedTrainingReportRequest {
            user_id: user.id,
            db: state.db.clone(),
            query,
            now: Utc::now(),
        })?;

    let mut cooldown =
        CooldownService::acquire(&state.db, CooldownType::ReportGeneration, user.id).await?;
    let response = ReportsService::build_training_report(prepared_request).await;
    if let Err(error) = cooldown.release().await {
        tracing::error!(error = ?error, "failed to release report generation cooldown");
        if response.is_ok() {
            return Err(error.into());
        }
    }

    response.map(Json).map_err(AppError::from)
}
