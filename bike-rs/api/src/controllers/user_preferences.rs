use crate::app_error::{ApiErrorResponse, AppError};
use crate::storage::AppStorage;
use axum::extract::State;
use axum::Json;
use bike_core::auth::UserContext;
use bike_core::entities::user_preferences;
use bike_core::training_profile::deserialize_heart_rate_zone_bounds;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;

const DEFAULT_UNIT_SYSTEM: &str = "imperial";
#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({"unit_system": "metric","estimated_ftp_watts": 245,"heart_rate_zone_bounds_bpm": [110,130,150,170],"xc_goal_event_name": "Autumn 100 km","xc_goal_event_profile": "xc_marathon","xc_goal_start_date": "2026-09-01","xc_goal_target_date": "2026-10-31","xc_goal_target_distance_meters": 100000.0,"xc_goal_target_elevation_gain_meters": 1500.0,"xc_goal_target_finish_time_seconds": 21600}))]
pub struct UserPreferencesResponse {
    #[schema(example = "metric")]
    pub unit_system: String,
    #[schema(example = 245)]
    pub estimated_ftp_watts: Option<i32>,
    #[schema(example = json!([110,130,150,170]))]
    pub heart_rate_zone_bounds_bpm: Option<Vec<i32>>,
    #[schema(example = "2026-09-01")]
    pub xc_goal_start_date: Option<String>,
    #[schema(example = "2026-10-31")]
    pub xc_goal_target_date: Option<String>,
    #[schema(example = 100000.0)]
    pub xc_goal_target_distance_meters: Option<f64>,
    #[schema(example = 1500.0)]
    pub xc_goal_target_elevation_gain_meters: Option<f64>,
    #[schema(example = "Autumn 100 km")]
    pub xc_goal_event_name: Option<String>,
    #[schema(example = 21600)]
    pub xc_goal_target_finish_time_seconds: Option<i32>,
    #[schema(example = "xc_marathon")]
    pub xc_goal_event_profile: Option<String>,
    pub xc_goal_backfill_status: Option<String>,
    pub xc_goal_backfill_completed_at: Option<DateTime<Utc>>,
}

pub use bike_core::entities::user_preferences::PreferencesUpdate as UpdateUserPreferencesRequest;

