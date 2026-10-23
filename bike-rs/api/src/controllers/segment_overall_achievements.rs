use crate::app_error::{ApiErrorResponse, AppError};
use crate::storage::AppStorage;
use axum::extract::{Path, State};
use axum::Json;
use bike_core::auth::entities::users;
use bike_core::auth::UserContext;
use bike_core::entities::{segment_overall_achievements, segments};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct OverallAchievementResponse {
    /// Current global all-time position, with one athlete per position.
    #[schema(minimum = 1, maximum = 10, example = 1)]
    pub position: i32,
    #[schema(example = 7)]
    pub user_id: i32,
    pub rider_name: Option<String>,
    #[schema(example = 90)]
    pub effort_id: i32,
    #[schema(example = 355)]
    pub duration_seconds: i32,
}

impl OverallAchievementResponse {
    fn from_saved(saved: segment_overall_achievements::Model, names: &HashMap<i32, String>) -> Self {
        Self {
            position: saved.position,
            user_id: saved.user_id,
            rider_name: names.get(&saved.user_id).cloned(),
            effort_id: saved.effort_id,
            duration_seconds: saved.duration_seconds,
        }
    }
}

#[utoipa::path(
    get,
    path = "/segments/{id}/overall-achievements",
    params(("id" = i32, Path, description = "Segment ID", example = 7)),
    responses(
        (status = 200, description = "Saved current global all-time top ten, one position per athlete", body = [OverallAchievementResponse]),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Segment not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "segments",
    security(("bearer_auth" = []))
)]
pub async fn get_overall_achievements(
    Path(id): Path<i32>,
    _user: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<Vec<OverallAchievementResponse>>, AppError> {
    segments::Model::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::not_found("Segment not found"))?;
    let saved = segment_overall_achievements::Model::for_segment(&state.db, id).await?;
    if saved.is_empty() {
        return Ok(Json(Vec::new()));
    }
    let user_ids = saved.iter().map(|row| row.user_id).collect::<Vec<_>>();
    let names = users::Model::rider_names(&state.db, &user_ids)
        .await?
        .into_iter()
        .map(|rider| (rider.id, rider.name))
        .collect();
    Ok(Json(
        saved
            .into_iter()
            .map(|row| OverallAchievementResponse::from_saved(row, &names))
            .collect(),
    ))
}

#[cfg(test)]
mod tests;
