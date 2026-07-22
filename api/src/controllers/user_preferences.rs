use crate::app_error::{ApiErrorResponse, AppError};
use crate::entities::user_preferences;
use crate::storage::AppStorage;
use crate::training_profile::{
    deserialize_heart_rate_zone_bounds, serialize_heart_rate_zone_bounds,
    validate_estimated_ftp_watts, validate_heart_rate_zone_bounds_bpm,
};
use crate::xc_goal_backfill::{clear_user_xc_goal_backfill_state, queue_user_xc_goal_backfill};
use axum::extract::State;
use axum::Json;
use chrono::{DateTime, NaiveDate, Utc};
use kaleido::auth::UserContext;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

const DEFAULT_UNIT_SYSTEM: &str = "imperial";
const XC_GOAL_EVENT_NAME_MAX_LENGTH: usize = 120;
const XC_GOAL_EVENT_PROFILES: &[&str] = &[
    "xc_marathon",
    "technical_singletrack",
    "endurance_mtb",
    "ultra_mtb",
    "custom",
];

#[derive(Debug, Serialize, ToSchema)]
pub struct UserPreferencesResponse {
    pub unit_system: String,
    pub estimated_ftp_watts: Option<i32>,
    pub heart_rate_zone_bounds_bpm: Option<Vec<i32>>,
    pub xc_goal_start_date: Option<String>,
    pub xc_goal_target_date: Option<String>,
    pub xc_goal_target_distance_meters: Option<f64>,
    pub xc_goal_target_elevation_gain_meters: Option<f64>,
    pub xc_goal_event_name: Option<String>,
    pub xc_goal_target_finish_time_seconds: Option<i32>,
    pub xc_goal_event_profile: Option<String>,
    pub xc_goal_backfill_status: Option<String>,
    pub xc_goal_backfill_completed_at: Option<DateTime<Utc>>,
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
    pub xc_goal_event_name: Option<String>,
    pub xc_goal_target_finish_time_seconds: Option<i32>,
    pub xc_goal_event_profile: Option<String>,
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
        payload.xc_goal_event_name.as_deref(),
        payload.xc_goal_target_finish_time_seconds,
        payload.xc_goal_event_profile.as_deref(),
    )?;
    let heart_rate_zone_bounds_json =
        serialize_heart_rate_zone_bounds(heart_rate_zone_bounds_bpm.as_deref())?;

    let existing_model = user_preferences::Entity::find()
        .filter(user_preferences::Column::UserId.eq(user.id))
        .one(&state.db)
        .await?;
    let previous_start_date = existing_model
        .as_ref()
        .and_then(|model| model.xc_goal_start_date);
    let should_request_xc_goal_backfill =
        xc_goal.start_date.is_some() && previous_start_date != xc_goal.start_date;

    let model = if let Some(existing) = existing_model {
        let mut active_model: user_preferences::ActiveModel = existing.into();
        active_model.unit_system = Set(unit_system.clone());
        active_model.estimated_ftp_watts = Set(estimated_ftp_watts);
        active_model.heart_rate_zone_bounds_json = Set(heart_rate_zone_bounds_json.clone());
        active_model.xc_goal_start_date = Set(xc_goal.start_date);
        active_model.xc_goal_target_date = Set(xc_goal.target_date);
        active_model.xc_goal_target_distance_meters = Set(xc_goal.target_distance_meters);
        active_model.xc_goal_target_elevation_gain_meters =
            Set(xc_goal.target_elevation_gain_meters);
        active_model.xc_goal_event_name = Set(xc_goal.event_name.clone());
        active_model.xc_goal_target_finish_time_seconds = Set(xc_goal.target_finish_time_seconds);
        active_model.xc_goal_event_profile = Set(xc_goal.event_profile.clone());
        if xc_goal.start_date.is_none() {
            active_model.xc_goal_backfill_status = Set(None);
            active_model.xc_goal_backfill_completed_at = Set(None);
        }
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
            xc_goal_event_name: Set(xc_goal.event_name.clone()),
            xc_goal_target_finish_time_seconds: Set(xc_goal.target_finish_time_seconds),
            xc_goal_event_profile: Set(xc_goal.event_profile.clone()),
            xc_goal_backfill_status: Set(None),
            xc_goal_backfill_completed_at: Set(None),
            ..Default::default()
        }
        .insert(&state.db)
        .await?
    };

    if should_request_xc_goal_backfill {
        queue_user_xc_goal_backfill(&state.db, &state.tasks, user.id).await?;
    } else if xc_goal.start_date.is_none() {
        clear_user_xc_goal_backfill_state(&state.db, user.id).await?;
    }

    let latest_model = user_preferences::Entity::find()
        .filter(user_preferences::Column::UserId.eq(user.id))
        .one(&state.db)
        .await?
        .unwrap_or(model);

    Ok(Json(response_from_model(Some(&latest_model))))
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

