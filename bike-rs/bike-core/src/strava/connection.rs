use crate::activity_import_lock::{
    load_user_activity_import_lock, release_user_activity_import_lock,
    ACTIVITY_IMPORT_LOCK_SOURCE_STRAVA_SYNC,
};
use crate::activity_import_pipeline::ActivityUploadPayload;
use crate::background_jobs::background_tasks;
use crate::config::Config;
use crate::entities::strava_connections;
use crate::integration_events_service::{
    self, NewIntegrationEvent, INTEGRATION_LEVEL_ERROR, INTEGRATION_LEVEL_INFO,
    INTEGRATION_LEVEL_SUCCESS, INTEGRATION_LEVEL_WARNING, INTEGRATION_PROVIDER_STRAVA,
};
use crate::jobs::StravaSyncTask;
use crate::strava::client::{StravaApiClient, StravaAuthorizationTokenResponse};
use crate::strava::provider_payload::{
    StoredStravaProviderPayload, StravaActivityStreams, StravaActivitySummary,
};
use crate::workflow_error::WorkflowError as AppError;
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use reqwest::Url;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use utoipa::ToSchema;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

pub const STRAVA_AUTHORIZE_URL: &str = "https://www.strava.com/oauth/authorize";
pub const STRAVA_SYNC_STATUS_NEVER: &str = "never";
pub const STRAVA_SYNC_STATUS_QUEUED: &str = "queued";
pub const STRAVA_SYNC_STATUS_RUNNING: &str = "running";
pub const STRAVA_SYNC_STATUS_FAILED: &str = "failed";
const STRAVA_REQUIRED_ACTIVITY_SCOPE: &str = "activity:read_all";
const STRAVA_STATE_MAX_AGE_MINUTES: i64 = 10;
const STRAVA_SYNC_TASK_TYPE: &str = "strava_sync";

