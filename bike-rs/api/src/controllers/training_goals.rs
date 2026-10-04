use crate::app_error::{ApiErrorResponse, AppError};
use crate::storage::AppStorage;
use axum::extract::State;
use axum::Json;
use bike_core::auth::UserContext;
use bike_core::services::training_goals::*;
use chrono::Utc;
use std::sync::Arc;

#[utoipa::path(
    get,
    path = "/training/xc-progress",
    responses(
        (status = 200, description = "XC goals and progress summary for the authenticated user", body = XcGoalProgressResponse, example = json!({"generated_at": "2026-09-27T12:00:00Z","summary": {"recent_window_days": 28,"recent_ride_count": 8,"comparable_ride_count": 5,"total_z2_time_seconds": 18000,"total_climbing_time_seconds": 4200,"total_climbing_elevation_gain_meters": 1200.0},"deficits": [],"race_results": [],"goals": [],"recommendations": [],"weekly_progress": [],"recent_rides": []})),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "training",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_xc_goal_progress(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<XcGoalProgressResponse>, AppError> {
    Ok(Json(
        TrainingGoalsService::xc_goal_progress(&state.db, user.id, Utc::now()).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/training/dh-progress",
    responses(
        (status = 200, description = "DH goals and progress summary for the authenticated user", body = DhGoalProgressResponse, example = json!({"generated_at": "2026-09-27T12:00:00Z","summary": {"segment_count": 2,"session_count": 4,"effort_count": 18,"average_efforts_per_session": 4.5},"goals": [],"recommendations": [],"segments": [],"recent_sessions": []})),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "training",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_dh_goal_progress(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<DhGoalProgressResponse>, AppError> {
    Ok(Json(
        TrainingGoalsService::dh_goal_progress(&state.db, user.id, Utc::now()).await?,
    ))
}