#[utoipa::path(
    get,
    path = "/preferences",
    responses(
        (status = 200, description = "User preferences for the authenticated Bike account", body = UserPreferencesResponse, example = json!({"unit_system": "metric","estimated_ftp_watts": 245,"heart_rate_zone_bounds_bpm": [110,130,150,170],"xc_goal_event_name": "Autumn 100 km","xc_goal_event_profile": "xc_marathon","xc_goal_start_date": "2026-09-01","xc_goal_target_date": "2026-10-31","xc_goal_target_distance_meters": 100000.0,"xc_goal_target_elevation_gain_meters": 1500.0,"xc_goal_target_finish_time_seconds": 21600})),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "preferences",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_preferences(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<UserPreferencesResponse>, AppError> {
    let model = user_preferences::Model::for_user(&state.db, user.id).await?;

    Ok(Json(response_from_model(model.as_ref())))
}

#[utoipa::path(
    put,
    path = "/preferences",
    request_body(content = UpdateUserPreferencesRequest, example = json!({"unit_system": "metric","estimated_ftp_watts": 245,"heart_rate_zone_bounds_bpm": [110,130,150,170],"xc_goal_event_name": "Autumn 100 km","xc_goal_event_profile": "xc_marathon","xc_goal_start_date": "2026-09-01","xc_goal_target_date": "2026-10-31","xc_goal_target_distance_meters": 100000.0,"xc_goal_target_elevation_gain_meters": 1500.0,"xc_goal_target_finish_time_seconds": 21600})),
    responses(
        (status = 200, description = "Updated Bike user preferences", body = UserPreferencesResponse, example = json!({"unit_system": "metric","estimated_ftp_watts": 245,"heart_rate_zone_bounds_bpm": [110,130,150,170],"xc_goal_event_name": "Autumn 100 km","xc_goal_event_profile": "xc_marathon","xc_goal_start_date": "2026-09-01","xc_goal_target_date": "2026-10-31","xc_goal_target_distance_meters": 100000.0,"xc_goal_target_elevation_gain_meters": 1500.0,"xc_goal_target_finish_time_seconds": 21600})),
        (status = 400, description = "Invalid preferences payload", body = ApiErrorResponse),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "preferences",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn update_preferences(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Json(payload): Json<UpdateUserPreferencesRequest>,
) -> Result<Json<UserPreferencesResponse>, AppError> {
    let model = bike_core::services::preferences::update_preferences(
        &state.db,
        &state.tasks,
        user.id,
        payload,
    )
    .await?;
    Ok(Json(response_from_model(Some(&model))))
}

fn response_from_model(model: Option<&user_preferences::Model>) -> UserPreferencesResponse {
    UserPreferencesResponse {
        unit_system: model
            .map(|preferences| preferences.unit_system.clone())
            .unwrap_or_else(|| DEFAULT_UNIT_SYSTEM.to_string()),
        estimated_ftp_watts: model.and_then(|preferences| preferences.estimated_ftp_watts),
        heart_rate_zone_bounds_bpm: model.and_then(|preferences| {
            deserialize_heart_rate_zone_bounds(preferences.heart_rate_zone_bounds_json.as_ref())
        }),
        xc_goal_start_date: model
            .and_then(|preferences| preferences.xc_goal_start_date)
            .map(|value| value.format("%Y-%m-%d").to_string()),
        xc_goal_target_date: model
            .and_then(|preferences| preferences.xc_goal_target_date)
            .map(|value| value.format("%Y-%m-%d").to_string()),
        xc_goal_target_distance_meters: model
            .and_then(|preferences| preferences.xc_goal_target_distance_meters),
        xc_goal_target_elevation_gain_meters: model
            .and_then(|preferences| preferences.xc_goal_target_elevation_gain_meters),
        xc_goal_event_name: model.and_then(|preferences| preferences.xc_goal_event_name.clone()),
        xc_goal_target_finish_time_seconds: model
            .and_then(|preferences| preferences.xc_goal_target_finish_time_seconds),
        xc_goal_event_profile: model
            .and_then(|preferences| preferences.xc_goal_event_profile.clone()),
        xc_goal_backfill_status: model
            .and_then(|preferences| preferences.xc_goal_backfill_status.clone()),
        xc_goal_backfill_completed_at: model
            .and_then(|preferences| preferences.xc_goal_backfill_completed_at),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use chrono::Utc;

    #[test]
    fn response_from_model_reads_training_profile_fields() {
        let now = Utc::now();
        let response = response_from_model(Some(&user_preferences::Model {
            id: 1,
            user_id: 3,
            unit_system: "mixed".to_string(),
            estimated_ftp_watts: Some(265),
            heart_rate_zone_bounds_json: Some(
                bike_core::training_profile::StoredHeartRateZoneBounds(vec![120, 140, 155, 170]),
            ),
            xc_goal_start_date: Some(NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()),
            xc_goal_target_date: Some(NaiveDate::from_ymd_opt(2026, 9, 20).unwrap()),
            xc_goal_target_distance_meters: Some(160_934.4),
            xc_goal_target_elevation_gain_meters: Some(3_962.4),
            xc_goal_event_name: Some("Marji Gesick MG100".to_string()),
            xc_goal_target_finish_time_seconds: Some(43_200),
            xc_goal_event_profile: Some("technical_singletrack".to_string()),
            xc_goal_backfill_status: Some("completed".to_string()),
            xc_goal_backfill_completed_at: Some(now),
            created_at: now,
            updated_at: now,
        }));

        assert_eq!(response.estimated_ftp_watts, Some(265));
        assert_eq!(
            response.heart_rate_zone_bounds_bpm,
            Some(vec![120, 140, 155, 170])
        );
        assert_eq!(response.xc_goal_start_date.as_deref(), Some("2026-06-01"));
        assert_eq!(response.xc_goal_target_date.as_deref(), Some("2026-09-20"));
        assert_eq!(response.xc_goal_target_distance_meters, Some(160_934.4));
        assert_eq!(response.xc_goal_target_elevation_gain_meters, Some(3_962.4));
        assert_eq!(
            response.xc_goal_event_name.as_deref(),
            Some("Marji Gesick MG100")
        );
        assert_eq!(response.xc_goal_target_finish_time_seconds, Some(43_200));
        assert_eq!(
            response.xc_goal_event_profile.as_deref(),
            Some("technical_singletrack")
        );
        assert_eq!(
            response.xc_goal_backfill_status.as_deref(),
            Some("completed")
        );
        assert_eq!(response.xc_goal_backfill_completed_at, Some(now));
    }
}
