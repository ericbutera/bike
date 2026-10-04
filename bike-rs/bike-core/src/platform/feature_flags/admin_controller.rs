use super::service::FeatureFlagService;
use super::traits::FeatureFlagStorage;
use crate::auth::extractors::{AdminUserContext, AuthStorage};
use crate::platform::data::pagination::PaginatedResponse;
use crate::platform::error::PlatformError;
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct FeatureFlagResponse {
    pub feature_key: String,
    pub enabled: bool,
    pub description: Option<String>,
}

#[derive(Deserialize, ToSchema)]
#[schema(example = json!({ "enabled": true }))]
pub struct UpdateFlagRequest {
    pub enabled: bool,
}

pub fn routes<S>() -> Router<Arc<S>>
where
    S: FeatureFlagStorage + AuthStorage,
{
    Router::new()
        .route("/", get(list_flags::<S>))
        .route("/:key", post(update_flag::<S>))
}

/// List all feature flags
#[utoipa::path(
    get,
    path = "/admin/feature-flags",
    responses(
        (status = 200, description = "List of feature flags", body = PaginatedResponse<FeatureFlagResponse>)
    ),
    tag = "admin",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_flags<S>(
    _admin: AdminUserContext<S>,
    State(state): State<Arc<S>>,
) -> Result<Json<PaginatedResponse<FeatureFlagResponse>>, PlatformError>
where
    S: FeatureFlagStorage + AuthStorage,
{
    let db = FeatureFlagStorage::db(state.as_ref());
    let flags = FeatureFlagService::list_all(db).await?;

    let data = flags
        .into_iter()
        .map(|f| FeatureFlagResponse {
            feature_key: f.feature_key,
            enabled: f.enabled,
            description: f.description,
        })
        .collect::<Vec<_>>();

    let total = data.len() as i64;
    Ok(Json(PaginatedResponse::new(data, 1, total, total)))
}

/// Update a single feature flag
#[utoipa::path(
    post,
    path = "/admin/feature-flags/{key}",
    params(
        ("key" = String, Path, description = "Feature flag key")
    ),
    request_body = UpdateFlagRequest,
    responses(
        (status = 200, description = "Flag updated", body = FeatureFlagResponse)
    ),
    tag = "admin",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn update_flag<S>(
    _admin: AdminUserContext<S>,
    State(state): State<Arc<S>>,
    axum::extract::Path(key): axum::extract::Path<String>,
    Json(payload): Json<UpdateFlagRequest>,
) -> Result<Json<FeatureFlagResponse>, PlatformError>
where
    S: FeatureFlagStorage + AuthStorage,
{
    let db = FeatureFlagStorage::db(state.as_ref());
    let flag = state
        .feature_flag_service()
        .update_flag(db, &key, payload.enabled)
        .await?;

    Ok(Json(FeatureFlagResponse {
        feature_key: flag.feature_key,
        enabled: flag.enabled,
        description: flag.description,
    }))
}
