use crate::activity_import_lock::{
    acquire_user_activity_import_lock, ensure_user_activity_import_lock_stage,
    load_user_activity_import_lock, release_user_activity_import_lock,
    ACTIVITY_IMPORT_LOCK_SOURCE_STRAVA_SYNC, ACTIVITY_IMPORT_LOCK_STAGE_QUEUED,
    ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
};
use crate::activity_import_pipeline::{
    finalize_activity_import_batch, mark_activity_imports_processed,
    persist_activity_upload_with_artifacts, ActivityImportArtifactPayload,
    ActivityUploadDeduplication, ActivityUploadPayload, PersistActivityUploadOutcome,
    PersistActivityUploadWithArtifactsRequest, ACTIVITY_IMPORT_ARTIFACT_KIND_GENERATED_EXPORT,
    ACTIVITY_IMPORT_ARTIFACT_KIND_PROVIDER_PAYLOAD, ACTIVITY_IMPORT_SOURCE_QUALITY_GENERATED_TCX,
    ACTIVITY_IMPORT_SOURCE_QUALITY_STRAVA_STREAMS,
};
use crate::activity_lifecycle::{
    delete_activity_with_derived_state, resume_incomplete_activity_imports_for_user,
};
use crate::analytics::{mark_segment_activity_changes, mark_user_fitness_dirty};
use crate::app_error::AppError;
use crate::config::Config;
use crate::entities::{activities, strava_connections};
use crate::integration_events::{
    self, NewIntegrationEvent, INTEGRATION_LEVEL_ERROR, INTEGRATION_LEVEL_INFO,
    INTEGRATION_LEVEL_SUCCESS, INTEGRATION_LEVEL_WARNING, INTEGRATION_PROVIDER_STRAVA,
};
use crate::strava_provider_payload::{
    StoredStravaProviderPayload, StravaActivityStreams, StravaActivitySummary, StravaStream,
};
use crate::tasks::{StravaSyncTask, TaskQueue};
use crate::training_profile::load_training_profile;
use axum::http::StatusCode;
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use kaleido::auth::entities::users;
use kaleido::background_jobs::background_tasks;
use reqwest::Client;
use reqwest::Url;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use utoipa::ToSchema;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

pub const STRAVA_AUTHORIZE_URL: &str = "https://www.strava.com/oauth/authorize";
pub const STRAVA_TOKEN_URL: &str = "https://www.strava.com/api/v3/oauth/token";
pub const STRAVA_DEAUTHORIZE_URL: &str = "https://www.strava.com/oauth/deauthorize";
pub const STRAVA_API_BASE_URL: &str = "https://www.strava.com/api/v3";
pub const STRAVA_PUSH_SUBSCRIPTIONS_URL: &str = "https://www.strava.com/api/v3/push_subscriptions";
pub const STRAVA_SYNC_STATUS_NEVER: &str = "never";
pub const STRAVA_SYNC_STATUS_QUEUED: &str = "queued";
pub const STRAVA_SYNC_STATUS_RUNNING: &str = "running";
pub const STRAVA_SYNC_STATUS_SUCCEEDED: &str = "succeeded";
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

#[derive(Debug, Deserialize)]
struct StravaAuthorizationTokenResponse {
    access_token: String,
    refresh_token: String,
    expires_at: i64,
    scope: String,
    athlete: StravaAthleteSummary,
}

#[derive(Debug, Deserialize)]
struct StravaRefreshTokenResponse {
    access_token: String,
    refresh_token: String,
    expires_at: i64,
    scope: Option<String>,
    athlete: Option<StravaAthleteSummary>,
}

