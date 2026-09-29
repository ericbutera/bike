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
    path = "/training/reports/definitions",
    responses(
        (status = 200, description = "Training report definitions", body = TrainingReportDefinitionsResponse, example = json!({"reports": [{"id": "ride_summary","display_name": "Ride summary","short_purpose": "Summarize distance, time, and climbing","supported_filters": ["min_distance"],"required_data_quality": ["distance"],"result_sections": ["ride_summary"],"metrics": [{"key": "distance_meters","label": "Distance","direction": "higher","unit": "m"}]}]})),
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
    path = "/training/reports",
    params(TrainingReportsQuery),
    responses(
        (status = 200, description = "Training reports over a selected boundary for the authenticated user", body = TrainingReportsResponse, example = json!({"generated_at": "2026-09-27T12:00:00Z","boundary": "week","range_start": "2026-09-20","range_end": "2026-09-27","points": [{"bucket_start": "2026-09-20","bucket_end": "2026-09-27","distance_meters": 75000.0,"distance_miles": 46.6,"z1_seconds": 1200,"z2_seconds": 6000,"z3_seconds": 900,"z4_seconds": 300,"z5_seconds": 0,"elevation_gain_meters": 900.0,"elevation_gain_feet": 2952.8,"activity_type_times": [{"activity_type": "training","seconds": 8400}]}]})),
        (status = 400, description = "Invalid query parameters", body = ApiErrorResponse),
        (status = 401, description = "Not authenticated"),
        (status = 429, description = "A report is already generating", body = ApiErrorResponse, example = json!({"message": "A report is already generating. Stay on that report until it finishes before starting another one. Try again in 30 seconds.", "retry_at": "2026-09-27T12:00:30Z"})),
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
