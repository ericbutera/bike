use crate::{app_error::AppError, storage::AppStorage};
use axum::{extract::State, Json};
use bike_core::auth::UserContext;
use bike_core::synthetics::{self, ScenarioManifest};
use std::sync::Arc;

pub async fn scenario(
    State(state): State<Arc<AppStorage>>,
    UserContext { user, .. }: UserContext<AppStorage>,
) -> Result<Json<ScenarioManifest>, AppError> {
    if !user.is_synthetic() {
        return Err(AppError::forbidden("Synthetic identity required"));
    }
    Ok(Json(synthetics::manifest(&state.db, user.id).await?))
}