#[derive(Debug, Clone, Default)]
struct XcGoalFields {
    start_date: Option<NaiveDate>,
    target_date: Option<NaiveDate>,
    target_distance_meters: Option<f64>,
    target_elevation_gain_meters: Option<f64>,
    event_name: Option<String>,
    target_finish_time_seconds: Option<i32>,
    event_profile: Option<String>,
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
    event_name: Option<&str>,
    target_finish_time_seconds: Option<i32>,
    event_profile: Option<&str>,
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
    let event_name = validate_optional_event_name(event_name)?;
    let target_finish_time_seconds =
        validate_target_finish_time_seconds(target_finish_time_seconds)?;
    let event_profile = validate_event_profile(event_profile)?;

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
        if event_name.is_some() || target_finish_time_seconds.is_some() || event_profile.is_some() {
            return Err(AppError::validation_field(
                "xc_goal_start_date",
                "XC goal event details require a complete event target",
            ));
        }

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
        event_name,
        target_finish_time_seconds,
        event_profile,
    })
}

fn validate_optional_event_name(value: Option<&str>) -> Result<Option<String>, AppError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    if value.chars().count() > XC_GOAL_EVENT_NAME_MAX_LENGTH {
        return Err(AppError::validation_field(
            "xc_goal_event_name",
            "XC goal event name must be 120 characters or fewer",
        ));
    }

    Ok(Some(value.to_string()))
}

fn validate_target_finish_time_seconds(value: Option<i32>) -> Result<Option<i32>, AppError> {
    match value {
        Some(seconds) if seconds <= 0 => Err(AppError::validation_field(
            "xc_goal_target_finish_time_seconds",
            "Target finish time must be greater than zero",
        )),
        _ => Ok(value),
    }
}

fn validate_event_profile(value: Option<&str>) -> Result<Option<String>, AppError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let normalized = value.to_ascii_lowercase();

    if !XC_GOAL_EVENT_PROFILES.contains(&normalized.as_str()) {
        return Err(AppError::validation_field(
            "xc_goal_event_profile",
            "XC goal event profile is not supported",
        ));
    }

    Ok(Some(normalized))
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

    #[test]
    fn validate_xc_goal_requires_complete_payload() {
        let error = validate_xc_goal(
            Some("2026-06-01"),
            Some("2026-09-20"),
            Some(100_000.0),
            None,
            None,
            None,
            None,
        )
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
            None,
            None,
            None,
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
            None,
            None,
            None,
        )
        .unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(
            error.message,
            "XC goal start date must be on or before the target date"
        );
    }

    #[test]
    fn validate_xc_goal_accepts_optional_event_details() {
        let goal = validate_xc_goal(
            Some("2026-06-01"),
            Some("2026-09-20"),
            Some(160_934.4),
            Some(3_962.4),
            Some(" Marji Gesick MG100 "),
            Some(43_200),
            Some("TECHNICAL_SINGLETRACK"),
        )
        .unwrap();

        assert_eq!(goal.event_name.as_deref(), Some("Marji Gesick MG100"));
        assert_eq!(goal.target_finish_time_seconds, Some(43_200));
        assert_eq!(goal.event_profile.as_deref(), Some("technical_singletrack"));
    }

    #[test]
    fn validate_xc_goal_rejects_event_details_without_complete_target() {
        let error = validate_xc_goal(
            None,
            None,
            None,
            None,
            Some("Lumberjack 100"),
            Some(36_000),
            None,
        )
        .unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(
            error.message,
            "XC goal event details require a complete event target"
        );
    }
}
