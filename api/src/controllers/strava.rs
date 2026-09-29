use crate::app_error::{ApiErrorResponse, AppError};
use crate::storage::AppStorage;
use axum::extract::{Query, State};
use axum::response::Redirect;
use axum::routing::{get, post};
use axum::{Json, Router};
use bike_core::config::Config;
use bike_core::entities::strava_connections;
use bike_core::integration_events_service::{
    self as integration_events, NewIntegrationEvent, INTEGRATION_LEVEL_ERROR,
    INTEGRATION_LEVEL_INFO, INTEGRATION_PROVIDER_STRAVA,
};
use bike_core::strava;
use kaleido::auth::openapi as auth_openapi;
use kaleido::auth::UserContext;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({"authorization_url": "https://www.strava.com/oauth/authorize?client_id=12345&response_type=code"}))]
pub struct StravaAuthorizeResponse {
    #[schema(
        example = "https://www.strava.com/oauth/authorize?client_id=12345&response_type=code"
    )]
    pub authorization_url: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({"configured": true,"connected": true,"athlete_id": 12345678,"athlete_name": "Alex Rider","athlete_username": "alex-rider","scopes": ["read","activity:read_all"],"last_sync_status": "succeeded","last_sync_imported_count": 3,"last_sync_duplicate_count": 1,"last_sync_failed_count": 0,"last_sync_finished_at": "2026-09-27T12:00:00Z"}))]
pub struct StravaConnectionResponse {
    #[schema(example = true)]
    pub configured: bool,
    #[schema(example = true)]
    pub connected: bool,
    #[schema(example = 12345678)]
    pub athlete_id: Option<i64>,
    #[schema(example = "Alex Rider")]
    pub athlete_name: Option<String>,
    #[schema(example = "alex-rider")]
    pub athlete_username: Option<String>,
    pub athlete_profile_medium_url: Option<String>,
    #[schema(example = json!(["read","activity:read_all"]))]
    pub scopes: Vec<String>,
    #[schema(example = "succeeded")]
    pub last_sync_status: String,
    pub last_sync_message: Option<String>,
    pub last_sync_started_at: Option<chrono::DateTime<chrono::Utc>>,
    #[schema(example = "2026-09-27T12:00:00Z")]
    pub last_sync_finished_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_synced_activity_started_at: Option<chrono::DateTime<chrono::Utc>>,
    #[schema(example = 3)]
    pub last_sync_imported_count: i32,
    #[schema(example = 1)]
    pub last_sync_duplicate_count: i32,
    #[schema(example = 0)]
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
    path = "/strava/connect",
    responses(
        (status = 200, description = "Strava authorization URL for the authenticated user", body = StravaAuthorizeResponse, example = json!({"authorization_url": "https://www.strava.com/oauth/authorize?client_id=12345&response_type=code"})),
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
    let url = match strava::create_authorization_url_for_user(Config::get(), user.id) {
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
            return Err(error.into());
        }
    };

    Ok(Json(StravaAuthorizeResponse {
        authorization_url: url.to_string(),
    }))
}

#[utoipa::path(
    get,
    path = "/strava/connection",
    responses(
        (status = 200, description = "Current Strava connection state for the authenticated user", body = StravaConnectionResponse, example = json!({"configured": true,"connected": true,"athlete_id": 12345678,"athlete_name": "Alex Rider","athlete_username": "alex-rider","scopes": ["read","activity:read_all"],"last_sync_status": "succeeded","last_sync_imported_count": 3,"last_sync_duplicate_count": 1,"last_sync_failed_count": 0,"last_sync_finished_at": "2026-09-27T12:00:00Z"})),
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
    path = "/strava/sync",
    responses(
        (status = 200, description = "Queued a Strava sync for the authenticated user", body = StravaConnectionResponse, example = json!({"configured": true,"connected": true,"athlete_id": 12345678,"athlete_name": "Alex Rider","scopes": ["read","activity:read_all"],"last_sync_status": "queued","last_sync_imported_count": 3,"last_sync_duplicate_count": 1,"last_sync_failed_count": 0})),
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
    path = "/strava/connection",
    responses(
        (status = 200, description = "Removed the authenticated user's Strava connection", body = auth_openapi::schemas::MessageResponse, example = json!({"message": "Strava connection removed."})),
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
    path = "/strava/callback",
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

        strava::exchange_code_for_connection(&state.db, &state.tasks, code, state_token)
            .await
            .map_err(AppError::from)
    }
    .await;

    match result {
        Ok(connection) => Redirect::to(&strava::build_frontend_account_redirect(
            Config::get(),
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
                Config::get(),
                "error",
                Some(&error.message),
            ))
        }
    }
}

#[utoipa::path(
    get,
    path = "/strava/webhook",
    responses(
        (status = 200, description = "Verifies the Strava webhook subscription handshake", body = strava::StravaWebhookChallengeResponse, example = json!({"hub.challenge": "strava-verification-challenge"})),
        (status = 400, description = "Invalid handshake query", body = ApiErrorResponse),
    ),
    tag = "strava"
)]
pub async fn handle_webhook_verification(
    State(state): State<Arc<AppStorage>>,
    Query(query): Query<strava::StravaWebhookSubscriptionQuery>,
) -> Result<Json<strava::StravaWebhookChallengeResponse>, AppError> {
    match strava::verify_webhook_subscription(Config::get(), &query) {
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

            Err(error.into())
        }
    }
}

#[utoipa::path(
    post,
    path = "/strava/webhook",
    request_body(content = strava::StravaWebhookEvent, example = json!({"aspect_type": "create","event_time": 1790500000,"object_id": 1234567890,"object_type": "activity","owner_id": 12345678,"subscription_id": 12345,"updates": {}})),
    responses(
        (status = 200, description = "Accepted a Strava webhook event", body = auth_openapi::schemas::MessageResponse, example = json!({"message": "ok"})),
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
    let config = Config::get();
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
