use crate::activity_details::ActivityRoutePoint;
use crate::app_error::{ApiErrorResponse, AppError};
use crate::config::Config;
use crate::entities::{garmin_iq_devices, segment_efforts, segment_user_summaries, segments};
use crate::segment_support::deserialize_segment_route_points;
use crate::storage::AppStorage;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use chrono::{DateTime, Duration, Utc};
use kaleido::auth::openapi as auth_openapi;
use kaleido::auth::UserContext;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

const DEFAULT_APPROACH_METERS_XC: f64 = 80.0;
const DEFAULT_APPROACH_METERS_DH: f64 = 120.0;
const MAX_SYNC_SEGMENTS: usize = 64;
const MAX_SYNC_ROUTE_POINTS_PER_SEGMENT: usize = 24;
const PAIRING_CODE_EXPIRY_MINUTES: i64 = 10;
const REFRESH_TOKEN_EXPIRY_DAYS: i64 = 180;
const ACCESS_TOKEN_EXPIRY_MINUTES: i64 = 60;

#[derive(Debug, Deserialize, ToSchema)]
pub struct GarminIqBeginLinkParams {
    pub install_id: String,
    pub device_name: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GarminIqBeginLinkResponse {
    pub install_id: String,
    pub pairing_code: String,
    pub expires_at: DateTime<Utc>,
    pub verification_url: String,
    pub poll_after_seconds: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GarminIqCompleteLinkRequest {
    pub pairing_code: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GarminIqCompleteLinkResponse {
    pub message: String,
    pub install_id: String,
    pub device_name: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GarminIqPollLinkParams {
    pub install_id: String,
    pub pairing_code: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GarminIqPollLinkResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token_expires_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GarminIqRefreshParams {
    pub install_id: String,
    pub refresh_token: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GarminIqRefreshResponse {
    pub access_token: String,
    pub access_token_expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GarminIqLinkedDeviceResponse {
    pub id: i32,
    pub install_id: String,
    pub device_name: Option<String>,
    pub linked_at: Option<DateTime<Utc>>,
    pub last_seen_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GarminIqSegmentSyncResponse {
    pub synced_at: DateTime<Utc>,
    pub segments: Vec<GarminIqSegmentSyncItem>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GarminIqSegmentSyncItem {
    pub id: i32,
    pub title: String,
    pub distance_meters: f64,
    pub approach_meters: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal_seconds: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr_seconds: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kom_seconds: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_attempt_seconds: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub route_points: Vec<GarminIqRoutePoint>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GarminIqRoutePoint {
    pub lat: f64,
    pub lon: f64,
    pub distance_meters: f64,
}

pub fn routes() -> Router<Arc<AppStorage>> {
    Router::new()
        .route("/link/begin", get(begin_link))
        .route("/link/reset", post(reset_link))
        .route("/link/complete", post(complete_link))
        .route("/link/poll", get(poll_link))
        .route("/auth/refresh", get(refresh_access_token))
        .route("/devices", get(list_linked_devices))
        .route("/devices/:id", delete(unlink_device))
        .route("/segments/sync", get(sync_segments))
}

#[utoipa::path(
    get,
    path = "/api/garmin-iq/link/begin",
    params(
        ("install_id" = String, Query, description = "Unique watch install identifier"),
        ("device_name" = Option<String>, Query, description = "Optional device label")
    ),
    responses(
        (status = 200, description = "Created or refreshed pairing code", body = GarminIqBeginLinkResponse),
        (status = 400, description = "Invalid link request", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "garmin-iq"
)]
pub async fn begin_link(
    Query(params): Query<GarminIqBeginLinkParams>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<GarminIqBeginLinkResponse>, AppError> {
    let install_id = normalize_install_id(&params.install_id)?;
    let pairing_code = generate_pairing_code();
    let expires_at = Utc::now() + Duration::minutes(PAIRING_CODE_EXPIRY_MINUTES);

    tracing::info!(
        %install_id,
        device_name = ?params.device_name,
        "garmin iq pairing begin requested"
    );

    let model = if let Some(existing) = garmin_iq_devices::Entity::find()
        .filter(garmin_iq_devices::Column::InstallId.eq(install_id.clone()))
        .one(&state.db)
        .await?
    {
        let mut active = existing.into_active_model();
        active.device_name = Set(params.device_name.clone());
        active.pairing_code = Set(Some(pairing_code.clone()));
        active.pairing_code_expires_at = Set(Some(expires_at));
        active.pairing_approved_at = Set(None);
        active.refresh_token_hash = Set(None);
        active.refresh_token_expires_at = Set(None);
        active.access_token_hash = Set(None);
        active.access_token_expires_at = Set(None);
        active.user_id = Set(None);
        active.revoked_at = Set(None);
        active.update(&state.db).await?
    } else {
        garmin_iq_devices::ActiveModel {
            install_id: Set(install_id.clone()),
            device_name: Set(params.device_name.clone()),
            pairing_code: Set(Some(pairing_code.clone())),
            pairing_code_expires_at: Set(Some(expires_at)),
            ..Default::default()
        }
        .insert(&state.db)
        .await?
    };

    tracing::info!(
        device_id = model.id,
        install_id = %model.install_id,
        expires_at = %expires_at,
        "garmin iq pairing code issued"
    );

    Ok(Json(GarminIqBeginLinkResponse {
        install_id: model.install_id,
        pairing_code: pairing_code.clone(),
        expires_at,
        verification_url: format!(
            "{}/account?garmin_pair={}",
            Config::get().frontend_url.trim_end_matches('/'),
            pairing_code
        ),
        poll_after_seconds: 5,
    }))
}

#[utoipa::path(
    post,
    path = "/api/garmin-iq/link/complete",
    request_body = GarminIqCompleteLinkRequest,
    responses(
        (status = 200, description = "Pairing code approved and linked to current account", body = GarminIqCompleteLinkResponse),
        (status = 400, description = "Invalid or expired pairing code", body = ApiErrorResponse),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "garmin-iq",
    security(("bearer_auth" = []))
)]
pub async fn complete_link(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Json(payload): Json<GarminIqCompleteLinkRequest>,
) -> Result<Json<GarminIqCompleteLinkResponse>, AppError> {
    let code = payload.pairing_code.trim().to_uppercase();
    if code.len() < 4 {
        return Err(AppError::validation_field(
            "pairing_code",
            "Pairing code is required",
        ));
    }

    let now = Utc::now();
    let model = garmin_iq_devices::Entity::find()
        .filter(garmin_iq_devices::Column::PairingCode.eq(code.clone()))
        .filter(garmin_iq_devices::Column::RevokedAt.is_null())
        .one(&state.db)
        .await?
        .ok_or_else(|| {
            tracing::warn!(
                user_id = user.id,
                pairing_code_len = code.len(),
                "garmin iq complete_link invalid pairing code"
            );
            AppError::bad_request("Invalid pairing code")
        })?;

    if model
        .pairing_code_expires_at
        .map(|value| value <= now)
        .unwrap_or(true)
    {
        tracing::warn!(device_id = model.id, install_id = %model.install_id, user_id = user.id, "garmin iq complete_link expired pairing code");
        return Err(AppError::bad_request("Pairing code expired"));
    }

    let mut active = model.clone().into_active_model();
    active.user_id = Set(Some(user.id));
    active.pairing_approved_at = Set(Some(now));
    active.update(&state.db).await?;

    tracing::info!(device_id = model.id, install_id = %model.install_id, user_id = user.id, "garmin iq pairing approved");

    Ok(Json(GarminIqCompleteLinkResponse {
        message: "Watch approved. Return to your Garmin device to finish linking.".to_string(),
        install_id: model.install_id,
        device_name: model.device_name,
    }))
}

#[utoipa::path(
    post,
    path = "/api/garmin-iq/link/reset",
    params(
        ("install_id" = String, Query, description = "Unique watch install identifier")
    ),
    responses(
        (status = 200, description = "Reset pairing record", body = auth_openapi::schemas::MessageResponse),
        (status = 400, description = "Invalid request", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "garmin-iq"
)]
pub async fn reset_link(
    Query(params): Query<GarminIqBeginLinkParams>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<auth_openapi::schemas::MessageResponse>, AppError> {
    let install_id = normalize_install_id(&params.install_id)?;

    tracing::info!(%install_id, "garmin iq reset requested");

    if let Some(model) = garmin_iq_devices::Entity::find()
        .filter(garmin_iq_devices::Column::InstallId.eq(install_id.clone()))
        .one(&state.db)
        .await?
    {
        let device_id = model.id;
        let mut active = model.into_active_model();
        active.revoked_at = Set(Some(Utc::now()));
        active.access_token_hash = Set(None);
        active.access_token_expires_at = Set(None);
        active.refresh_token_hash = Set(None);
        active.refresh_token_expires_at = Set(None);
        active.user_id = Set(None);
        active.pairing_code = Set(None);
        active.pairing_code_expires_at = Set(None);
        active.pairing_approved_at = Set(None);
        active.update(&state.db).await?;

        tracing::info!(device_id, install_id = %install_id, "garmin iq reset completed");
    } else {
        tracing::info!(%install_id, "garmin iq reset requested for unknown install_id");
    }

    Ok(Json(auth_openapi::schemas::MessageResponse {
        message: "Garmin IQ device reset.".to_string(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/garmin-iq/link/poll",
    params(
        ("install_id" = String, Query, description = "Unique watch install identifier"),
        ("pairing_code" = String, Query, description = "Pairing code shown on watch")
    ),
    responses(
        (status = 200, description = "Current pairing state", body = GarminIqPollLinkResponse),
        (status = 400, description = "Invalid or expired polling request", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "garmin-iq"
)]
pub async fn poll_link(
    Query(params): Query<GarminIqPollLinkParams>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<GarminIqPollLinkResponse>, AppError> {
    let install_id = normalize_install_id(&params.install_id)?;
    let code = params.pairing_code.trim().to_uppercase();

    let model = garmin_iq_devices::Entity::find()
        .filter(garmin_iq_devices::Column::InstallId.eq(install_id))
        .filter(garmin_iq_devices::Column::PairingCode.eq(code))
        .filter(garmin_iq_devices::Column::RevokedAt.is_null())
        .one(&state.db)
        .await?
        .ok_or_else(|| {
            tracing::warn!(install_id = %params.install_id, pairing_code_len = params.pairing_code.trim().len(), "garmin iq poll invalid pairing state");
            AppError::bad_request("Invalid pairing state")
        })?;

    let now = Utc::now();

    // Once pairing is approved, poll is only allowed to issue tokens once.
    // The watch may continue polling for a short time after receiving the
    // first linked response; rotating tokens on each poll invalidates the
    // credentials the watch is already using for sync.
    if model.pairing_approved_at.is_some() && model.user_id.is_some() {
        let device_id = model.id;
        let model_install_id = model.install_id.clone();
        let model_user_id = model.user_id;

        if model.refresh_token_hash.is_some() && model.access_token_hash.is_some() {
            let mut active = model.into_active_model();
            active.last_seen_at = Set(Some(now));
            active.update(&state.db).await?;

            tracing::info!(device_id, install_id = %model_install_id, user_id = ?model_user_id, "garmin iq pairing linked; tokens already issued");

            return Ok(Json(GarminIqPollLinkResponse {
                status: "linked".to_string(),
                message: Some("Device linked".to_string()),
                access_token: None,
                access_token_expires_at: None,
                refresh_token: None,
            }));
        }

        let (access_token, access_expires_at, refresh_token, refresh_expires_at) = issue_tokens();
        let mut active = model.into_active_model();
        active.refresh_token_hash = Set(Some(hash_token(&refresh_token)));
        active.refresh_token_expires_at = Set(Some(refresh_expires_at));
        active.access_token_hash = Set(Some(hash_token(&access_token)));
        active.access_token_expires_at = Set(Some(access_expires_at));
        active.last_seen_at = Set(Some(now));
        active.update(&state.db).await?;

        tracing::info!(device_id, install_id = %model_install_id, user_id = ?model_user_id, "garmin iq pairing linked and tokens issued (post-approval)");

        return Ok(Json(GarminIqPollLinkResponse {
            status: "linked".to_string(),
            message: Some("Device linked".to_string()),
            access_token: Some(access_token),
            access_token_expires_at: Some(access_expires_at),
            refresh_token: Some(refresh_token),
        }));
    }

    // If not approved yet, check whether the pairing code has expired.
    if model
        .pairing_code_expires_at
        .map(|value| value <= now)
        .unwrap_or(true)
    {
        tracing::info!(device_id = model.id, install_id = %model.install_id, "garmin iq poll expired pairing code");
        return Ok(Json(GarminIqPollLinkResponse {
            status: "expired".to_string(),
            message: Some("Pairing code expired. Start link again.".to_string()),
            access_token: None,
            access_token_expires_at: None,
            refresh_token: None,
        }));
    }

    // Still waiting for the user to approve the pairing in the account UI.
    if model.pairing_approved_at.is_none() || model.user_id.is_none() {
        return Ok(Json(GarminIqPollLinkResponse {
            status: "pending".to_string(),
            message: Some("Waiting for account approval".to_string()),
            access_token: None,
            access_token_expires_at: None,
            refresh_token: None,
        }));
    }

    Ok(Json(GarminIqPollLinkResponse {
        status: "pending".to_string(),
        message: Some("Waiting for account approval".to_string()),
        access_token: None,
        access_token_expires_at: None,
        refresh_token: None,
    }))
}

#[utoipa::path(
    get,
    path = "/api/garmin-iq/auth/refresh",
    params(
        ("install_id" = String, Query, description = "Unique watch install identifier"),
        ("refresh_token" = String, Query, description = "Refresh token issued during link")
    ),
    responses(
        (status = 200, description = "Issued short-lived access token", body = GarminIqRefreshResponse),
        (status = 401, description = "Invalid refresh credentials", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "garmin-iq"
)]
pub async fn refresh_access_token(
    Query(params): Query<GarminIqRefreshParams>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<GarminIqRefreshResponse>, AppError> {
    let install_id = normalize_install_id(&params.install_id)?;
    let refresh_hash = hash_token(&params.refresh_token);

    let model = garmin_iq_devices::Entity::find()
        .filter(garmin_iq_devices::Column::InstallId.eq(install_id))
        .filter(garmin_iq_devices::Column::RefreshTokenHash.eq(Some(refresh_hash)))
        .filter(garmin_iq_devices::Column::RevokedAt.is_null())
        .one(&state.db)
        .await?
        .ok_or_else(|| {
            tracing::warn!(install_id = %params.install_id, "garmin iq refresh invalid token");
            unauthorized("Invalid refresh token")
        })?;

    let now = Utc::now();
    if model
        .refresh_token_expires_at
        .map(|value| value <= now)
        .unwrap_or(true)
    {
        tracing::warn!(device_id = model.id, install_id = %model.install_id, "garmin iq refresh token expired");
        return Err(unauthorized("Refresh token expired"));
    }

    let access_token = format!("gqa_{}", Uuid::new_v4().simple());
    let access_expires_at = now + Duration::minutes(ACCESS_TOKEN_EXPIRY_MINUTES);

    let device_id = model.id;
    let model_install_id = model.install_id.clone();
    let mut active = model.into_active_model();
    active.access_token_hash = Set(Some(hash_token(&access_token)));
    active.access_token_expires_at = Set(Some(access_expires_at));
    active.last_seen_at = Set(Some(now));
    active.update(&state.db).await?;

    tracing::info!(device_id, install_id = %model_install_id, "garmin iq access token refreshed");

    Ok(Json(GarminIqRefreshResponse {
        access_token,
        access_token_expires_at: access_expires_at,
    }))
}

#[utoipa::path(
    get,
    path = "/api/garmin-iq/devices",
    responses(
        (status = 200, description = "Linked Garmin IQ devices for current account", body = [GarminIqLinkedDeviceResponse]),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "garmin-iq",
    security(("bearer_auth" = []))
)]
pub async fn list_linked_devices(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<Vec<GarminIqLinkedDeviceResponse>>, AppError> {
    let models = garmin_iq_devices::Entity::find()
        .filter(garmin_iq_devices::Column::UserId.eq(Some(user.id)))
        .filter(garmin_iq_devices::Column::RevokedAt.is_null())
        .order_by_desc(garmin_iq_devices::Column::UpdatedAt)
        .all(&state.db)
        .await?;

    Ok(Json(
        models
            .into_iter()
            .map(|model| GarminIqLinkedDeviceResponse {
                id: model.id,
                install_id: model.install_id,
                device_name: model.device_name,
                linked_at: model.pairing_approved_at,
                last_seen_at: model.last_seen_at,
            })
            .collect(),
    ))
}

#[utoipa::path(
    delete,
    path = "/api/garmin-iq/devices/{id}",
    params(("id" = i32, Path, description = "Linked Garmin IQ device row id")),
    responses(
        (status = 200, description = "Unlinked Garmin IQ device", body = auth_openapi::schemas::MessageResponse),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Device not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "garmin-iq",
    security(("bearer_auth" = []))
)]
pub async fn unlink_device(
    Path(id): Path<i32>,
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<auth_openapi::schemas::MessageResponse>, AppError> {
    let model = garmin_iq_devices::Entity::find()
        .filter(garmin_iq_devices::Column::Id.eq(id))
        .filter(garmin_iq_devices::Column::UserId.eq(Some(user.id)))
        .filter(garmin_iq_devices::Column::RevokedAt.is_null())
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Garmin IQ device not found"))?;

    let mut active = model.into_active_model();
    active.revoked_at = Set(Some(Utc::now()));
    active.access_token_hash = Set(None);
    active.access_token_expires_at = Set(None);
    active.refresh_token_hash = Set(None);
    active.refresh_token_expires_at = Set(None);
    active.update(&state.db).await?;

    Ok(Json(auth_openapi::schemas::MessageResponse {
        message: "Garmin IQ device unlinked.".to_string(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/garmin-iq/segments/sync",
    responses(
        (status = 200, description = "Segment sync payload for Garmin IQ watch clients", body = GarminIqSegmentSyncResponse),
        (status = 401, description = "Missing or invalid Garmin IQ access token", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "garmin-iq",
    security(("bearer_auth" = []))
)]
pub async fn sync_segments(
    headers: HeaderMap,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<GarminIqSegmentSyncResponse>, AppError> {
    let raw_token =
        extract_bearer_token(&headers).ok_or_else(|| unauthorized("Missing bearer token"))?;
    let token_hash = hash_token(&raw_token);
    let now = Utc::now();

    let device = garmin_iq_devices::Entity::find()
        .filter(garmin_iq_devices::Column::AccessTokenHash.eq(Some(token_hash)))
        .filter(garmin_iq_devices::Column::RevokedAt.is_null())
        .one(&state.db)
        .await?
        .ok_or_else(|| unauthorized("Invalid access token"))?;

    if device
        .access_token_expires_at
        .map(|value| value <= now)
        .unwrap_or(true)
    {
        return Err(unauthorized("Access token expired"));
    }

    let user_id = device
        .user_id
        .ok_or_else(|| unauthorized("Device not linked to user"))?;

    let mut active_device = device.into_active_model();
    active_device.last_seen_at = Set(Some(now));
    active_device.last_sync_at = Set(Some(now));
    active_device.update(&state.db).await?;

    let mut segment_models = segments::Entity::find()
        .filter(segments::Column::UserId.eq(user_id))
        .all(&state.db)
        .await?;

    segment_models.sort_by(|left, right| {
        right
            .starred
            .cmp(&left.starred)
            .then_with(|| right.created_at.cmp(&left.created_at))
    });
    segment_models.truncate(MAX_SYNC_SEGMENTS);

    let segment_ids = segment_models
        .iter()
        .map(|segment| segment.id)
        .collect::<Vec<_>>();
    let user_summaries = segment_user_summaries::Entity::find()
        .filter(segment_user_summaries::Column::UserId.eq(user_id))
        .filter(segment_user_summaries::Column::SegmentId.is_in(segment_ids.iter().copied()))
        .all(&state.db)
        .await?;

    let mut pr_by_segment_id = HashMap::<i32, i32>::new();
    for summary in user_summaries {
        if let Some(value) = summary.personal_best_duration_seconds {
            pr_by_segment_id.insert(summary.segment_id, value);
        }
    }

    // Compute KOM (best overall duration) per segment by scanning efforts ordered
    // by duration ascending and taking the first encountered per segment.
    let kom_efforts = segment_efforts::Entity::find()
        .filter(segment_efforts::Column::SegmentId.is_in(segment_ids.iter().copied()))
        .order_by_asc(segment_efforts::Column::DurationSeconds)
        .all(&state.db)
        .await?;

    let mut kom_by_segment_id = HashMap::<i32, i32>::new();
    for effort in kom_efforts {
        if effort.duration_seconds > 0 && !kom_by_segment_id.contains_key(&effort.segment_id) {
            kom_by_segment_id.insert(effort.segment_id, effort.duration_seconds);
        }
    }

    // Compute the most recent attempt for the current user per segment.
    let recent_efforts = segment_efforts::Entity::find()
        .filter(segment_efforts::Column::SegmentId.is_in(segment_ids.iter().copied()))
        .filter(segment_efforts::Column::UserId.eq(user_id))
        .order_by_desc(segment_efforts::Column::CreatedAt)
        .all(&state.db)
        .await?;

    let mut last_attempt_by_segment_id = HashMap::<i32, (i32, DateTime<Utc>)>::new();
    for effort in recent_efforts {
        last_attempt_by_segment_id
            .entry(effort.segment_id)
            .or_insert((effort.duration_seconds, effort.created_at));
    }

    let mut items = Vec::new();

    for segment in segment_models {
        let route_points = deserialize_segment_route_points(segment.route_data_json.as_ref());
        if route_points.len() < 2 {
            continue;
        }

        let mapped_route_points = map_route_points_for_sync(&route_points);
        let distance_meters = segment
            .distance_meters
            .or_else(|| {
                mapped_route_points
                    .last()
                    .map(|point| point.distance_meters)
            })
            .unwrap_or(0.0);

        if distance_meters <= 0.0 {
            continue;
        }

        let approach_meters = if segment.mode == "dh" {
            DEFAULT_APPROACH_METERS_DH
        } else {
            DEFAULT_APPROACH_METERS_XC
        };

        items.push(GarminIqSegmentSyncItem {
            id: segment.id,
            title: segment.title,
            distance_meters,
            approach_meters,
            goal_seconds: None,
            pr_seconds: pr_by_segment_id.get(&segment.id).copied(),
            kom_seconds: kom_by_segment_id.get(&segment.id).copied(),
            last_attempt_seconds: last_attempt_by_segment_id.get(&segment.id).map(|v| v.0),
            last_attempt_at: last_attempt_by_segment_id.get(&segment.id).map(|v| v.1),
            route_points: mapped_route_points,
        });
    }

    Ok(Json(GarminIqSegmentSyncResponse {
        synced_at: Utc::now(),
        segments: items,
    }))
}

fn map_route_points_for_sync(route_points: &[ActivityRoutePoint]) -> Vec<GarminIqRoutePoint> {
    if route_points.len() <= MAX_SYNC_ROUTE_POINTS_PER_SEGMENT {
        return route_points
            .iter()
            .map(|point| GarminIqRoutePoint {
                lat: point.latitude,
                lon: point.longitude,
                distance_meters: point.distance_meters.unwrap_or_default(),
            })
            .collect();
    }

    let last_index = route_points.len() - 1;
    let denominator = MAX_SYNC_ROUTE_POINTS_PER_SEGMENT - 1;

    (0..MAX_SYNC_ROUTE_POINTS_PER_SEGMENT)
        .map(|sample_index| {
            let source_index = sample_index * last_index / denominator;
            let point = &route_points[source_index];

            GarminIqRoutePoint {
                lat: point.latitude,
                lon: point.longitude,
                distance_meters: point.distance_meters.unwrap_or_default(),
            }
        })
        .collect()
}

fn normalize_install_id(raw: &str) -> Result<String, AppError> {
    let value = raw.trim();
    if value.len() < 8 || value.len() > 128 {
        return Err(AppError::validation_field(
            "install_id",
            "install_id must be between 8 and 128 characters",
        ));
    }

    Ok(value.to_string())
}

fn generate_pairing_code() -> String {
    Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(6)
        .collect::<String>()
        .to_uppercase()
}

fn issue_tokens() -> (String, DateTime<Utc>, String, DateTime<Utc>) {
    let now = Utc::now();
    let refresh_token = format!("gqr_{}", Uuid::new_v4().simple());
    let access_token = format!("gqa_{}", Uuid::new_v4().simple());
    let access_expires_at = now + Duration::minutes(ACCESS_TOKEN_EXPIRY_MINUTES);
    let refresh_expires_at = now + Duration::days(REFRESH_TOKEN_EXPIRY_DAYS);
    (
        access_token,
        access_expires_at,
        refresh_token,
        refresh_expires_at,
    )
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let prefix = "Bearer ";
    if !value.starts_with(prefix) {
        return None;
    }

    let token = value[prefix.len()..].trim();
    if token.is_empty() {
        return None;
    }

    Some(token.to_string())
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

fn unauthorized(message: impl Into<String>) -> AppError {
    AppError {
        status: StatusCode::UNAUTHORIZED,
        message: message.into(),
        errors: None,
    }
}
