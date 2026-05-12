use crate::app_error::{ApiErrorResponse, AppError};
use crate::entities::user_preferences;
use crate::storage::AppStorage;
use crate::training_profile::{
    deserialize_heart_rate_zone_bounds, serialize_heart_rate_zone_bounds,
    validate_estimated_ftp_watts, validate_heart_rate_zone_bounds_bpm,
};
use axum::extract::State;
use axum::Json;
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
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateUserPreferencesRequest {
    pub unit_system: String,
    pub estimated_ftp_watts: Option<i32>,
    pub heart_rate_zone_bounds_bpm: Option<Vec<i32>>,
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
        active_model.update(&state.db).await?
    } else {
        user_preferences::ActiveModel {
            user_id: Set(user.id),
            unit_system: Set(unit_system.clone()),
            estimated_ftp_watts: Set(estimated_ftp_watts),
            heart_rate_zone_bounds_json: Set(heart_rate_zone_bounds_json),
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
    }
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
            heart_rate_zone_bounds_json: Some(
                crate::training_profile::StoredHeartRateZoneBounds(vec![120, 140, 155, 170]),
            ),
            created_at: now,
            updated_at: now,
        }));

        assert_eq!(response.estimated_ftp_watts, Some(265));
        assert_eq!(
            response.heart_rate_zone_bounds_bpm,
            Some(vec![120, 140, 155, 170])
        );
    }
}
