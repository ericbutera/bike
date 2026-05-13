use crate::app_error::{ApiErrorResponse, AppError};
use crate::entities::strava_connections;
use crate::integration_events::{
    self, NewIntegrationEvent, INTEGRATION_LEVEL_ERROR, INTEGRATION_LEVEL_INFO,
    INTEGRATION_PROVIDER_STRAVA,
};
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
        .route(
            "/webhook",
            get(handle_webhook_verification).post(handle_webhook_event),
        )
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
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<StravaAuthorizeResponse>, AppError> {
    let url = match strava::create_authorization_url_for_user(crate::config::Config::get(), user.id)
    {
        Ok(url) => {
            record_strava_event_best_effort(
                &state.db,
                Some(user.id),
                "oauth.connect_started",
                INTEGRATION_LEVEL_INFO,
                "Started Strava OAuth connect flow.",
                None,
            )
            .await;
            url
        }
        Err(error) => {
            record_strava_event_best_effort(
                &state.db,
                Some(user.id),
                "oauth.connect_failed",
                INTEGRATION_LEVEL_ERROR,
                error.message.clone(),
                Some(serde_json::json!({
                    "stage": "begin_connect",
                })),
            )
            .await;
            return Err(error);
        }
    };

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
        Err(error) => {
            if query.error.is_some() || query.code.is_none() || query.state.is_none() {
                record_strava_event_best_effort(
                    &state.db,
                    None,
                    "oauth.connect_failed",
                    INTEGRATION_LEVEL_ERROR,
                    error.message.clone(),
                    Some(serde_json::json!({
                        "stage": "callback_prevalidation",
                        "query_error": query.error,
                        "has_code": query.code.is_some(),
                        "has_state": query.state.is_some(),
                    })),
                )
                .await;
            }

            Redirect::to(&strava::build_frontend_account_redirect(
                crate::config::Config::get(),
                "error",
                Some(&error.message),
            ))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/strava/webhook",
    responses(
        (status = 200, description = "Verifies the Strava webhook subscription handshake", body = strava::StravaWebhookChallengeResponse),
        (status = 400, description = "Invalid handshake query", body = ApiErrorResponse),
    ),
    tag = "strava"
)]
pub async fn handle_webhook_verification(
    State(state): State<Arc<AppStorage>>,
    Query(query): Query<strava::StravaWebhookSubscriptionQuery>,
) -> Result<Json<strava::StravaWebhookChallengeResponse>, AppError> {
    match strava::verify_webhook_subscription(crate::config::Config::get(), &query) {
        Ok(response) => {
            record_strava_event_best_effort(
                &state.db,
                None,
                "webhook.verification_succeeded",
                INTEGRATION_LEVEL_INFO,
                "Verified Strava webhook handshake.",
                Some(serde_json::json!({
                    "mode": query.mode,
                    "has_challenge": query.challenge.is_some(),
                })),
            )
            .await;

            Ok(Json(response))
        }
        Err(error) => {
            record_strava_event_best_effort(
                &state.db,
                None,
                "webhook.verification_failed",
                INTEGRATION_LEVEL_ERROR,
                error.message.clone(),
                Some(serde_json::json!({
                    "mode": query.mode,
                    "has_challenge": query.challenge.is_some(),
                    "has_verify_token": query.verify_token.is_some(),
                })),
            )
            .await;

            Err(error)
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/strava/webhook",
    request_body = strava::StravaWebhookEvent,
    responses(
        (status = 200, description = "Accepted a Strava webhook event"),
    ),
    tag = "strava"
)]
pub async fn handle_webhook_event(
    State(state): State<Arc<AppStorage>>,
    Json(event): Json<strava::StravaWebhookEvent>,
) -> Result<Json<auth_openapi::schemas::MessageResponse>, AppError> {
    strava::handle_webhook_event(&state.db, &state.tasks, &event).await?;

    Ok(Json(auth_openapi::schemas::MessageResponse {
        message: "ok".to_string(),
    }))
}

async fn response_from_model(
    db: &DatabaseConnection,
    model: Option<&strava_connections::Model>,
) -> Result<StravaConnectionResponse, AppError> {
    let config = crate::config::Config::get();
    let resolved = match model {
        Some(connection) => Some(strava::resolve_connection_sync_state(db, connection).await?),
        None => None,
    };
    let connection = resolved.as_ref().map(|state| &state.connection);

    Ok(StravaConnectionResponse {
        configured: config.strava_enabled(),
        connected: connection.is_some(),
        athlete_id: connection.map(|connection| connection.athlete_id),
        athlete_name: connection.and_then(strava::athlete_display_name),
        athlete_username: connection.and_then(|connection| connection.athlete_username.clone()),
        athlete_profile_medium_url: connection
            .and_then(|connection| connection.athlete_profile_medium_url.clone()),
        scopes: connection
            .map(|connection| strava::parse_scope_list(&connection.scopes))
            .unwrap_or_default(),
        last_sync_status: connection
            .map(|connection| connection.last_sync_status.clone())
            .unwrap_or_else(|| strava::STRAVA_SYNC_STATUS_NEVER.to_string()),
        last_sync_message: connection.and_then(|connection| connection.last_sync_message.clone()),
        last_sync_started_at: connection.and_then(|connection| connection.last_sync_started_at),
        last_sync_finished_at: connection.and_then(|connection| connection.last_sync_finished_at),
        last_synced_activity_started_at: connection
            .and_then(|connection| connection.last_synced_activity_started_at),
        last_sync_imported_count: connection
            .map(|connection| connection.last_sync_imported_count)
            .unwrap_or_default(),
        last_sync_duplicate_count: connection
            .map(|connection| connection.last_sync_duplicate_count)
            .unwrap_or_default(),
        last_sync_failed_count: connection
            .map(|connection| connection.last_sync_failed_count)
            .unwrap_or_default(),
    })
}

async fn record_strava_event_best_effort(
    db: &DatabaseConnection,
    user_id: Option<i32>,
    event_type: &str,
    level: &str,
    message: impl Into<String>,
    payload: Option<serde_json::Value>,
) {
    let message = message.into();

    if let Err(error) = integration_events::record_event(
        db,
        NewIntegrationEvent {
            user_id,
            provider: INTEGRATION_PROVIDER_STRAVA.to_string(),
            event_type: event_type.to_string(),
            level: level.to_string(),
            message: message.clone(),
            connection_id: None,
            payload,
        },
    )
    .await
    {
        tracing::warn!(
            event_type,
            user_id,
            message = %error.message,
            log_message = %message,
            "failed to persist controller-level Strava integration event"
        );
    }
}
