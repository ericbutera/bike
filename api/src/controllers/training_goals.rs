use crate::app_error::{ApiErrorResponse, AppError};
use crate::storage::AppStorage;
use axum::extract::State;
use axum::Json;
use bike_core::services::training_goals::*;
use chrono::Utc;
use kaleido::auth::UserContext;
use std::sync::Arc;

#[utoipa::path(
    get,
    path = "/api/training/xc-progress",
    responses(
        (status = 200, description = "XC goals and progress summary for the authenticated user", body = XcGoalProgressResponse),
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
    path = "/api/training/dh-progress",
    responses(
        (status = 200, description = "DH goals and progress summary for the authenticated user", body = DhGoalProgressResponse),
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