#[derive(Debug, Deserialize)]
struct StravaAthleteSummary {
    id: i64,
    username: Option<String>,
    firstname: Option<String>,
    lastname: Option<String>,
    profile_medium: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StravaFault {
    message: Option<String>,
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
pub struct StravaWebhookChallengeResponse {
    #[serde(rename = "hub.challenge")]
    pub challenge: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct StravaWebhookEvent {
    pub aspect_type: String,
    pub event_time: i64,
    pub object_id: i64,
    pub object_type: String,
    pub owner_id: i64,
    pub subscription_id: i64,
    pub updates: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct StravaPushSubscription {
    id: i64,
    callback_url: String,
}

struct StravaApiClient {
    client: Client,
    config: &'static Config,
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
    tasks: &TaskQueue,
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
        let client = StravaApiClient::new(config)?;
        let token_response = client.exchange_authorization_code(code).await?;

        if !scopes_allow_activity_import(&token_response.scope) {
            return Err(AppError::bad_request(missing_activity_scope_message()));
        }

        let connection =
            upsert_connection_from_token(db, verified_state.user_id, token_response).await?;
        ensure_webhook_subscription(db, config).await?;

        match queue_sync_task_for_connection(db, tasks, &connection, "Initial Strava sync queued.")
            .await
        {
            Ok(connection) => Ok(connection),
            Err(error) if error.status == StatusCode::CONFLICT => {
                set_sync_message(
                    db,
                    &connection,
                    "Strava connected. Start a sync after the current import finishes.",
                )
                .await
            }
            Err(error) => Err(error),
        }
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

pub async fn ensure_webhook_subscription_registered(
    db: &DatabaseConnection,
) -> Result<(), AppError> {
    ensure_webhook_subscription(db, Config::get()).await
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

pub async fn handle_webhook_event(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    event: &StravaWebhookEvent,
) -> Result<(), AppError> {
    record_strava_event_best_effort(
        db,
        None,
        None,
        "webhook.received",
        INTEGRATION_LEVEL_INFO,
        format!(
            "Received Strava webhook {} {}.",
            event.object_type, event.aspect_type
        ),
        Some(serde_json::json!(event)),
    )
    .await;

    match event.object_type.as_str() {
        "athlete" => {
            if event.aspect_type == "update" && athlete_update_revokes_access(event) {
                disconnect_connection_by_athlete_id(db, event.owner_id).await?;
            }
        }
        "activity" => {
            if let Some(connection) = load_connection_by_athlete_id(db, event.owner_id).await? {
                let resolved = resolve_connection_sync_state(db, &connection).await?;
                if event.aspect_type == "delete" {
                    let deleted = delete_strava_activity_by_correlation_id(
                        db,
                        &Config::get().uploads_dir,
                        tasks,
                        resolved.connection.user_id,
                        event.object_id,
                    )
                    .await?;
                    record_connection_strava_event_best_effort(
                        db,
                        &resolved.connection,
                        "webhook.activity_delete",
                        if deleted {
                            INTEGRATION_LEVEL_SUCCESS
                        } else {
                            INTEGRATION_LEVEL_WARNING
                        },
                        if deleted {
                            format!(
                                "Strava webhook deleted imported activity {}.",
                                event.object_id
                            )
                        } else {
                            format!(
                                "Strava webhook delete for activity {} did not match an imported Bike activity.",
                                event.object_id
                            )
                        },
                        Some(serde_json::json!({
                            "activity_id": event.object_id,
                            "aspect_type": event.aspect_type,
                        })),
                    )
                    .await;
                } else if resolved.active_sync_status.is_none() {
                    if !scopes_allow_activity_import(&resolved.connection.scopes) {
                        record_connection_strava_event_best_effort(
                            db,
                            &resolved.connection,
                            "webhook.activity_ignored",
                            INTEGRATION_LEVEL_WARNING,
                            &missing_activity_scope_message(),
                            Some(serde_json::json!({
                                "activity_id": event.object_id,
                                "aspect_type": event.aspect_type,
                                "granted_scopes": parse_scope_list(&resolved.connection.scopes),
                            })),
                        )
                        .await;
                    } else {
                        let _ = queue_sync_task_for_connection(
                            db,
                            tasks,
                            &resolved.connection,
                            "Strava webhook update queued a sync.",
                        )
                        .await;
                    }
                } else {
                    record_connection_strava_event_best_effort(
                        db,
                        &resolved.connection,
                        "webhook.activity_ignored",
                        INTEGRATION_LEVEL_WARNING,
                        "Ignored Strava activity webhook because a sync is already active.",
                        Some(serde_json::json!({
                            "activity_id": event.object_id,
                            "aspect_type": event.aspect_type,
                            "active_sync_status": resolved.active_sync_status,
                        })),
                    )
                    .await;
                }
            } else {
                record_strava_event_best_effort(
                    db,
                    None,
                    None,
                    "webhook.activity_ignored",
                    INTEGRATION_LEVEL_WARNING,
                    "Ignored Strava activity webhook because no Bike connection matched the athlete.",
                    Some(serde_json::json!({
                        "owner_id": event.owner_id,
                        "activity_id": event.object_id,
                        "aspect_type": event.aspect_type,
                    })),
                )
                .await;
            }
        }
        _ => {}
    }

    Ok(())
}

pub async fn queue_connection_sync(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    user_id: i32,
) -> Result<strava_connections::Model, AppError> {
    ensure_strava_configured(Config::get())?;

    if let Err(error) = ensure_webhook_subscription(db, Config::get()).await {
        tracing::warn!(
            message = %error.message,
            "failed to ensure Strava webhook subscription before queueing sync"
        );
    }

    let connection = load_connection(db, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("Connect Strava before starting a sync"))?;
    let resolved = resolve_connection_sync_state(db, &connection).await?;

    if resolved.active_sync_status.is_some() {
        return Ok(resolved.connection);
    }

    ensure_connection_scopes_allow_activity_import(&resolved.connection)?;

    queue_sync_task_for_connection(db, tasks, &resolved.connection, "Strava sync queued.").await
}

pub async fn disconnect_connection(db: &DatabaseConnection, user_id: i32) -> Result<(), AppError> {
    let Some(connection) = load_connection(db, user_id).await? else {
        return Ok(());
    };

    disconnect_connection_internal(
        db,
        &connection,
        true,
        "User requested Strava disconnect.",
        Some(serde_json::json!({
            "trigger": "user",
        })),
    )
    .await
}

pub async fn process_strava_sync(
    db: &DatabaseConnection,
    uploads_dir: &str,
    connection_id: i32,
) -> Result<(), AppError> {
    let Some(connection) = strava_connections::Entity::find_by_id(connection_id)
        .one(db)
        .await?
    else {
        tracing::info!(
            connection_id,
            "skipping Strava sync because the connection no longer exists"
        );
        return Ok(());
    };
    ensure_user_activity_import_lock_stage(
        db,
        connection.user_id,
        ACTIVITY_IMPORT_LOCK_SOURCE_STRAVA_SYNC,
        ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
    )
    .await?;

    let result = async {
        let connection = mark_sync_running(db, &connection).await?;
        let client = StravaApiClient::new(Config::get())?;
        let connection = ensure_fresh_access_token(db, &client, connection).await?;
        ensure_connection_scopes_allow_activity_import(&connection)?;
        let user = users::Entity::find_by_id(connection.user_id)
            .one(db)
            .await?
            .ok_or_else(|| AppError::internal(format!("User {} for Strava sync was not found", connection.user_id)))?;
        let user_storage_key = user.pid.to_string();
        let tasks = TaskQueue::new(db.clone());
        resume_incomplete_activity_imports_for_user(db, uploads_dir, &tasks, connection.user_id)
            .await?;
        let training_profile = load_training_profile(db, connection.user_id).await?;
        let latest_user_activity_started_at =
            load_latest_user_activity_started_at(db, connection.user_id).await?;
        let after_epoch = strava_sync_after_epoch(
            connection.last_synced_activity_started_at,
            latest_user_activity_started_at,
        );

        let mut page = 1usize;
        let mut imported_count = 0i32;
        let mut duplicate_count = 0i32;
        let mut failed_count = 0i32;
        let mut affected_segment_ids = Vec::new();
        let mut imported_import_ids = Vec::new();
        let mut fitness_dirty_from_day: Option<chrono::NaiveDate> = None;
        let mut latest_started_at = connection.last_synced_activity_started_at;

        // TODO: Large initial syncs can still put many generated files in one monthly
        // bucket; add finer-grained sharding if that becomes an operational problem.
        loop {
            if stop_if_connection_removed(db, &connection).await? {
                return Ok(());
            }

            let activities = client
                .list_activities(&connection.access_token, after_epoch, page, 100)
                .await
                .inspect_err(|error| {
                    tracing::error!(message = %error.message, page, "failed to list Strava activities");
                })?;

            if activities.is_empty() {
                break;
            }

            for activity in activities.iter().rev() {
                if stop_if_connection_removed(db, &connection).await? {
                    return Ok(());
                }

                latest_started_at = Some(match latest_started_at {
                    Some(current) if current >= activity.start_date => current,
                    _ => activity.start_date,
                });

                let streams = match client
                    .get_activity_streams(&connection.access_token, activity.id)
                    .await
                {
                    Ok(value) => value,
                    Err(error) => {
                        failed_count += 1;
                        tracing::warn!(
                            activity_id = activity.id,
                            message = %error.message,
                            "failed to fetch Strava activity streams"
                        );
                        continue;
                    }
                };

                let import_payload = match build_activity_upload(activity, &streams) {
                    Ok(value) => value,
                    Err(error) => {
                        failed_count += 1;
                        tracing::warn!(
                            activity_id = activity.id,
                            message = %error.message,
                            "failed to build synthetic Strava activity upload"
                        );
                        continue;
                    }
                };

                if stop_if_connection_removed(db, &connection).await? {
                    return Ok(());
                }

                let persist_request = PersistActivityUploadWithArtifactsRequest {
                    uploads_dir,
                    user_storage_key: &user_storage_key,
                    user_id: connection.user_id,
                    upload: import_payload.generated_tcx_upload,
                    primary_artifact_kind: ACTIVITY_IMPORT_ARTIFACT_KIND_GENERATED_EXPORT,
                    primary_source_quality: ACTIVITY_IMPORT_SOURCE_QUALITY_GENERATED_TCX,
                    additional_artifacts: vec![import_payload.provider_payload_artifact],
                    source: "strava_sync",
                    deduplication: ActivityUploadDeduplication::Enabled,
                    training_profile: Some(&training_profile),
                };

                match persist_activity_upload_with_artifacts(db, persist_request).await {
                    Ok(PersistActivityUploadOutcome::Imported(persisted)) => {
                        imported_count += 1;
                        imported_import_ids.push(persisted.import.id);
                        affected_segment_ids.extend(persisted.affected_segment_ids);
                        fitness_dirty_from_day = Some(match fitness_dirty_from_day {
                            Some(current) => current.min(persisted.fitness_dirty_from_day),
                            None => persisted.fitness_dirty_from_day,
                        });
                    }
                    Ok(PersistActivityUploadOutcome::Duplicate(_)) => {
                        duplicate_count += 1;
                    }
                    Err(error) => {
                        failed_count += 1;
                        tracing::warn!(
                            activity_id = activity.id,
                            message = %error.message,
                            "failed to persist Strava activity upload"
                        );
                    }
                }
            }

            if activities.len() < 100 {
                break;
            }

            page += 1;
        }

        if stop_if_connection_removed(db, &connection).await? {
            return Ok(());
        }

        if imported_count > 0 {
            finalize_activity_import_batch(
                db,
                &tasks,
                connection.user_id,
                affected_segment_ids,
                fitness_dirty_from_day,
                Utc::now(),
            )
            .await?;
            mark_activity_imports_processed(db, &imported_import_ids).await?;
        }

        if failed_count > 0 && imported_count == 0 && duplicate_count == 0 {
            let message = "Strava sync could not import any activities".to_string();
            mark_sync_failed(
                db,
                &connection,
                &message,
                imported_count,
                duplicate_count,
                failed_count,
            )
            .await?;
            return Err(AppError::internal(message));
        }

        let message = build_sync_summary_message(imported_count, duplicate_count, failed_count);
        mark_sync_succeeded(
            db,
            &connection,
            latest_started_at,
            imported_count,
            duplicate_count,
            failed_count,
            &message,
        )
        .await?;

        Ok(())
    }
    .await;

    if let Err(error) = &result {
        let _ = mark_sync_failed_if_running(db, connection.id, &error.message).await;
    }

    let release_result = release_user_activity_import_lock(
        db,
        connection.user_id,
        ACTIVITY_IMPORT_LOCK_SOURCE_STRAVA_SYNC,
    )
    .await;

    match (result, release_result) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Ok(_), Ok(())) => Ok(()),
    }
}

impl StravaApiClient {
    fn new(config: &'static Config) -> Result<Self, AppError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .user_agent(format!("{}/strava-sync", config.app_name))
            .build()
            .map_err(|error| {
                tracing::error!(error = ?error, "failed to build Strava HTTP client");
                AppError::internal("Failed to initialize Strava integration")
            })?;

        Ok(Self { client, config })
    }

    async fn exchange_authorization_code(
        &self,
        code: &str,
    ) -> Result<StravaAuthorizationTokenResponse, AppError> {
        self.parse_json_response(
            self.client
                .post(STRAVA_TOKEN_URL)
                .form(&[
                    ("client_id", self.config.strava_client_id.as_str()),
                    ("client_secret", self.config.strava_client_secret.as_str()),
                    ("code", code),
                    ("grant_type", "authorization_code"),
                ])
                .send()
                .await,
            "exchange a Strava authorization code",
        )
        .await
    }

    async fn refresh_access_token(
        &self,
        refresh_token: &str,
    ) -> Result<StravaRefreshTokenResponse, AppError> {
        self.parse_json_response(
            self.client
                .post(STRAVA_TOKEN_URL)
                .form(&[
                    ("client_id", self.config.strava_client_id.as_str()),
                    ("client_secret", self.config.strava_client_secret.as_str()),
                    ("refresh_token", refresh_token),
                    ("grant_type", "refresh_token"),
                ])
                .send()
                .await,
            "refresh a Strava access token",
        )
        .await
    }

    async fn list_activities(
        &self,
        access_token: &str,
        after_epoch: Option<i64>,
        page: usize,
        per_page: usize,
    ) -> Result<Vec<StravaActivitySummary>, AppError> {
        let mut request = self
            .client
            .get(format!("{STRAVA_API_BASE_URL}/athlete/activities"))
            .bearer_auth(access_token)
            .query(&[("page", page), ("per_page", per_page)]);

        if let Some(after_epoch) = after_epoch {
            request = request.query(&[("after", after_epoch)]);
        }

        self.parse_json_response(request.send().await, "list Strava activities")
            .await
    }

    async fn get_activity_streams(
        &self,
        access_token: &str,
        activity_id: i64,
    ) -> Result<StravaActivityStreams, AppError> {
        self.parse_json_response(
            self.client
                .get(format!(
                    "{STRAVA_API_BASE_URL}/activities/{activity_id}/streams"
                ))
                .bearer_auth(access_token)
                .query(&[
                    (
                        "keys",
                        "time,distance,latlng,altitude,velocity_smooth,heartrate,cadence,watts,temp,moving,grade_smooth",
                    ),
                    ("key_by_type", "true"),
                ])
                .send()
                .await,
            "fetch Strava activity streams",
        )
        .await
    }

    async fn deauthorize(&self, access_token: &str) -> Result<(), AppError> {
        self.parse_json_response::<serde_json::Value>(
            self.client
                .post(STRAVA_DEAUTHORIZE_URL)
                .query(&[("access_token", access_token)])
                .send()
                .await,
            "deauthorize the Strava app",
        )
        .await
        .map(|_| ())
    }

    async fn list_push_subscriptions(&self) -> Result<Vec<StravaPushSubscription>, AppError> {
        self.parse_json_response(
            self.client
                .get(STRAVA_PUSH_SUBSCRIPTIONS_URL)
                .query(&[
                    ("client_id", self.config.strava_client_id.as_str()),
                    ("client_secret", self.config.strava_client_secret.as_str()),
                ])
                .send()
                .await,
            "list Strava webhook subscriptions",
        )
        .await
    }

    async fn create_push_subscription(&self) -> Result<StravaPushSubscription, AppError> {
        let callback_url = self.config.strava_webhook_callback_url();
        self.parse_json_response(
            self.client
                .post(STRAVA_PUSH_SUBSCRIPTIONS_URL)
                .form(&[
                    ("client_id", self.config.strava_client_id.as_str()),
                    ("client_secret", self.config.strava_client_secret.as_str()),
                    ("callback_url", callback_url.as_str()),
                    (
                        "verify_token",
                        self.config.strava_webhook_verify_token.as_str(),
                    ),
                ])
                .send()
                .await,
            "create a Strava webhook subscription",
        )
        .await
    }

    async fn parse_json_response<T>(
        &self,
        response: Result<reqwest::Response, reqwest::Error>,
        action: &str,
    ) -> Result<T, AppError>
    where
        T: DeserializeOwned,
    {
        let response = response.map_err(|error| {
            tracing::error!(error = ?error, action, "Strava request failed");
            AppError::internal(format!("Failed to {action}"))
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            let message = serde_json::from_str::<StravaFault>(&body)
                .ok()
                .and_then(|fault| fault.message)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| format!("Strava request failed with status {status}"));

            return Err(if status.is_client_error() {
                AppError::bad_request(message)
            } else {
                AppError::internal(message)
            });
        }

        response.json::<T>().await.map_err(|error| {
            tracing::error!(error = ?error, action, "failed to parse Strava response");
            AppError::internal(format!(
                "Failed to parse Strava response while trying to {action}"
            ))
        })
    }
}

async fn ensure_webhook_subscription(
    db: &DatabaseConnection,
    config: &Config,
) -> Result<(), AppError> {
    if !config.strava_webhook_enabled() {
        return Ok(());
    }

    let client = StravaApiClient::new(Config::get())?;
    let callback_url = config.strava_webhook_callback_url();
    let subscriptions = match client.list_push_subscriptions().await {
        Ok(subscriptions) => subscriptions,
        Err(error) => {
            record_strava_event_best_effort(
                db,
                None,
                None,
                "webhook.subscription_failed",
                INTEGRATION_LEVEL_ERROR,
                error.message.clone(),
                Some(serde_json::json!({
                    "stage": "list_push_subscriptions",
                    "callback_url": callback_url,
                })),
            )
            .await;
            return Err(error);
        }
    };

    if subscriptions
        .iter()
        .any(|subscription| subscription.callback_url == callback_url)
    {
        return Ok(());
    }

    let subscription = match client.create_push_subscription().await {
        Ok(subscription) => subscription,
        Err(error) => {
            record_strava_event_best_effort(
                db,
                None,
                None,
                "webhook.subscription_failed",
                INTEGRATION_LEVEL_ERROR,
                error.message.clone(),
                Some(serde_json::json!({
                    "stage": "create_push_subscription",
                    "callback_url": callback_url,
                })),
            )
            .await;
            return Err(error);
        }
    };
    tracing::info!(
        subscription_id = subscription.id,
        callback_url,
        "created Strava webhook subscription"
    );

    record_strava_event_best_effort(
        db,
        None,
        None,
        "webhook.subscription_created",
        INTEGRATION_LEVEL_SUCCESS,
        format!("Created Strava webhook subscription {}.", subscription.id),
        Some(serde_json::json!({
            "subscription_id": subscription.id,
            "callback_url": callback_url,
        })),
    )
    .await;

    Ok(())
}

async fn load_latest_user_activity_started_at(
    db: &DatabaseConnection,
    user_id: i32,
) -> Result<Option<DateTime<Utc>>, AppError> {
    activities::Entity::find()
        .filter(activities::Column::UserId.eq(user_id))
        .order_by_desc(activities::Column::StartedAt)
        .one(db)
        .await
        .map(|activity| activity.map(|activity| activity.started_at))
        .map_err(AppError::from)
}

async fn load_connection_by_athlete_id(
    db: &DatabaseConnection,
    athlete_id: i64,
) -> Result<Option<strava_connections::Model>, AppError> {
    strava_connections::Entity::find()
        .filter(strava_connections::Column::AthleteId.eq(athlete_id))
        .one(db)
        .await
        .map_err(AppError::from)
}

async fn disconnect_connection_by_athlete_id(
    db: &DatabaseConnection,
    athlete_id: i64,
) -> Result<(), AppError> {
    let Some(connection) = load_connection_by_athlete_id(db, athlete_id).await? else {
        return Ok(());
    };

    record_connection_strava_event_best_effort(
        db,
        &connection,
        "webhook.athlete_deauthorized",
        INTEGRATION_LEVEL_WARNING,
        "Strava webhook reported that the athlete revoked access.",
        Some(serde_json::json!({
            "athlete_id": athlete_id,
        })),
    )
    .await;

    disconnect_connection_internal(
        db,
        &connection,
        false,
        "Strava revoked access via webhook.",
        Some(serde_json::json!({
            "trigger": "webhook",
            "athlete_id": athlete_id,
        })),
    )
    .await
}

fn athlete_update_revokes_access(event: &StravaWebhookEvent) -> bool {
    event
        .updates
        .as_ref()
        .and_then(|updates| updates.get("authorized"))
        .and_then(|value| {
            value.as_str().or_else(|| {
                value
                    .as_bool()
                    .map(|flag| if flag { "true" } else { "false" })
            })
        })
        == Some("false")
}

fn strava_sync_after_started_at(
    last_synced_activity_started_at: Option<DateTime<Utc>>,
    latest_user_activity_started_at: Option<DateTime<Utc>>,
) -> Option<DateTime<Utc>> {
    match (
        last_synced_activity_started_at,
        latest_user_activity_started_at,
    ) {
        (Some(last_synced), Some(latest_user_activity)) => {
            Some(last_synced.max(latest_user_activity))
        }
        (Some(last_synced), None) => Some(last_synced),
        (None, Some(latest_user_activity)) => Some(latest_user_activity),
        (None, None) => None,
    }
}

fn strava_sync_after_epoch(
    last_synced_activity_started_at: Option<DateTime<Utc>>,
    latest_user_activity_started_at: Option<DateTime<Utc>>,
) -> Option<i64> {
    strava_sync_after_started_at(
        last_synced_activity_started_at,
        latest_user_activity_started_at,
    )
    .map(|timestamp| (timestamp - Duration::minutes(5)).timestamp())
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
        active_model.access_token = Set(token_response.access_token);
        active_model.refresh_token = Set(token_response.refresh_token);
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
            access_token: Set(token_response.access_token),
            refresh_token: Set(token_response.refresh_token),
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

async fn ensure_fresh_access_token(
    db: &DatabaseConnection,
    client: &StravaApiClient,
    connection: strava_connections::Model,
) -> Result<strava_connections::Model, AppError> {
    if connection.expires_at > Utc::now() + Duration::minutes(5) {
        return Ok(connection);
    }

    let token_response = client
        .refresh_access_token(&connection.refresh_token)
        .await?;
    let expires_at = DateTime::<Utc>::from_timestamp(token_response.expires_at, 0)
        .ok_or_else(|| AppError::internal("Strava returned an invalid token expiration"))?;
    let mut active_model: strava_connections::ActiveModel = connection.clone().into();
    active_model.scopes = Set(token_response
        .scope
        .unwrap_or_else(|| connection.scopes.clone()));
    active_model.access_token = Set(token_response.access_token);
    active_model.refresh_token = Set(token_response.refresh_token);
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

async fn mark_sync_queued(
    db: &DatabaseConnection,
    connection: &strava_connections::Model,
    message: &str,
) -> Result<strava_connections::Model, AppError> {
    let mut active_model: strava_connections::ActiveModel = connection.clone().into();
    active_model.last_sync_status = Set(STRAVA_SYNC_STATUS_QUEUED.to_string());
    active_model.last_sync_message = Set(Some(message.to_string()));
    let connection = active_model.update(db).await.map_err(AppError::from)?;
    record_connection_strava_event_best_effort(
        db,
        &connection,
        "sync.queued",
        INTEGRATION_LEVEL_INFO,
        message,
        None,
    )
    .await;
    Ok(connection)
}

async fn mark_sync_running(
    db: &DatabaseConnection,
    connection: &strava_connections::Model,
) -> Result<strava_connections::Model, AppError> {
    let mut active_model: strava_connections::ActiveModel = connection.clone().into();
    active_model.last_sync_status = Set(STRAVA_SYNC_STATUS_RUNNING.to_string());
    active_model.last_sync_message = Set(None);
    active_model.last_sync_started_at = Set(Some(Utc::now()));
    active_model.last_sync_finished_at = Set(None);
    active_model.last_sync_imported_count = Set(0);
    active_model.last_sync_duplicate_count = Set(0);
    active_model.last_sync_failed_count = Set(0);
    let connection = active_model.update(db).await.map_err(AppError::from)?;
    record_connection_strava_event_best_effort(
        db,
        &connection,
        "sync.running",
        INTEGRATION_LEVEL_INFO,
        "Strava sync started.",
        None,
    )
    .await;
    Ok(connection)
}

async fn mark_sync_succeeded(
    db: &DatabaseConnection,
    connection: &strava_connections::Model,
    latest_started_at: Option<DateTime<Utc>>,
    imported_count: i32,
    duplicate_count: i32,
    failed_count: i32,
    message: &str,
) -> Result<strava_connections::Model, AppError> {
    let mut active_model: strava_connections::ActiveModel = connection.clone().into();
    active_model.last_synced_activity_started_at = Set(latest_started_at);
    active_model.last_sync_status = Set(STRAVA_SYNC_STATUS_SUCCEEDED.to_string());
    active_model.last_sync_message = Set(Some(message.to_string()));
    active_model.last_sync_finished_at = Set(Some(Utc::now()));
    active_model.last_sync_imported_count = Set(imported_count);
    active_model.last_sync_duplicate_count = Set(duplicate_count);
    active_model.last_sync_failed_count = Set(failed_count);
    let connection = active_model.update(db).await.map_err(AppError::from)?;
    record_connection_strava_event_best_effort(
        db,
        &connection,
        "sync.succeeded",
        INTEGRATION_LEVEL_SUCCESS,
        message,
        Some(serde_json::json!({
            "imported_count": imported_count,
            "duplicate_count": duplicate_count,
            "failed_count": failed_count,
            "last_synced_activity_started_at": latest_started_at,
        })),
    )
    .await;
    Ok(connection)
}

async fn mark_sync_failed(
    db: &DatabaseConnection,
    connection: &strava_connections::Model,
    message: &str,
    imported_count: i32,
    duplicate_count: i32,
    failed_count: i32,
) -> Result<strava_connections::Model, AppError> {
    let mut active_model: strava_connections::ActiveModel = connection.clone().into();
    active_model.last_sync_status = Set(STRAVA_SYNC_STATUS_FAILED.to_string());
    active_model.last_sync_message = Set(Some(message.to_string()));
    active_model.last_sync_finished_at = Set(Some(Utc::now()));
    active_model.last_sync_imported_count = Set(imported_count);
    active_model.last_sync_duplicate_count = Set(duplicate_count);
    active_model.last_sync_failed_count = Set(failed_count);
    let connection = active_model.update(db).await.map_err(AppError::from)?;
    record_connection_strava_event_best_effort(
        db,
        &connection,
        "sync.failed",
        INTEGRATION_LEVEL_ERROR,
        message,
        Some(serde_json::json!({
            "imported_count": imported_count,
            "duplicate_count": duplicate_count,
            "failed_count": failed_count,
        })),
    )
    .await;
    Ok(connection)
}

async fn mark_sync_failed_if_running(
    db: &DatabaseConnection,
    connection_id: i32,
    message: &str,
) -> Result<(), AppError> {
    let Some(connection) = strava_connections::Entity::find_by_id(connection_id)
        .one(db)
        .await?
    else {
        return Ok(());
    };

    if connection.last_sync_status != STRAVA_SYNC_STATUS_RUNNING {
        return Ok(());
    }

    mark_sync_failed(db, &connection, message, 0, 0, 0)
        .await
        .map(|_| ())
}

async fn set_sync_message(
    db: &DatabaseConnection,
    connection: &strava_connections::Model,
    message: &str,
) -> Result<strava_connections::Model, AppError> {
    let mut active_model: strava_connections::ActiveModel = connection.clone().into();
    active_model.last_sync_message = Set(Some(message.to_string()));
    active_model.update(db).await.map_err(AppError::from)
}

async fn queue_sync_task_for_connection(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    connection: &strava_connections::Model,
    queued_message: &str,
) -> Result<strava_connections::Model, AppError> {
    acquire_user_activity_import_lock(
        db,
        connection.user_id,
        ACTIVITY_IMPORT_LOCK_SOURCE_STRAVA_SYNC,
        ACTIVITY_IMPORT_LOCK_STAGE_QUEUED,
    )
    .await?;

    let queued_connection = match mark_sync_queued(db, connection, queued_message).await {
        Ok(connection) => connection,
        Err(error) => {
            release_user_activity_import_lock(
                db,
                connection.user_id,
                ACTIVITY_IMPORT_LOCK_SOURCE_STRAVA_SYNC,
            )
            .await?;
            return Err(error);
        }
    };

    if let Err(message) = tasks.sync_strava_connection(queued_connection.id).await {
        tracing::error!(connection_id = queued_connection.id, %message, "failed to enqueue Strava sync task");
        let failed_message = format!("Failed to queue Strava sync: {message}");
        let _ = mark_sync_failed(db, &queued_connection, &failed_message, 0, 0, 0).await;
        release_user_activity_import_lock(
            db,
            connection.user_id,
            ACTIVITY_IMPORT_LOCK_SOURCE_STRAVA_SYNC,
        )
        .await?;
        return Err(AppError::internal(failed_message));
    }

    Ok(queued_connection)
}

struct StravaActivityImportPayload {
    generated_tcx_upload: ActivityUploadPayload,
    provider_payload_artifact: ActivityImportArtifactPayload,
}

fn build_activity_upload(
    activity: &StravaActivitySummary,
    streams: &StravaActivityStreams,
) -> Result<StravaActivityImportPayload, AppError> {
    let original_filename = format!(
        "{}.tcx",
        sanitize_title_for_filename(&activity.name, activity.id)
    );
    let bytes = build_tcx_document(activity, streams).into_bytes();
    let provider_payload = serde_json::to_vec(&StoredStravaProviderPayload::new(
        activity.clone(),
        streams.clone(),
    ))
    .map_err(|error| {
        AppError::internal(format!(
            "Failed to serialize Strava provider payload for activity {}: {error}",
            activity.id
        ))
    })?;

    Ok(StravaActivityImportPayload {
        generated_tcx_upload: ActivityUploadPayload {
            original_filename,
            format: "tcx".to_string(),
            mime_type: Some("application/vnd.garmin.tcx+xml".to_string()),
            source_correlation_id: Some(activity.id.to_string()),
            bytes,
        },
        provider_payload_artifact: ActivityImportArtifactPayload {
            artifact_kind: ACTIVITY_IMPORT_ARTIFACT_KIND_PROVIDER_PAYLOAD.to_string(),
            format: "json".to_string(),
            source_quality: ACTIVITY_IMPORT_SOURCE_QUALITY_STRAVA_STREAMS.to_string(),
            original_filename: format!("strava_activity_{}.json", activity.id),
            mime_type: Some("application/json".to_string()),
            bytes: provider_payload,
        },
    })
}

async fn delete_strava_activity_by_correlation_id(
    db: &DatabaseConnection,
    uploads_dir: &str,
    tasks: &TaskQueue,
    user_id: i32,
    source_correlation_id: i64,
) -> Result<bool, AppError> {
    let Some(activity) = activities::Entity::find()
        .filter(activities::Column::UserId.eq(user_id))
        .filter(activities::Column::Source.eq("strava_sync"))
        .filter(activities::Column::SourceCorrelationId.eq(source_correlation_id.to_string()))
        .one(db)
        .await?
    else {
        return Ok(false);
    };

    let fitness_dirty_from_day = activity.started_at.date_naive();
    let affected_segment_ids =
        delete_activity_with_derived_state(db, uploads_dir, user_id, activity).await?;
    let changed_at = Utc::now();
    mark_user_fitness_dirty(db, user_id, fitness_dirty_from_day, changed_at).await?;
    mark_segment_activity_changes(db, &affected_segment_ids, changed_at).await?;
    tasks.rebuild_fitness_freshness(user_id).await;
    tasks.rebuild_segment_analytics(affected_segment_ids).await;

    Ok(true)
}

pub async fn resolve_connection_sync_state(
    db: &DatabaseConnection,
    connection: &strava_connections::Model,
) -> Result<ResolvedStravaConnectionSyncState, AppError> {
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

async fn connection_still_exists(
    db: &DatabaseConnection,
    connection_id: i32,
) -> Result<bool, AppError> {
    Ok(strava_connections::Entity::find_by_id(connection_id)
        .one(db)
        .await?
        .is_some())
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

async fn stop_if_connection_removed(
    db: &DatabaseConnection,
    connection: &strava_connections::Model,
) -> Result<bool, AppError> {
    if connection_still_exists(db, connection.id).await? {
        return Ok(false);
    }

    tracing::info!(
        connection_id = connection.id,
        "stopping Strava sync because the connection was removed"
    );
    record_connection_strava_event_best_effort(
        db,
        connection,
        "sync.cancelled",
        INTEGRATION_LEVEL_WARNING,
        "Stopped Strava sync because the connection was removed.",
        None,
    )
    .await;
    Ok(true)
}

async fn disconnect_connection_internal(
    db: &DatabaseConnection,
    connection: &strava_connections::Model,
    should_deauthorize_remote: bool,
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
            "should_deauthorize_remote": should_deauthorize_remote,
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

    if should_deauthorize_remote && should_attempt_remote_strava_deauthorize() {
        let client = StravaApiClient::new(Config::get())?;
        if let Err(error) = client.deauthorize(&resolved.connection.access_token).await {
            tracing::warn!(message = %error.message, "failed to deauthorize Strava connection before delete");
            record_connection_strava_event_best_effort(
                db,
                &resolved.connection,
                "disconnect.remote_deauthorize_failed",
                INTEGRATION_LEVEL_WARNING,
                format!(
                    "Failed to deauthorize Strava during disconnect: {}",
                    error.message
                ),
                None,
            )
            .await;
        }
    }

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

    if let Err(error) = integration_events::record_event(
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

#[cfg(not(test))]
fn should_attempt_remote_strava_deauthorize() -> bool {
    Config::get().strava_enabled()
}

#[cfg(test)]
fn should_attempt_remote_strava_deauthorize() -> bool {
    false
}

fn build_tcx_document(activity: &StravaActivitySummary, streams: &StravaActivityStreams) -> String {
    let total_time_seconds = activity
        .elapsed_time
        .or(activity.moving_time)
        .unwrap_or_default()
        .max(0);
    let distance_meters = activity
        .distance
        .or_else(|| {
            streams
                .distance
                .as_ref()
                .and_then(|stream| stream.data.last().copied())
        })
        .unwrap_or_default();
    let max_speed_mps = activity.max_speed.or_else(|| {
        streams
            .velocity_smooth
            .as_ref()
            .and_then(|stream| stream.data.iter().copied().reduce(f64::max))
    });
    let average_heart_rate_bpm = activity
        .average_heartrate
        .map(|value| value.round() as i32)
        .or_else(|| average_i32_stream(streams.heartrate.as_ref()));
    let max_heart_rate_bpm = activity
        .max_heartrate
        .map(|value| value.round() as i32)
        .or_else(|| max_i32_stream(streams.heartrate.as_ref()));
    let average_cadence_rpm = activity
        .average_cadence
        .map(|value| value.round() as i32)
        .or_else(|| average_f64_stream(streams.cadence.as_ref()).map(|value| value.round() as i32));
    let calories = activity
        .calories
        .map(|value| value.round() as i32)
        .unwrap_or_default();
    let trackpoint_count = max_trackpoint_count(streams);
    let start_date = activity.start_date.to_rfc3339();
    let sport = tcx_sport(activity);
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<TrainingCenterDatabase xmlns=\"http://www.garmin.com/xmlschemas/TrainingCenterDatabase/v2\" xmlns:ns3=\"http://www.garmin.com/xmlschemas/ActivityExtension/v2\">\n  <Activities>\n",
    );

    xml.push_str(&format!(
        "    <Activity Sport=\"{}\">\n      <Id>{}</Id>\n      <Lap StartTime=\"{}\">\n        <TotalTimeSeconds>{}</TotalTimeSeconds>\n        <DistanceMeters>{:.3}</DistanceMeters>\n",
        escape_xml_text(sport),
        escape_xml_text(&start_date),
        escape_xml_text(&start_date),
        total_time_seconds,
        distance_meters,
    ));

    if let Some(max_speed_mps) = max_speed_mps {
        xml.push_str(&format!(
            "        <MaximumSpeed>{:.3}</MaximumSpeed>\n",
            max_speed_mps
        ));
    }
    if average_heart_rate_bpm.is_some() || max_heart_rate_bpm.is_some() {
        if let Some(value) = average_heart_rate_bpm {
            xml.push_str(&format!(
                "        <AverageHeartRateBpm><Value>{}</Value></AverageHeartRateBpm>\n",
                value
            ));
        }
        if let Some(value) = max_heart_rate_bpm {
            xml.push_str(&format!(
                "        <MaximumHeartRateBpm><Value>{}</Value></MaximumHeartRateBpm>\n",
                value
            ));
        }
    }
    if let Some(cadence) = average_cadence_rpm {
        xml.push_str(&format!("        <Cadence>{}</Cadence>\n", cadence));
    }
    if calories > 0 {
        xml.push_str(&format!("        <Calories>{}</Calories>\n", calories));
    }

    if trackpoint_count > 0 {
        xml.push_str("        <Track>\n");
        for index in 0..trackpoint_count {
            let elapsed_seconds = stream_time_value(streams, index)
                .or_else(|| {
                    interpolate_elapsed_seconds(index, trackpoint_count, total_time_seconds)
                })
                .unwrap_or(index as i32)
                .max(0);
            let timestamp =
                (activity.start_date + Duration::seconds(i64::from(elapsed_seconds))).to_rfc3339();
            let distance = stream_f64_value(streams.distance.as_ref(), index)
                .or_else(|| interpolate_distance(index, trackpoint_count, distance_meters));

            xml.push_str("          <Trackpoint>\n");
            xml.push_str(&format!(
                "            <Time>{}</Time>\n",
                escape_xml_text(&timestamp)
            ));
            if let Some([latitude, longitude]) = stream_latlng_value(streams, index) {
                xml.push_str("            <Position>\n");
                xml.push_str(&format!(
                    "              <LatitudeDegrees>{:.7}</LatitudeDegrees>\n              <LongitudeDegrees>{:.7}</LongitudeDegrees>\n",
                    latitude, longitude
                ));
                xml.push_str("            </Position>\n");
            }
            if let Some(altitude) = stream_f64_value(streams.altitude.as_ref(), index) {
                xml.push_str(&format!(
                    "            <AltitudeMeters>{:.3}</AltitudeMeters>\n",
                    altitude
                ));
            }
            if let Some(distance) = distance {
                xml.push_str(&format!(
                    "            <DistanceMeters>{:.3}</DistanceMeters>\n",
                    distance
                ));
            }
            if let Some(heart_rate) = stream_i32_value(streams.heartrate.as_ref(), index) {
                xml.push_str(&format!(
                    "            <HeartRateBpm><Value>{}</Value></HeartRateBpm>\n",
                    heart_rate
                ));
            }
            if let Some(cadence) = stream_f64_value(streams.cadence.as_ref(), index) {
                xml.push_str(&format!(
                    "            <Cadence>{}</Cadence>\n",
                    cadence.round() as i32
                ));
            }
            if let Some(watts) = stream_i32_value(streams.watts.as_ref(), index) {
                xml.push_str("            <Extensions>\n");
                xml.push_str("              <ns3:TPX>\n");
                xml.push_str(&format!(
                    "                <ns3:Watts>{}</ns3:Watts>\n",
                    watts
                ));
                xml.push_str("              </ns3:TPX>\n");
                xml.push_str("            </Extensions>\n");
            }
            xml.push_str("          </Trackpoint>\n");
        }
        xml.push_str("        </Track>\n");
    }

    xml.push_str("      </Lap>\n    </Activity>\n  </Activities>\n</TrainingCenterDatabase>\n");
    xml
}

fn build_sync_summary_message(
    imported_count: i32,
    duplicate_count: i32,
    failed_count: i32,
) -> String {
    if imported_count == 0 && duplicate_count == 0 && failed_count == 0 {
        return "No new Strava activities found.".to_string();
    }

    let mut parts = Vec::new();
    if imported_count > 0 {
        parts.push(format!("Imported {imported_count}"));
    }
    if duplicate_count > 0 {
        parts.push(format!("Skipped {duplicate_count} duplicates"));
    }
    if failed_count > 0 {
        parts.push(format!("{failed_count} failed"));
    }

    if parts.is_empty() {
        "Strava sync finished.".to_string()
    } else {
        format!("{}.", parts.join(". "))
    }
}

fn sanitize_title_for_filename(title: &str, activity_id: i64) -> String {
    let sanitized = title
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("_");

    if sanitized.is_empty() {
        format!("strava_activity_{activity_id}")
    } else {
        format!("{sanitized}_{activity_id}")
    }
}

fn tcx_sport(activity: &StravaActivitySummary) -> &str {
    match activity
        .sport_type
        .as_deref()
        .or(activity.legacy_type.as_deref())
        .unwrap_or("Activity")
    {
        "Ride" | "VirtualRide" | "MountainBikeRide" | "GravelRide" | "EBikeRide"
        | "EMountainBikeRide" | "Velomobile" | "Handcycle" => "Ride",
        "Run" | "VirtualRun" | "TrailRun" | "Walk" | "Hike" => "Run",
        "Swim" => "Swim",
        other => other,
    }
}

fn max_trackpoint_count(streams: &StravaActivityStreams) -> usize {
    [
        streams
            .time
            .as_ref()
            .map(|stream| stream.data.len())
            .unwrap_or(0),
        streams
            .distance
            .as_ref()
            .map(|stream| stream.data.len())
            .unwrap_or(0),
        streams
            .latlng
            .as_ref()
            .map(|stream| stream.data.len())
            .unwrap_or(0),
        streams
            .altitude
            .as_ref()
            .map(|stream| stream.data.len())
            .unwrap_or(0),
        streams
            .heartrate
            .as_ref()
            .map(|stream| stream.data.len())
            .unwrap_or(0),
        streams
            .cadence
            .as_ref()
            .map(|stream| stream.data.len())
            .unwrap_or(0),
        streams
            .watts
            .as_ref()
            .map(|stream| stream.data.len())
            .unwrap_or(0),
    ]
    .into_iter()
    .max()
    .unwrap_or(0)
}

fn average_i32_stream(stream: Option<&StravaStream<i32>>) -> Option<i32> {
    let data = &stream?.data;
    if data.is_empty() {
        None
    } else {
        Some((data.iter().copied().sum::<i32>() as f64 / data.len() as f64).round() as i32)
    }
}

fn max_i32_stream(stream: Option<&StravaStream<i32>>) -> Option<i32> {
    stream?.data.iter().copied().max()
}

fn average_f64_stream(stream: Option<&StravaStream<f64>>) -> Option<f64> {
    let data = &stream?.data;
    if data.is_empty() {
        None
    } else {
        Some(data.iter().copied().sum::<f64>() / data.len() as f64)
    }
}

fn stream_time_value(streams: &StravaActivityStreams, index: usize) -> Option<i32> {
    stream_i32_value(streams.time.as_ref(), index)
}

fn stream_i32_value(stream: Option<&StravaStream<i32>>, index: usize) -> Option<i32> {
    stream.and_then(|stream| stream.data.get(index)).copied()
}

fn stream_f64_value(stream: Option<&StravaStream<f64>>, index: usize) -> Option<f64> {
    stream.and_then(|stream| stream.data.get(index)).copied()
}

fn stream_latlng_value(streams: &StravaActivityStreams, index: usize) -> Option<[f64; 2]> {
    streams
        .latlng
        .as_ref()
        .and_then(|stream| stream.data.get(index))
        .copied()
}

fn interpolate_elapsed_seconds(index: usize, count: usize, total_time_seconds: i32) -> Option<i32> {
    if count == 0 {
        return None;
    }

    if count == 1 || total_time_seconds <= 0 {
        return Some(0);
    }

    Some((((index as f64) / ((count - 1) as f64)) * f64::from(total_time_seconds)).round() as i32)
}

fn interpolate_distance(index: usize, count: usize, total_distance_meters: f64) -> Option<f64> {
    if count == 0 || total_distance_meters <= 0.0 {
        return None;
    }

    if count == 1 {
        return Some(total_distance_meters);
    }

    Some(((index as f64) / ((count - 1) as f64)) * total_distance_meters)
}

fn escape_xml_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
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

fn ensure_connection_scopes_allow_activity_import(
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
    use chrono::Duration as ChronoDuration;
    use sea_orm::{ActiveModelTrait, ConnectionTrait, Database, EntityTrait, QueryOrder, Schema};

    async fn test_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory db");

        let schema = Schema::new(db.get_database_backend());
        db.execute(&schema.create_table_from_entity(strava_connections::Entity))
            .await
            .expect("create Strava connections table");
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

    async fn load_event_types(db: &DatabaseConnection) -> Vec<String> {
        integration_events_entity::Entity::find()
            .order_by_asc(integration_events_entity::Column::Id)
            .all(db)
            .await
            .expect("load integration events")
            .into_iter()
            .map(|event| event.event_type)
            .collect()
    }

    fn test_config() -> Config {
        Config {
            database_url: "postgres://localhost/test".to_string(),
            frontend_url: "http://localhost:3001".to_string(),
            cors_allowed_origins: vec!["http://localhost:3001".to_string()],
            api_url: "http://localhost:3000".to_string(),
            strava_client_id: "12345".to_string(),
            strava_client_secret: "secret".to_string(),
            strava_oauth_scopes: "activity:read_all profile:read_all".to_string(),
            strava_webhook_verify_token: "verify-token".to_string(),
            strava_webhook_callback_url: None,
            uploads_dir: "./uploads".to_string(),
            max_upload_bytes: 1024,
            max_archive_fetch_bytes: 1024,
            archive_fetch_timeout_seconds: 60,
            jwt_secret: "test-secret".to_string(),
            auth_password_enabled: true,
            auth_registration_enabled: true,
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
    fn strava_sync_after_started_at_prefers_latest_existing_activity() {
        let last_synced_activity_started_at = DateTime::parse_from_rfc3339("2024-05-01T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let latest_user_activity_started_at = DateTime::parse_from_rfc3339("2026-05-11T13:23:17Z")
            .unwrap()
            .with_timezone(&Utc);

        assert_eq!(
            strava_sync_after_started_at(
                Some(last_synced_activity_started_at),
                Some(latest_user_activity_started_at),
            ),
            Some(latest_user_activity_started_at),
        );
    }

    #[test]
    fn strava_sync_after_started_at_keeps_newer_strava_cursor() {
        let last_synced_activity_started_at = DateTime::parse_from_rfc3339("2026-05-12T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let latest_user_activity_started_at = DateTime::parse_from_rfc3339("2026-05-11T13:23:17Z")
            .unwrap()
            .with_timezone(&Utc);

        assert_eq!(
            strava_sync_after_started_at(
                Some(last_synced_activity_started_at),
                Some(latest_user_activity_started_at),
            ),
            Some(last_synced_activity_started_at),
        );
    }

    #[test]
    fn parses_refresh_token_response_without_scope_or_athlete() {
        let response = serde_json::from_str::<StravaRefreshTokenResponse>(
            r#"{
                "access_token": "refreshed-access",
                "expires_at": 1760000000,
                "expires_in": 21600,
                "refresh_token": "refreshed-refresh"
            }"#,
        )
        .expect("parse refresh response");

        assert_eq!(response.access_token, "refreshed-access");
        assert_eq!(response.refresh_token, "refreshed-refresh");
        assert_eq!(response.expires_at, 1_760_000_000);
        assert_eq!(response.scope, None);
        assert!(response.athlete.is_none());
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
    fn builds_tcx_document_from_summary_and_streams() {
        let activity = StravaActivitySummary {
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
            legacy_type: Some("Ride".to_string()),
            start_date: DateTime::parse_from_rfc3339("2026-05-12T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        };
        let streams = StravaActivityStreams {
            time: Some(test_stream(vec![0, 160, 320])),
            distance: Some(test_stream(vec![0.0, 500.0, 1000.0])),
            latlng: Some(crate::strava_provider_payload::StravaLatLngStream {
                data: vec![[35.0, -82.0], [35.0005, -82.0005], [35.001, -82.001]],
                original_size: None,
                resolution: None,
                series_type: None,
            }),
            altitude: Some(test_stream(vec![700.0, 720.0, 725.0])),
            velocity_smooth: None,
            heartrate: Some(test_stream(vec![140, 145, 150])),
            cadence: Some(test_stream(vec![86.0, 88.0, 90.0])),
            watts: Some(test_stream(vec![205, 220, 235])),
            temp: None,
            moving: None,
            grade_smooth: None,
        };

        let tcx = build_tcx_document(&activity, &streams);

        assert!(tcx.contains("<Activity Sport=\"Ride\">"));
        assert!(tcx.contains("<DistanceMeters>1000.000</DistanceMeters>"));
        assert!(tcx.contains("<LatitudeDegrees>35.0000000</LatitudeDegrees>"));
        assert!(tcx.contains("<HeartRateBpm><Value>140</Value></HeartRateBpm>"));
        assert!(tcx.contains("<Cadence>86</Cadence>"));
        assert!(tcx.contains("<ns3:Watts>205</ns3:Watts>"));
    }

    #[test]
    fn builds_strava_import_payload_with_provider_artifact_and_generated_tcx() {
        let activity = StravaActivitySummary {
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

        assert_eq!(payload.generated_tcx_upload.format, "tcx");
        assert_eq!(
            payload.provider_payload_artifact.artifact_kind,
            ACTIVITY_IMPORT_ARTIFACT_KIND_PROVIDER_PAYLOAD
        );
        assert_eq!(
            payload.provider_payload_artifact.source_quality,
            ACTIVITY_IMPORT_SOURCE_QUALITY_STRAVA_STREAMS
        );
        let raw_json: serde_json::Value =
            serde_json::from_slice(&payload.provider_payload_artifact.bytes)
                .expect("provider payload json");
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
        let queue = TaskQueue::new(db.clone());

        queue
            .sync_strava_connection(connection.id)
            .await
            .expect("enqueue sync task");
        acquire_user_activity_import_lock(
            &db,
            connection.user_id,
            ACTIVITY_IMPORT_LOCK_SOURCE_STRAVA_SYNC,
            ACTIVITY_IMPORT_LOCK_STAGE_QUEUED,
        )
        .await
        .expect("acquire sync lock");

        disconnect_connection(&db, connection.user_id)
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
    async fn process_strava_sync_is_noop_when_connection_is_missing() {
        let db = test_db().await;

        process_strava_sync(&db, "/tmp", 999)
            .await
            .expect("missing connection should be treated as a no-op");
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

    #[tokio::test]
    async fn webhook_athlete_deauthorization_disconnects_connection_and_logs_events() {
        let db = test_db().await;
        let connection = insert_connection(&db, 9, STRAVA_SYNC_STATUS_NEVER).await;
        let tasks = TaskQueue::new(db.clone());

        handle_webhook_event(
            &db,
            &tasks,
            &StravaWebhookEvent {
                aspect_type: "update".to_string(),
                event_time: Utc::now().timestamp(),
                object_id: connection.athlete_id,
                object_type: "athlete".to_string(),
                owner_id: connection.athlete_id,
                subscription_id: 1,
                updates: Some(serde_json::json!({
                    "authorized": false,
                })),
            },
        )
        .await
        .expect("handle athlete revoke webhook");

        assert!(load_connection(&db, connection.user_id)
            .await
            .expect("load connection")
            .is_none());

        let event_types = load_event_types(&db).await;
        assert!(event_types.contains(&"webhook.received".to_string()));
        assert!(event_types.contains(&"webhook.athlete_deauthorized".to_string()));
        assert!(event_types.contains(&"disconnect.completed".to_string()));
    }

    #[tokio::test]
    async fn webhook_activity_update_queues_sync_and_logs_events() {
        let db = test_db().await;
        let connection = insert_connection(&db, 10, STRAVA_SYNC_STATUS_NEVER).await;
        let tasks = TaskQueue::new(db.clone());

        handle_webhook_event(
            &db,
            &tasks,
            &StravaWebhookEvent {
                aspect_type: "create".to_string(),
                event_time: Utc::now().timestamp(),
                object_id: 4_242,
                object_type: "activity".to_string(),
                owner_id: connection.athlete_id,
                subscription_id: 1,
                updates: None,
            },
        )
        .await
        .expect("handle activity update webhook");

        let queued_connection = load_connection(&db, connection.user_id)
            .await
            .expect("load connection")
            .expect("connection still exists");
        assert_eq!(
            queued_connection.last_sync_status,
            STRAVA_SYNC_STATUS_QUEUED
        );

        let task = background_tasks::Entity::find()
            .one(&db)
            .await
            .expect("load task")
            .expect("queued task exists");
        assert_eq!(task.status, background_tasks::TaskStatus::Pending.as_str());
        assert!(task_targets_connection(&task, connection.id));

        let event_types = load_event_types(&db).await;
        assert!(event_types.contains(&"webhook.received".to_string()));
        assert!(event_types.contains(&"sync.queued".to_string()));
    }

    #[tokio::test]
    async fn webhook_activity_update_without_required_scope_is_ignored() {
        let db = test_db().await;
        let connection =
            insert_connection_with_scopes(&db, 11, STRAVA_SYNC_STATUS_NEVER, "activity:read").await;
        let tasks = TaskQueue::new(db.clone());

        handle_webhook_event(
            &db,
            &tasks,
            &StravaWebhookEvent {
                aspect_type: "create".to_string(),
                event_time: Utc::now().timestamp(),
                object_id: 8_484,
                object_type: "activity".to_string(),
                owner_id: connection.athlete_id,
                subscription_id: 1,
                updates: None,
            },
        )
        .await
        .expect("handle activity update webhook with missing scope");

        assert!(background_tasks::Entity::find()
            .all(&db)
            .await
            .expect("load background tasks")
            .is_empty());

        let events = integration_events_entity::Entity::find()
            .order_by_asc(integration_events_entity::Column::Id)
            .all(&db)
            .await
            .expect("load integration events");
        assert!(events
            .iter()
            .any(|event| event.event_type == "webhook.activity_ignored"
                && event.message == missing_activity_scope_message()));
        assert!(!events.iter().any(|event| event.event_type == "sync.queued"));
    }

    #[tokio::test]
    async fn webhook_activity_delete_without_connection_logs_ignored_event() {
        let db = test_db().await;
        let tasks = TaskQueue::new(db.clone());

        handle_webhook_event(
            &db,
            &tasks,
            &StravaWebhookEvent {
                aspect_type: "delete".to_string(),
                event_time: Utc::now().timestamp(),
                object_id: 5_555,
                object_type: "activity".to_string(),
                owner_id: 999_999,
                subscription_id: 1,
                updates: None,
            },
        )
        .await
        .expect("handle activity delete webhook");

        let background_tasks = background_tasks::Entity::find()
            .order_by_asc(background_tasks::Column::Id)
            .all(&db)
            .await
            .expect("load queued tasks");
        assert!(background_tasks.is_empty());

        assert!(activities::Entity::find()
            .all(&db)
            .await
            .expect("load activities")
            .is_empty());
        assert!(analytics_user_states::Entity::find()
            .all(&db)
            .await
            .expect("load analytics state")
            .is_empty());

        let event_types = load_event_types(&db).await;
        assert!(event_types.contains(&"webhook.received".to_string()));
        assert!(event_types.contains(&"webhook.activity_ignored".to_string()));
    }
}
