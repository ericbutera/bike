use crate::app_error::{ApiErrorResponse, AppError};
use crate::entities::user_preferences;
use crate::storage::AppStorage;
use crate::training_profile::{
    deserialize_heart_rate_zone_bounds, serialize_heart_rate_zone_bounds,
    validate_estimated_ftp_watts, validate_heart_rate_zone_bounds_bpm,
};
use axum::extract::State;
use axum::Json;
use chrono::NaiveDate;
use kaleido::auth::UserContext;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

const DEFAULT_UNIT_SYSTEM: &str = "mixed";

#[derive(Debug, Serialize, ToSchema)]
pub struct UserPreferencesResponse {
    pub unit_system: String,
    pub estimated_ftp_watts: Option<i32>,
    pub heart_rate_zone_bounds_bpm: Option<Vec<i32>>,
    pub xc_goal_start_date: Option<String>,
    pub xc_goal_target_date: Option<String>,
    pub xc_goal_target_distance_meters: Option<f64>,
    pub xc_goal_target_elevation_gain_meters: Option<f64>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateUserPreferencesRequest {
    pub unit_system: String,
    pub estimated_ftp_watts: Option<i32>,
    pub heart_rate_zone_bounds_bpm: Option<Vec<i32>>,
    pub xc_goal_start_date: Option<String>,
    pub xc_goal_target_date: Option<String>,
    pub xc_goal_target_distance_meters: Option<f64>,
    pub xc_goal_target_elevation_gain_meters: Option<f64>,
}

#[utoipa::path(
    get,
    path = "/api/preferences",
    responses(
        (status = 200, description = "User preferences for the authenticated Bike account", body = UserPreferencesResponse),
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
    let model = user_preferences::Entity::find()
        .filter(user_preferences::Column::UserId.eq(user.id))
        .one(&state.db)
        .await?;

    Ok(Json(response_from_model(model.as_ref())))
}

#[utoipa::path(
    put,
    path = "/api/preferences",
    request_body = UpdateUserPreferencesRequest,
    responses(
        (status = 200, description = "Updated Bike user preferences", body = UserPreferencesResponse),
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
    let unit_system = validate_unit_system(&payload.unit_system)?;
    let estimated_ftp_watts = validate_estimated_ftp_watts(payload.estimated_ftp_watts)?;
    let heart_rate_zone_bounds_bpm =
        validate_heart_rate_zone_bounds_bpm(payload.heart_rate_zone_bounds_bpm)?;
    let xc_goal = validate_xc_goal(
        payload.xc_goal_start_date.as_deref(),
        payload.xc_goal_target_date.as_deref(),
        payload.xc_goal_target_distance_meters,
        payload.xc_goal_target_elevation_gain_meters,
    )?;
    let heart_rate_zone_bounds_json =
        serialize_heart_rate_zone_bounds(heart_rate_zone_bounds_bpm.as_deref())?;

    let model = if let Some(existing) = user_preferences::Entity::find()
        .filter(user_preferences::Column::UserId.eq(user.id))
        .one(&state.db)
        .await?
    {
        let mut active_model: user_preferences::ActiveModel = existing.into();
        active_model.unit_system = Set(unit_system.clone());
        active_model.estimated_ftp_watts = Set(estimated_ftp_watts);
        active_model.heart_rate_zone_bounds_json = Set(heart_rate_zone_bounds_json.clone());
        active_model.xc_goal_start_date = Set(xc_goal.start_date);
        active_model.xc_goal_target_date = Set(xc_goal.target_date);
        active_model.xc_goal_target_distance_meters = Set(xc_goal.target_distance_meters);
        active_model.xc_goal_target_elevation_gain_meters =
            Set(xc_goal.target_elevation_gain_meters);
        active_model.update(&state.db).await?
    } else {
        user_preferences::ActiveModel {
            user_id: Set(user.id),
            unit_system: Set(unit_system.clone()),
            estimated_ftp_watts: Set(estimated_ftp_watts),
            heart_rate_zone_bounds_json: Set(heart_rate_zone_bounds_json),
            xc_goal_start_date: Set(xc_goal.start_date),
            xc_goal_target_date: Set(xc_goal.target_date),
            xc_goal_target_distance_meters: Set(xc_goal.target_distance_meters),
            xc_goal_target_elevation_gain_meters: Set(xc_goal.target_elevation_gain_meters),
            ..Default::default()
        }
        .insert(&state.db)
        .await?
    };

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
            .map(|preferences| preferences.xc_goal_start_date)
            .flatten()
            .map(|value| value.format("%Y-%m-%d").to_string()),
        xc_goal_target_date: model
            .map(|preferences| preferences.xc_goal_target_date)
            .flatten()
            .map(|value| value.format("%Y-%m-%d").to_string()),
        xc_goal_target_distance_meters: model
            .and_then(|preferences| preferences.xc_goal_target_distance_meters),
        xc_goal_target_elevation_gain_meters: model
            .and_then(|preferences| preferences.xc_goal_target_elevation_gain_meters),
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct XcGoalFields {
    start_date: Option<NaiveDate>,
    target_date: Option<NaiveDate>,
    target_distance_meters: Option<f64>,
    target_elevation_gain_meters: Option<f64>,
}

fn validate_unit_system(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();

    match normalized.as_str() {
        "metric" | "imperial" | "mixed" => Ok(normalized),
        _ => Err(AppError::validation_field(
            "unit_system",
            "Unit system must be metric, imperial, or mixed",
        )),
    }
}

fn validate_xc_goal(
    start_date: Option<&str>,
    target_date: Option<&str>,
    target_distance_meters: Option<f64>,
    target_elevation_gain_meters: Option<f64>,
) -> Result<XcGoalFields, AppError> {
    let parsed_start_date = start_date
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| parse_goal_date("xc_goal_start_date", "XC goal start date", value))
        .transpose()?;
    let parsed_target_date = target_date
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| parse_goal_date("xc_goal_target_date", "XC goal target date", value))
        .transpose()?;
    let target_distance_meters =
        validate_positive_metric("xc_goal_target_distance_meters", target_distance_meters)?;
    let target_elevation_gain_meters = validate_positive_metric(
        "xc_goal_target_elevation_gain_meters",
        target_elevation_gain_meters,
    )?;

