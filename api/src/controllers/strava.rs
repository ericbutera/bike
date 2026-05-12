use crate::app_error::{ApiErrorResponse, AppError};
use crate::entities::strava_connections;
use crate::storage::AppStorage;
use crate::strava;
use axum::extract::{Query, State};
use axum::response::Redirect;
use axum::routing::{get, post};
use axum::{Json, Router};
use kaleido::auth::openapi as auth_openapi;
use kaleido::auth::UserContext;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct StravaAuthorizeResponse {
    pub authorization_url: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct StravaConnectionResponse {
    pub configured: bool,
    pub connected: bool,
    pub athlete_id: Option<i64>,
    pub athlete_name: Option<String>,
    pub athlete_username: Option<String>,
    pub athlete_profile_medium_url: Option<String>,
    pub scopes: Vec<String>,
    pub last_sync_status: String,
    pub last_sync_message: Option<String>,
    pub last_sync_started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_sync_finished_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_synced_activity_started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_sync_imported_count: i32,
    pub last_sync_duplicate_count: i32,
    pub last_sync_failed_count: i32,
}

#[derive(Debug, Deserialize)]
pub struct StravaCallbackQuery {
    pub code: Option<String>,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

pub fn routes() -> Router<Arc<AppStorage>> {
    Router::new()
        .route("/connect", post(begin_connect))
        .route(
            "/connection",
            get(get_connection).delete(disconnect_connection),
        )
        .route("/sync", post(queue_sync))
        .route("/callback", get(handle_callback))
}

#[utoipa::path(
    post,
    path = "/api/strava/connect",
    responses(
        (status = 200, description = "Strava authorization URL for the authenticated user", body = StravaAuthorizeResponse),
        (status = 400, description = "Strava integration is not configured", body = ApiErrorResponse),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "strava",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn begin_connect(
    UserContext { user, .. }: UserContext<AppStorage>,
) -> Result<Json<StravaAuthorizeResponse>, AppError> {
    let url = strava::create_authorization_url_for_user(crate::config::Config::get(), user.id)?;

    Ok(Json(StravaAuthorizeResponse {
        authorization_url: url.to_string(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/strava/connection",
    responses(
        (status = 200, description = "Current Strava connection state for the authenticated user", body = StravaConnectionResponse),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "strava",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_connection(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<StravaConnectionResponse>, AppError> {
    let connection = strava::load_connection(&state.db, user.id).await?;

    Ok(Json(
        response_from_model(&state.db, connection.as_ref()).await?,
    ))
}

#[utoipa::path(
    post,
    path = "/api/strava/sync",
    responses(
        (status = 200, description = "Queued a Strava sync for the authenticated user", body = StravaConnectionResponse),
        (status = 400, description = "Strava integration is not configured", body = ApiErrorResponse),
        (status = 409, description = "Another activity import is already running or queued", body = ApiErrorResponse),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "No Strava connection exists", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "strava",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn queue_sync(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<StravaConnectionResponse>, AppError> {
    let connection = strava::queue_connection_sync(&state.db, &state.tasks, user.id).await?;

    Ok(Json(
        response_from_model(&state.db, Some(&connection)).await?,
    ))
}

#[utoipa::path(
    delete,
    path = "/api/strava/connection",
    responses(
        (status = 200, description = "Removed the authenticated user's Strava connection", body = auth_openapi::schemas::MessageResponse),
        (status = 409, description = "The user's Strava sync is queued or running", body = ApiErrorResponse),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "strava",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn disconnect_connection(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<auth_openapi::schemas::MessageResponse>, AppError> {
    strava::disconnect_connection(&state.db, user.id).await?;

    Ok(Json(auth_openapi::schemas::MessageResponse {
        message: "Strava connection removed.".to_string(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/strava/callback",
    responses(
        (status = 303, description = "Redirects the browser back to the account page after handling the Strava callback"),
    ),
    tag = "strava"
)]
pub async fn handle_callback(
    State(state): State<Arc<AppStorage>>,
    Query(query): Query<StravaCallbackQuery>,
) -> Redirect {
    let result = async {
        if let Some(error) = query.error.as_deref() {
            return Err(AppError::bad_request(format!(
                "Strava authorization was not completed: {error}"
            )));
        }

        let code = query
            .code
            .as_deref()
            .ok_or_else(|| AppError::bad_request("Missing Strava authorization code"))?;
        let state_token = query
            .state
            .as_deref()
            .ok_or_else(|| AppError::bad_request("Missing Strava authorization state"))?;

        strava::exchange_code_for_connection(&state.db, &state.tasks, code, state_token).await
    }
    .await;

    match result {
        Ok(connection) => Redirect::to(&strava::build_frontend_account_redirect(
            crate::config::Config::get(),
            "connected",
            connection.last_sync_message.as_deref(),
        )),
        Err(error) => Redirect::to(&strava::build_frontend_account_redirect(
            crate::config::Config::get(),
            "error",
            Some(&error.message),
        )),
    }
}

async fn response_from_model(
    _db: &DatabaseConnection,
    model: Option<&strava_connections::Model>,
) -> Result<StravaConnectionResponse, AppError> {
    let config = crate::config::Config::get();

    Ok(StravaConnectionResponse {
        configured: config.strava_enabled(),
        connected: model.is_some(),
        athlete_id: model.map(|connection| connection.athlete_id),
        athlete_name: model.and_then(strava::athlete_display_name),
        athlete_username: model.and_then(|connection| connection.athlete_username.clone()),
        athlete_profile_medium_url: model
            .and_then(|connection| connection.athlete_profile_medium_url.clone()),
        scopes: model
            .map(|connection| strava::parse_scope_list(&connection.scopes))
            .unwrap_or_default(),
        last_sync_status: model
            .map(|connection| connection.last_sync_status.clone())
            .unwrap_or_else(|| strava::STRAVA_SYNC_STATUS_NEVER.to_string()),
        last_sync_message: model.and_then(|connection| connection.last_sync_message.clone()),
        last_sync_started_at: model.and_then(|connection| connection.last_sync_started_at),
        last_sync_finished_at: model.and_then(|connection| connection.last_sync_finished_at),
        last_synced_activity_started_at: model
            .and_then(|connection| connection.last_synced_activity_started_at),
        last_sync_imported_count: model
            .map(|connection| connection.last_sync_imported_count)
            .unwrap_or_default(),
        last_sync_duplicate_count: model
            .map(|connection| connection.last_sync_duplicate_count)
            .unwrap_or_default(),
        last_sync_failed_count: model
            .map(|connection| connection.last_sync_failed_count)
            .unwrap_or_default(),
    })
}
