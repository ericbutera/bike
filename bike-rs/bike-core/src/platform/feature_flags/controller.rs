use super::service::FeatureFlagService;
use super::traits::FeatureFlagStorage;
use crate::platform::data::pagination::PaginatedResponse;
use crate::platform::error::PlatformError;
use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct PublicFlagResponse {
    pub feature_key: String,
    pub enabled: bool,
}

pub fn routes<S>() -> Router<Arc<S>>
where
    S: FeatureFlagStorage,
{
    Router::new().route("/", get(public_flags::<S>))
}

/// Public endpoint returning all feature flags (safe subset) for clients.
#[utoipa::path(
    get,
    path = "/feature-flags",
    responses(
        (status = 200, description = "List of feature flags", body = PaginatedResponse<PublicFlagResponse>)
    ),
    tag = "flags"
)]
pub async fn public_flags<S>(
    State(state): State<Arc<S>>,
) -> Result<Json<PaginatedResponse<PublicFlagResponse>>, PlatformError>
where
    S: FeatureFlagStorage,
{
    let db = state.db();
    let flags = FeatureFlagService::list_all(db).await?;

    let data = flags
        .into_iter()
        .map(|f| PublicFlagResponse {
            feature_key: f.feature_key,
            enabled: f.enabled,
        })
        .collect::<Vec<_>>();

    // TODO: pagination query param parsing/deduplication
    let total = data.len() as i64;
    Ok(Json(PaginatedResponse::new(data, 1, total, total)))
}