#[derive(Debug, Clone)]
pub struct ResolvedStravaConnectionSyncState {
    pub connection: strava_connections::Model,
    pub active_sync_status: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedStravaState {
    pub user_id: i32,
    pub issued_at: DateTime<Utc>,
    pub nonce: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct StravaWebhookSubscriptionQuery {
    #[serde(rename = "hub.mode")]
    pub mode: Option<String>,
    #[serde(rename = "hub.challenge")]
    pub challenge: Option<String>,
    #[serde(rename = "hub.verify_token")]
    pub verify_token: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({"hub.challenge": "strava-verification-challenge"}))]
pub struct StravaWebhookChallengeResponse {
    #[serde(rename = "hub.challenge")]
    #[schema(example = "strava-verification-challenge")]
    pub challenge: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[schema(example = json!({"aspect_type": "create","event_time": 1790500000,"object_id": 1234567890,"object_type": "activity","owner_id": 12345678,"subscription_id": 12345,"updates": {}}))]
pub struct StravaWebhookEvent {
    #[schema(example = "create")]
    pub aspect_type: String,
    #[schema(example = 1790500000)]
    pub event_time: i64,
    #[schema(example = 1234567890)]
    pub object_id: i64,
    #[schema(example = "activity")]
    pub object_type: String,
    #[schema(example = 12345678)]
    pub owner_id: i64,
    #[schema(example = 12345)]
    pub subscription_id: i64,
    #[schema(example = json!({}))]
    pub updates: Option<serde_json::Value>,
}

pub fn build_redirect_uri(config: &Config) -> String {
    format!(
        "{}/api/strava/callback",
        config.api_url.trim_end_matches('/')
    )
}

pub fn build_authorization_url(config: &Config, state: &str) -> Result<Url, String> {
    let mut url = Url::parse(STRAVA_AUTHORIZE_URL)
        .map_err(|error| format!("Invalid Strava authorize URL: {error}"))?;
    let scopes = requested_oauth_scopes(config);

    url.query_pairs_mut()
        .append_pair("client_id", config.strava_client_id.trim())
        .append_pair("redirect_uri", &build_redirect_uri(config))
        .append_pair("response_type", "code")
        .append_pair("approval_prompt", "auto")
        .append_pair("scope", &scopes.join(","))
        .append_pair("state", state);

    Ok(url)
}

pub fn create_authorization_url_for_user(config: &Config, user_id: i32) -> Result<Url, AppError> {
    ensure_strava_configured(config)?;
    let state = create_state_token(config, user_id).map_err(|message| {
        AppError::internal(format!(
            "Failed to initialize Strava connect flow: {message}"
        ))
    })?;

    build_authorization_url(config, &state).map_err(AppError::internal)
}

pub fn create_state_token(config: &Config, user_id: i32) -> Result<String, String> {
    let issued_at = Utc::now();
    let nonce = Uuid::new_v4().simple().to_string();
    let payload = format!(
        "{user_id}:{issued_at}:{nonce}",
        issued_at = issued_at.timestamp()
    );
    let signature = sign_state(config, &payload)?;

    Ok(format!("{payload}:{signature}"))
}

pub fn verify_state_token(config: &Config, token: &str) -> Result<VerifiedStravaState, String> {
    let mut parts = token.split(':');
    let user_id = parts
        .next()
        .ok_or_else(|| "Missing state user id".to_string())?
        .parse::<i32>()
        .map_err(|_| "Invalid state user id".to_string())?;
    let issued_at = parts
        .next()
        .ok_or_else(|| "Missing state timestamp".to_string())?
        .parse::<i64>()
        .map_err(|_| "Invalid state timestamp".to_string())?;
    let nonce = parts
        .next()
        .ok_or_else(|| "Missing state nonce".to_string())?
        .to_string();
    let provided_signature = parts
        .next()
        .ok_or_else(|| "Missing state signature".to_string())?;

    if parts.next().is_some() {
        return Err("Invalid state token format".to_string());
    }

    let payload = format!("{user_id}:{issued_at}:{nonce}");
    let expected_signature = sign_state(config, &payload)?;
    if expected_signature != provided_signature {
        return Err("Invalid state signature".to_string());
    }

    let issued_at = DateTime::<Utc>::from_timestamp(issued_at, 0)
        .ok_or_else(|| "Invalid state timestamp".to_string())?;
    if Utc::now() > issued_at + Duration::minutes(STRAVA_STATE_MAX_AGE_MINUTES) {
        return Err("Strava connect session expired. Try again.".to_string());
    }

    Ok(VerifiedStravaState {
        user_id,
        issued_at,
        nonce,
    })
}

pub fn scopes_allow_activity_import(scopes: &str) -> bool {
    parse_scope_list(scopes)
        .iter()
        .any(|scope| scope == STRAVA_REQUIRED_ACTIVITY_SCOPE)
}

pub fn parse_scope_list(raw: &str) -> Vec<String> {
    raw.split([' ', ','])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub fn athlete_display_name(connection: &strava_connections::Model) -> Option<String> {
    let full_name = [
        connection.athlete_first_name.as_deref(),
        connection.athlete_last_name.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>()
    .join(" ");

    if !full_name.is_empty() {
        Some(full_name)
    } else {
        connection.athlete_username.clone()
    }
}

pub fn build_frontend_account_redirect(
    config: &Config,
    status: &str,
    message: Option<&str>,
) -> String {
    let fallback = format!("{}/account", config.frontend_url.trim_end_matches('/'));
    let mut url = match Url::parse(&fallback) {
        Ok(value) => value,
        Err(_) => return fallback,
    };

    url.query_pairs_mut().append_pair("strava", status);
    if let Some(message) = message.filter(|value| !value.trim().is_empty()) {
        url.query_pairs_mut().append_pair("strava_message", message);
    }

    url.to_string()
}

pub async fn load_connection(
    db: &DatabaseConnection,
    user_id: i32,
) -> Result<Option<strava_connections::Model>, AppError> {
    strava_connections::Entity::find()
        .filter(strava_connections::Column::UserId.eq(user_id))
        .one(db)
        .await
        .map_err(AppError::from)
}

pub async fn exchange_code_for_connection(
    db: &DatabaseConnection,
    code: &str,
    state_token: &str,
) -> Result<strava_connections::Model, AppError> {
    let config = Config::get();
    ensure_strava_configured(config)?;

    let verified_state = match verify_state_token(config, state_token) {
        Ok(state) => state,
        Err(message) => {
            record_strava_event_best_effort(
                db,
                None,
                None,
                "oauth.connect_failed",
                INTEGRATION_LEVEL_ERROR,
                message.clone(),
                Some(serde_json::json!({
                    "stage": "verify_state",
                })),
            )
            .await;
            return Err(AppError::bad_request(message));
        }
    };

    let result = async {
        let client = StravaApiClient::new(db, config)?;
        let token_response = client.exchange_authorization_code(code).await?;

        if !scopes_allow_activity_import(&token_response.scope) {
            return Err(AppError::bad_request(missing_activity_scope_message()));
        }

        let connection =
            upsert_connection_from_token(db, verified_state.user_id, token_response).await?;

        crate::strava::workflow::queue_sync(db, &connection, false).await?;
        load_connection(db, connection.user_id)
            .await?
            .ok_or_else(|| AppError::not_found("Strava connection was removed"))
    }
    .await;

    match result {
        Ok(connection) => {
            record_connection_strava_event_best_effort(
                db,
                &connection,
                "oauth.connected",
                INTEGRATION_LEVEL_SUCCESS,
                "Strava connection established.",
                Some(serde_json::json!({
                    "athlete_id": connection.athlete_id,
                    "scopes": parse_scope_list(&connection.scopes),
                })),
            )
            .await;
            Ok(connection)
        }
        Err(error) => {
            record_strava_event_best_effort(
                db,
                Some(verified_state.user_id),
                None,
                "oauth.connect_failed",
                INTEGRATION_LEVEL_ERROR,
                error.message.clone(),
                Some(serde_json::json!({
                    "stage": "callback",
                })),
            )
            .await;
            Err(error)
        }
    }
}

pub fn verify_webhook_subscription(
    config: &Config,
    query: &StravaWebhookSubscriptionQuery,
) -> Result<StravaWebhookChallengeResponse, AppError> {
    ensure_strava_webhook_configured(config)?;

    if query.mode.as_deref() != Some("subscribe") {
        return Err(AppError::bad_request("Invalid Strava webhook mode"));
    }

    let challenge = query
        .challenge
        .as_deref()
        .ok_or_else(|| AppError::bad_request("Missing Strava webhook challenge"))?;
    let verify_token = query
        .verify_token
        .as_deref()
        .ok_or_else(|| AppError::bad_request("Missing Strava webhook verify token"))?;

    if verify_token != config.strava_webhook_verify_token {
        return Err(AppError::bad_request("Invalid Strava webhook verify token"));
    }

    Ok(StravaWebhookChallengeResponse {
        challenge: challenge.to_string(),
    })
}

pub async fn forget_local_connection(
    db: &DatabaseConnection,
    user_id: i32,
) -> Result<(), AppError> {
    let Some(connection) = load_connection(db, user_id).await? else {
        return Ok(());
    };
    disconnect_connection_internal(
        db,
        &connection,
        "Removed the local Strava connection after the cleanup workflow.",
        Some(serde_json::json!({ "trigger": "workflow" })),
    )
    .await
}

fn ensure_strava_configured(config: &Config) -> Result<(), AppError> {
    if config.strava_enabled() {
        Ok(())
    } else {
        Err(AppError::bad_request(
            "Strava integration is not configured on this Bike deployment",
        ))
    }
}

fn ensure_strava_webhook_configured(config: &Config) -> Result<(), AppError> {
    if config.strava_webhook_enabled() {
        Ok(())
    } else {
        Err(AppError::bad_request(
            "Strava webhook integration is not configured on this Bike deployment",
        ))
    }
}

async fn upsert_connection_from_token(
    db: &DatabaseConnection,
    user_id: i32,
    token_response: StravaAuthorizationTokenResponse,
) -> Result<strava_connections::Model, AppError> {
    if let Some(existing_for_athlete) = strava_connections::Entity::find()
        .filter(strava_connections::Column::AthleteId.eq(token_response.athlete.id))
        .one(db)
        .await?
    {
        if existing_for_athlete.user_id != user_id {
            return Err(AppError::bad_request(
                "That Strava athlete is already connected to another Bike account",
            ));
        }
    }

    let expires_at = DateTime::<Utc>::from_timestamp(token_response.expires_at, 0)
        .ok_or_else(|| AppError::internal("Strava returned an invalid token expiration"))?;

    if let Some(existing) = load_connection(db, user_id).await? {
        let mut active_model: strava_connections::ActiveModel = existing.into();
        active_model.athlete_id = Set(token_response.athlete.id);
        active_model.athlete_username = Set(token_response.athlete.username);
        active_model.athlete_first_name = Set(token_response.athlete.firstname);
        active_model.athlete_last_name = Set(token_response.athlete.lastname);
        active_model.athlete_profile_medium_url = Set(token_response.athlete.profile_medium);
        active_model.scopes = Set(token_response.scope);
        active_model.access_token = Set(crate::strava::credentials::seal(
            token_response.access_token.as_bytes(),
        )?);
        active_model.refresh_token = Set(crate::strava::credentials::seal(
            token_response.refresh_token.as_bytes(),
        )?);
        active_model.created_at = Set(Utc::now());
        active_model.expires_at = Set(expires_at);
        active_model.update(db).await.map_err(AppError::from)
    } else {
        strava_connections::ActiveModel {
            user_id: Set(user_id),
            athlete_id: Set(token_response.athlete.id),
            athlete_username: Set(token_response.athlete.username),
            athlete_first_name: Set(token_response.athlete.firstname),
            athlete_last_name: Set(token_response.athlete.lastname),
            athlete_profile_medium_url: Set(token_response.athlete.profile_medium),
            scopes: Set(token_response.scope),
            access_token: Set(crate::strava::credentials::seal(
                token_response.access_token.as_bytes(),
            )?),
            refresh_token: Set(crate::strava::credentials::seal(
                token_response.refresh_token.as_bytes(),
            )?),
            expires_at: Set(expires_at),
            last_sync_status: Set(STRAVA_SYNC_STATUS_NEVER.to_string()),
            last_sync_imported_count: Set(0),
            last_sync_duplicate_count: Set(0),
            last_sync_failed_count: Set(0),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(AppError::from)
    }
}

pub(crate) async fn ensure_fresh_access_token(
    db: &DatabaseConnection,
    client: &StravaApiClient,
    connection: strava_connections::Model,
) -> Result<strava_connections::Model, AppError> {
    if connection.expires_at > Utc::now() + Duration::minutes(5) {
        return Ok(connection);
    }

    let refresh_token = crate::strava::credentials::open(&connection.refresh_token)?;
    let archive = crate::strava::client::RequestArchive {
        store: crate::strava::objects::ObjectStore::new(&Config::get().uploads_dir),
        key: super::paths::refresh(&connection, &refresh_token),
    };
    let token_response = archive
        .scope(client.refresh_access_token(&refresh_token))
        .await?;
    let expires_at = DateTime::<Utc>::from_timestamp(token_response.expires_at, 0)
        .ok_or_else(|| AppError::internal("Strava returned an invalid token expiration"))?;
    let mut active_model: strava_connections::ActiveModel = connection.clone().into();
    active_model.scopes = Set(token_response
        .scope
        .unwrap_or_else(|| connection.scopes.clone()));
    active_model.access_token = Set(crate::strava::credentials::seal(
        token_response.access_token.as_bytes(),
    )?);
    active_model.refresh_token = Set(crate::strava::credentials::seal(
        token_response.refresh_token.as_bytes(),
    )?);
    active_model.expires_at = Set(expires_at);

    if let Some(athlete) = token_response.athlete {
        active_model.athlete_id = Set(athlete.id);
        active_model.athlete_username = Set(athlete.username);
        active_model.athlete_first_name = Set(athlete.firstname);
        active_model.athlete_last_name = Set(athlete.lastname);
        active_model.athlete_profile_medium_url = Set(athlete.profile_medium);
    }

    active_model.update(db).await.map_err(AppError::from)
}

pub(crate) fn build_activity_upload(
    activity: &StravaActivitySummary,
    streams: &StravaActivityStreams,
) -> Result<ActivityUploadPayload, AppError> {
    let bytes = serde_json::to_vec(&StoredStravaProviderPayload::new(
        activity.clone(),
        streams.clone(),
    ))
    .map_err(|error| {
        AppError::internal(format!(
            "Failed to serialize Strava activity {}: {error}",
            activity.id
        ))
    })?;
    Ok(ActivityUploadPayload {
        original_filename: format!("strava_activity_{}.json", activity.id),
        format: "json".into(),
        mime_type: Some("application/json".into()),
        source_correlation_id: Some(activity.id.to_string()),
        bytes,
    })
}

pub async fn resolve_connection_sync_state(
    db: &DatabaseConnection,
    connection: &strava_connections::Model,
) -> Result<ResolvedStravaConnectionSyncState, AppError> {
    if let Some(run) =
        crate::entities::strava_workflows::Model::active_collection(db, connection.id).await?
    {
        return Ok(ResolvedStravaConnectionSyncState {
            connection: connection.clone(),
            active_sync_status: Some(if run.status == "running" {
                STRAVA_SYNC_STATUS_RUNNING
            } else {
                STRAVA_SYNC_STATUS_QUEUED
            }),
        });
    }
    let active_tasks = find_active_connection_sync_tasks(db, connection.id).await?;
    let active_sync_status = if active_tasks
        .iter()
        .any(|task| task.status == background_tasks::TaskStatus::Processing.as_str())
    {
        Some(STRAVA_SYNC_STATUS_RUNNING)
    } else if !active_tasks.is_empty() {
        Some(STRAVA_SYNC_STATUS_QUEUED)
    } else {
        None
    };

    if let Some(active_sync_status) = active_sync_status {
        let connection = if connection.last_sync_status != active_sync_status {
            let mut active_model: strava_connections::ActiveModel = connection.clone().into();
            active_model.last_sync_status = Set(active_sync_status.to_string());
            active_model.update(db).await.map_err(AppError::from)?
        } else {
            connection.clone()
        };

        return Ok(ResolvedStravaConnectionSyncState {
            connection,
            active_sync_status: Some(active_sync_status),
        });
    }

    let mut next_connection = connection.clone();
    if let Some(lock) = load_user_activity_import_lock(db, connection.user_id).await? {
        if lock.source == ACTIVITY_IMPORT_LOCK_SOURCE_STRAVA_SYNC {
            release_user_activity_import_lock(
                db,
                connection.user_id,
                ACTIVITY_IMPORT_LOCK_SOURCE_STRAVA_SYNC,
            )
            .await?;
        }
    }

    if matches!(
        connection.last_sync_status.as_str(),
        STRAVA_SYNC_STATUS_QUEUED | STRAVA_SYNC_STATUS_RUNNING
    ) {
        let (fallback_status, fallback_message) = if connection.last_sync_status
            == STRAVA_SYNC_STATUS_RUNNING
        {
            (
                STRAVA_SYNC_STATUS_FAILED,
                "The previous Strava sync stopped before it completed. Start another sync when ready.",
            )
        } else {
            (
                STRAVA_SYNC_STATUS_NEVER,
                "Strava sync is not currently active. Start another sync when ready.",
            )
        };
        let mut active_model: strava_connections::ActiveModel = connection.clone().into();
        active_model.last_sync_status = Set(fallback_status.to_string());
        active_model.last_sync_message = Set(Some(fallback_message.to_string()));
        next_connection = active_model.update(db).await.map_err(AppError::from)?;
    }

    Ok(ResolvedStravaConnectionSyncState {
        connection: next_connection,
        active_sync_status: None,
    })
}

async fn find_active_connection_sync_tasks(
    db: &DatabaseConnection,
    connection_id: i32,
) -> Result<Vec<background_tasks::Model>, AppError> {
    let tasks = background_tasks::Entity::find()
        .filter(background_tasks::Column::TaskType.eq(STRAVA_SYNC_TASK_TYPE))
        .filter(background_tasks::Column::Status.is_in([
            background_tasks::TaskStatus::Pending.as_str(),
            background_tasks::TaskStatus::Processing.as_str(),
        ]))
        .order_by_desc(background_tasks::Column::CreatedAt)
        .all(db)
        .await?;

    Ok(tasks
        .into_iter()
        .filter(|task| task_targets_connection(task, connection_id))
        .collect())
}

async fn cancel_pending_connection_sync_tasks(
    db: &DatabaseConnection,
    connection_id: i32,
) -> Result<usize, AppError> {
    let tasks = find_active_connection_sync_tasks(db, connection_id).await?;
    let mut cancelled = 0usize;

    for task in tasks {
        if task.status != background_tasks::TaskStatus::Pending.as_str() {
            continue;
        }

        task.mark_completed_with_result(
            db,
            Some("Cancelled because the Strava connection was disconnected.".to_string()),
        )
        .await?;
        cancelled += 1;
    }

    Ok(cancelled)
}

fn task_targets_connection(task: &background_tasks::Model, connection_id: i32) -> bool {
    serde_json::from_value::<StravaSyncTask>(
        task.payload
            .get("data")
            .cloned()
            .unwrap_or_else(|| task.payload.clone()),
    )
    .map(|task| task.connection_id == connection_id)
    .unwrap_or(false)
}

async fn disconnect_connection_internal(
    db: &DatabaseConnection,
    connection: &strava_connections::Model,
    reason: &str,
    payload: Option<serde_json::Value>,
) -> Result<(), AppError> {
    let resolved = resolve_connection_sync_state(db, connection).await?;
    record_connection_strava_event_best_effort(
        db,
        &resolved.connection,
        "disconnect.requested",
        INTEGRATION_LEVEL_INFO,
        reason,
        Some(serde_json::json!({
            "active_sync_status": resolved.active_sync_status,
            "cleanup_local": true,
        })),
    )
    .await;

    let cancelled = cancel_pending_connection_sync_tasks(db, resolved.connection.id).await?;
    if cancelled > 0 {
        record_connection_strava_event_best_effort(
            db,
            &resolved.connection,
            "sync.cancelled",
            INTEGRATION_LEVEL_WARNING,
            format!("Cancelled {cancelled} queued Strava sync task(s)."),
            Some(serde_json::json!({
                "cancelled_task_count": cancelled,
            })),
        )
        .await;
    }

    release_user_activity_import_lock(
        db,
        resolved.connection.user_id,
        ACTIVITY_IMPORT_LOCK_SOURCE_STRAVA_SYNC,
    )
    .await?;

    strava_connections::Entity::delete_by_id(resolved.connection.id)
        .exec(db)
        .await?;

    record_connection_strava_event_best_effort(
        db,
        &resolved.connection,
        "disconnect.completed",
        INTEGRATION_LEVEL_SUCCESS,
        "Strava connection removed.",
        payload,
    )
    .await;

    Ok(())
}

async fn record_connection_strava_event_best_effort(
    db: &DatabaseConnection,
    connection: &strava_connections::Model,
    event_type: &str,
    level: &str,
    message: impl Into<String>,
    payload: Option<serde_json::Value>,
) {
    record_strava_event_best_effort(
        db,
        Some(connection.user_id),
        Some(connection.id),
        event_type,
        level,
        message,
        payload,
    )
    .await;
}

async fn record_strava_event_best_effort(
    db: &DatabaseConnection,
    user_id: Option<i32>,
    connection_id: Option<i32>,
    event_type: &str,
    level: &str,
    message: impl Into<String>,
    payload: Option<serde_json::Value>,
) {
    let message = message.into();

    if let Err(error) = integration_events_service::record_event(
        db,
        NewIntegrationEvent {
            user_id,
            provider: INTEGRATION_PROVIDER_STRAVA.to_string(),
            event_type: event_type.to_string(),
            level: level.to_string(),
            message: message.clone(),
            connection_id,
            payload,
        },
    )
    .await
    {
        tracing::warn!(
            event_type,
            connection_id,
            user_id,
            message = %error.message,
            log_message = %message,
            "failed to persist Strava integration event"
        );
    }
}

fn requested_oauth_scopes(config: &Config) -> Vec<String> {
    let mut scopes = config.strava_oauth_scope_list();
    if !scopes
        .iter()
        .any(|scope| scope == STRAVA_REQUIRED_ACTIVITY_SCOPE)
    {
        scopes.push(STRAVA_REQUIRED_ACTIVITY_SCOPE.to_string());
    }
    scopes
}

fn missing_activity_scope_message() -> String {
    format!(
        "Strava must grant {STRAVA_REQUIRED_ACTIVITY_SCOPE} so Bike can import private and Only Me activities. Reconnect Strava and approve {STRAVA_REQUIRED_ACTIVITY_SCOPE}."
    )
}

pub(crate) fn ensure_connection_scopes_allow_activity_import(
    connection: &strava_connections::Model,
) -> Result<(), AppError> {
    if scopes_allow_activity_import(&connection.scopes) {
        Ok(())
    } else {
        Err(AppError::bad_request(missing_activity_scope_message()))
    }
}

fn sign_state(config: &Config, payload: &str) -> Result<String, String> {
    let mut mac = HmacSha256::new_from_slice(config.jwt_secret.as_bytes())
        .map_err(|_| "Invalid JWT secret for Strava state signing".to_string())?;
    mac.update(payload.as_bytes());

    Ok(hex::encode(mac.finalize().into_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity_import_lock::{
        acquire_user_activity_import_lock, load_user_activity_import_lock,
        ACTIVITY_IMPORT_LOCK_STAGE_QUEUED,
    };
    use crate::entities::activities;
    use crate::entities::activity_import_locks;
    use crate::entities::analytics_user_states;
    use crate::entities::integration_events as integration_events_entity;
    use crate::entities::segment_efforts;
    use crate::strava::provider_payload::StravaStream;
    use axum::http::StatusCode;
    use chrono::Duration as ChronoDuration;
    use sea_orm::{ActiveModelTrait, ConnectionTrait, Database, EntityTrait, Schema};

    async fn test_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory db");

        let schema = Schema::new(db.get_database_backend());
        db.execute(&schema.create_table_from_entity(strava_connections::Entity))
            .await
            .expect("create Strava connections table");
        db.execute(&schema.create_table_from_entity(crate::entities::strava_workflows::Entity))
            .await
            .expect("create Strava workflows table");
        db.execute(&schema.create_table_from_entity(activity_import_locks::Entity))
            .await
            .expect("create activity import locks table");
        db.execute(&schema.create_table_from_entity(background_tasks::Entity))
            .await
            .expect("create background tasks table");
        db.execute(&schema.create_table_from_entity(activities::Entity))
            .await
            .expect("create activities table");
        db.execute(&schema.create_table_from_entity(segment_efforts::Entity))
            .await
            .expect("create segment efforts table");
        db.execute(&schema.create_table_from_entity(analytics_user_states::Entity))
            .await
            .expect("create analytics user states table");
        db.execute(&schema.create_table_from_entity(integration_events_entity::Entity))
            .await
            .expect("create integration events table");
        crate::background_jobs::history_tests::diagnostic_tables(&db).await;

        db
    }

    async fn insert_connection_with_scopes(
        db: &DatabaseConnection,
        user_id: i32,
        last_sync_status: &str,
        scopes: &str,
    ) -> strava_connections::Model {
        strava_connections::ActiveModel {
            user_id: Set(user_id),
            athlete_id: Set(10_000 + i64::from(user_id)),
            athlete_username: Set(Some(format!("athlete-{user_id}"))),
            athlete_first_name: Set(Some("Test".to_string())),
            athlete_last_name: Set(Some("Rider".to_string())),
            athlete_profile_medium_url: Set(None),
            scopes: Set(scopes.to_string()),
            access_token: Set("access-token".to_string()),
            refresh_token: Set("refresh-token".to_string()),
            expires_at: Set(Utc::now() + ChronoDuration::hours(1)),
            last_sync_status: Set(last_sync_status.to_string()),
            last_sync_message: Set(Some("Strava sync queued.".to_string())),
            last_sync_started_at: Set(None),
            last_sync_finished_at: Set(None),
            last_sync_imported_count: Set(0),
            last_sync_duplicate_count: Set(0),
            last_sync_failed_count: Set(0),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert Strava connection")
    }

    async fn insert_connection(
        db: &DatabaseConnection,
        user_id: i32,
        last_sync_status: &str,
    ) -> strava_connections::Model {
        insert_connection_with_scopes(db, user_id, last_sync_status, "activity:read_all").await
    }

    fn test_config() -> Config {
        Config {
            grafana_url: None,
            environment: "test".to_string(),
            database_url: "postgres://localhost/test".to_string(),
            frontend_url: "http://localhost:3001".to_string(),
            cors_allowed_origins: vec!["http://localhost:3001".to_string()],
            api_url: "http://localhost:3000".to_string(),
            strava_client_id: "12345".to_string(),
            strava_client_secret: "secret".to_string(),
            strava_oauth_scopes: "activity:read_all profile:read_all".to_string(),
            strava_webhook_verify_token: "verify-token".to_string(),
            uploads_dir: "./uploads".to_string(),
            map_renderer_grpc_address: None,
            map_service_token: String::new(),
            map_image_cache_dir: "./map-image-cache".into(),
            map_image_cache_ttl_seconds: 604800,
            max_upload_bytes: 1024,
            max_archive_fetch_bytes: 1024,
            archive_fetch_timeout_seconds: 60,
            jwt_secret: "test-secret".to_string(),
            synthetic_key: None,
            local_admin_enabled: false,
            app_name: "Bike".to_string(),
            smtp_host: "localhost".to_string(),
            smtp_port: 1025,
            smtp_username: None,
            smtp_password: None,
            smtp_from_email: "noreply@example.com".to_string(),
            smtp_from_name: "Bike".to_string(),
        }
    }

    #[test]
    fn state_token_round_trips() {
        let config = test_config();
        let token = create_state_token(&config, 42).unwrap();
        let verified = verify_state_token(&config, &token).unwrap();

        assert_eq!(verified.user_id, 42);
        assert!(!verified.nonce.is_empty());
    }

    #[test]
    fn rejects_tampered_state_token() {
        let config = test_config();
        let token = create_state_token(&config, 42).unwrap();
        let tampered = token.replacen("42:", "99:", 1);

        assert!(verify_state_token(&config, &tampered).is_err());
    }

    #[test]
    fn builds_authorization_url_with_expected_query() {
        let config = test_config();
        let url = build_authorization_url(&config, "signed-state").unwrap();

        assert_eq!(
            url.origin().unicode_serialization(),
            "https://www.strava.com"
        );
        assert_eq!(url.path(), "/oauth/authorize");
        assert!(url.as_str().contains("client_id=12345"));
        assert!(url
            .as_str()
            .contains("scope=activity%3Aread_all%2Cprofile%3Aread_all"));
        assert!(url.as_str().contains("state=signed-state"));
    }

    #[test]
    fn requires_activity_read_all_scope() {
        assert!(!scopes_allow_activity_import("activity:read"));
        assert!(scopes_allow_activity_import("read activity:read_all"));
        assert!(!scopes_allow_activity_import("read profile:read_all"));
    }

    #[test]
    fn authorization_url_always_requests_activity_read_all() {
        let mut config = test_config();
        config.strava_oauth_scopes = "read profile:read_all activity:read".to_string();

        let url = build_authorization_url(&config, "signed-state").unwrap();

        assert!(url
            .as_str()
            .contains("scope=read%2Cprofile%3Aread_all%2Cactivity%3Aread%2Cactivity%3Aread_all"));
    }

    #[test]
    fn verifies_strava_webhook_subscription_query() {
        let config = test_config();
        let response = verify_webhook_subscription(
            &config,
            &StravaWebhookSubscriptionQuery {
                mode: Some("subscribe".to_string()),
                challenge: Some("challenge-value".to_string()),
                verify_token: Some("verify-token".to_string()),
            },
        )
        .unwrap();

        assert_eq!(response.challenge, "challenge-value");
    }

    #[test]
    fn rejects_invalid_strava_webhook_verify_token() {
        let config = test_config();
        let error = verify_webhook_subscription(
            &config,
            &StravaWebhookSubscriptionQuery {
                mode: Some("subscribe".to_string()),
                challenge: Some("challenge-value".to_string()),
                verify_token: Some("wrong-token".to_string()),
            },
        )
        .unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn builds_strava_import_directly_from_provider_payload() {
        let activity = StravaActivitySummary {
            additional_fields: Default::default(),
            id: 99,
            name: "Lunch Ride".to_string(),
            distance: Some(1000.0),
            moving_time: Some(300),
            elapsed_time: Some(320),
            max_speed: Some(6.2),
            average_heartrate: Some(145.0),
            max_heartrate: Some(162.0),
            average_cadence: Some(88.0),
            calories: Some(120.0),
            sport_type: Some("Ride".to_string()),
            trainer: None,
            legacy_type: Some("Ride".to_string()),
            start_date: DateTime::parse_from_rfc3339("2026-05-12T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        };
        let streams = StravaActivityStreams {
            time: Some(test_stream(vec![0, 160, 320])),
            distance: Some(test_stream(vec![0.0, 500.0, 1000.0])),
            latlng: None,
            altitude: None,
            velocity_smooth: None,
            heartrate: None,
            cadence: None,
            watts: None,
            temp: Some(test_stream(vec![18, 19, 20])),
            moving: Some(test_stream(vec![true, true, false])),
            grade_smooth: Some(test_stream(vec![0.1, 0.2, -0.1])),
        };

        let payload = build_activity_upload(&activity, &streams).expect("build payload");

        assert_eq!(payload.format, "json");
        assert_eq!(payload.original_filename, "strava_activity_99.json");
        assert_eq!(payload.source_correlation_id.as_deref(), Some("99"));
        let raw_json: serde_json::Value =
            serde_json::from_slice(&payload.bytes).expect("provider payload json");
        assert_eq!(raw_json["v"], 1);
        assert_eq!(raw_json["provider"], "strava");
        assert_eq!(raw_json["provider_activity_id"], 99);
        assert_eq!(raw_json["activity"]["id"], 99);
        assert_eq!(raw_json["streams"]["temp"]["data"][0], 18);
        assert_eq!(raw_json["streams"]["moving"]["data"][2], false);
        assert_eq!(raw_json["streams"]["grade_smooth"]["data"][1], 0.2);
    }

    fn test_stream<T>(data: Vec<T>) -> StravaStream<T> {
        StravaStream {
            data,
            original_size: None,
            resolution: None,
            series_type: None,
        }
    }

    #[tokio::test]
    async fn resolves_stale_queued_sync_state_when_no_task_exists() {
        let db = test_db().await;
        let connection = insert_connection(&db, 7, STRAVA_SYNC_STATUS_QUEUED).await;

        let resolved = resolve_connection_sync_state(&db, &connection)
            .await
            .expect("resolve sync state");

        assert_eq!(resolved.active_sync_status, None);
        assert_eq!(
            resolved.connection.last_sync_status,
            STRAVA_SYNC_STATUS_NEVER
        );
        assert_eq!(
            resolved.connection.last_sync_message.as_deref(),
            Some("Strava sync is not currently active. Start another sync when ready."),
        );
    }

    #[tokio::test]
    async fn resolves_stale_running_sync_state_when_no_task_exists() {
        let db = test_db().await;
        let connection = insert_connection(&db, 12, STRAVA_SYNC_STATUS_RUNNING).await;
        let mut active_model: strava_connections::ActiveModel = connection.clone().into();
        active_model.last_sync_started_at = Set(Some(Utc::now()));
        let running_connection = active_model
            .update(&db)
            .await
            .expect("mark running connection started");

        let resolved = resolve_connection_sync_state(&db, &running_connection)
            .await
            .expect("resolve sync state");

        assert_eq!(resolved.active_sync_status, None);
        assert_eq!(
            resolved.connection.last_sync_status,
            STRAVA_SYNC_STATUS_FAILED
        );
        assert_eq!(
            resolved.connection.last_sync_message.as_deref(),
            Some(
                "The previous Strava sync stopped before it completed. Start another sync when ready.",
            ),
        );
    }

    #[tokio::test]
    async fn disconnect_connection_cancels_pending_sync_task_and_releases_lock() {
        let db = test_db().await;
        let connection = insert_connection(&db, 8, STRAVA_SYNC_STATUS_QUEUED).await;
        background_tasks::Model::enqueue(
            &db,
            "strava_sync".into(),
            serde_json::json!({"type": "StravaSync", "data": {"connection_id": connection.id}}),
            None,
            3,
        )
        .await
        .expect("enqueue historical sync task");
        acquire_user_activity_import_lock(
            &db,
            connection.user_id,
            ACTIVITY_IMPORT_LOCK_SOURCE_STRAVA_SYNC,
            ACTIVITY_IMPORT_LOCK_STAGE_QUEUED,
        )
        .await
        .expect("acquire sync lock");

        forget_local_connection(&db, connection.user_id)
            .await
            .expect("disconnect Strava");

        assert!(load_connection(&db, connection.user_id)
            .await
            .expect("load connection")
            .is_none());
        assert!(load_user_activity_import_lock(&db, connection.user_id)
            .await
            .expect("load lock")
            .is_none());

        let task = background_tasks::Entity::find()
            .one(&db)
            .await
            .expect("load task")
            .expect("task exists");
        assert_eq!(
            task.status,
            background_tasks::TaskStatus::Completed.as_str()
        );
        assert_eq!(
            task.result.as_deref(),
            Some("Cancelled because the Strava connection was disconnected."),
        );
    }

    #[tokio::test]
    async fn weak_connection_scope_returns_activity_scope_error() {
        let db = test_db().await;
        let connection =
            insert_connection_with_scopes(&db, 42, STRAVA_SYNC_STATUS_NEVER, "activity:read").await;

        let error = ensure_connection_scopes_allow_activity_import(&connection).unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.message, missing_activity_scope_message());
    }
}