    let field_count = [
        parsed_start_date.is_some(),
        parsed_target_date.is_some(),
        target_distance_meters.is_some(),
        target_elevation_gain_meters.is_some(),
    ]
    .into_iter()
    .filter(|value| *value)
    .count();

    if field_count == 0 {
        return Ok(XcGoalFields::default());
    }

    if field_count != 4 {
        return Err(AppError::validation_field(
            "xc_goal_start_date",
            "XC goal requires a training start date, target date, target distance, and target climbing gain",
        ));
    }

    if parsed_start_date > parsed_target_date {
        return Err(AppError::validation_field(
            "xc_goal_start_date",
            "XC goal start date must be on or before the target date",
        ));
    }

    Ok(XcGoalFields {
        start_date: parsed_start_date,
        target_date: parsed_target_date,
        target_distance_meters,
        target_elevation_gain_meters,
    })
}

fn validate_positive_metric(field: &str, value: Option<f64>) -> Result<Option<f64>, AppError> {
    match value {
        Some(next) if !next.is_finite() || next <= 0.0 => Err(AppError::validation_field(
            field,
            "Value must be greater than zero",
        )),
        _ => Ok(value),
    }
}

fn parse_goal_date(field: &str, label: &str, value: &str) -> Result<NaiveDate, AppError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        AppError::validation_field(field, &format!("{label} must use YYYY-MM-DD format"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use chrono::Utc;

    #[test]
    fn validate_unit_system_accepts_supported_values() {
        assert_eq!(validate_unit_system("metric").unwrap(), "metric");
        assert_eq!(validate_unit_system("imperial").unwrap(), "imperial");
        assert_eq!(validate_unit_system(" mixed ").unwrap(), "mixed");
    }

    #[test]
    fn validate_unit_system_rejects_unknown_values() {
        let error = validate_unit_system("nautical").unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(
            error.message,
            "Unit system must be metric, imperial, or mixed"
        );
    }

    #[test]
    fn response_from_model_reads_training_profile_fields() {
        let now = Utc::now();
        let response = response_from_model(Some(&user_preferences::Model {
            id: 1,
            user_id: 3,
            unit_system: "mixed".to_string(),
            estimated_ftp_watts: Some(265),
            heart_rate_zone_bounds_json: Some(crate::training_profile::StoredHeartRateZoneBounds(
                vec![120, 140, 155, 170],
            )),
            xc_goal_start_date: Some(NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()),
            xc_goal_target_date: Some(NaiveDate::from_ymd_opt(2026, 9, 20).unwrap()),
            xc_goal_target_distance_meters: Some(160_934.4),
            xc_goal_target_elevation_gain_meters: Some(3_962.4),
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
    }

    #[test]
    fn validate_xc_goal_requires_complete_payload() {
        let error =
            validate_xc_goal(Some("2026-06-01"), Some("2026-09-20"), Some(100_000.0), None)
                .unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(
            error.message,
            "XC goal requires a training start date, target date, target distance, and target climbing gain"
        );
    }

    #[test]
    fn validate_xc_goal_rejects_invalid_date_format() {
        let error = validate_xc_goal(
            Some("2026-06-01"),
            Some("09/20/2026"),
            Some(100_000.0),
            Some(1_000.0),
        )
        .unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(
            error.message,
            "XC goal target date must use YYYY-MM-DD format"
        );
    }

    #[test]
    fn validate_xc_goal_rejects_start_date_after_target_date() {
        let error = validate_xc_goal(
            Some("2026-09-21"),
            Some("2026-09-20"),
            Some(100_000.0),
            Some(1_000.0),
        )
        .unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(
            error.message,
            "XC goal start date must be on or before the target date"
        );
    }
}
