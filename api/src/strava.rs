use crate::activity_import_lock::{
    acquire_user_activity_import_lock, mark_user_activity_import_lock_stage,
    release_user_activity_import_lock, ACTIVITY_IMPORT_LOCK_SOURCE_STRAVA_SYNC,
    ACTIVITY_IMPORT_LOCK_STAGE_QUEUED, ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
};
use crate::activity_import_pipeline::{
    finalize_activity_import_batch, persist_activity_upload, ActivityUploadPayload,
    PersistActivityUploadOutcome,
};
use crate::app_error::AppError;
use crate::config::Config;
use crate::entities::strava_connections;
use crate::tasks::TaskQueue;
use crate::training_profile::load_training_profile;
use axum::http::StatusCode;
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use kaleido::auth::entities::users;
use reqwest::Client;
use reqwest::Url;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use sha2::Sha256;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

pub const STRAVA_AUTHORIZE_URL: &str = "https://www.strava.com/oauth/authorize";
pub const STRAVA_TOKEN_URL: &str = "https://www.strava.com/api/v3/oauth/token";
pub const STRAVA_DEAUTHORIZE_URL: &str = "https://www.strava.com/oauth/deauthorize";
pub const STRAVA_API_BASE_URL: &str = "https://www.strava.com/api/v3";
pub const STRAVA_SYNC_STATUS_NEVER: &str = "never";
pub const STRAVA_SYNC_STATUS_QUEUED: &str = "queued";
pub const STRAVA_SYNC_STATUS_RUNNING: &str = "running";
pub const STRAVA_SYNC_STATUS_SUCCEEDED: &str = "succeeded";
pub const STRAVA_SYNC_STATUS_FAILED: &str = "failed";
const STRAVA_STATE_MAX_AGE_MINUTES: i64 = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedStravaState {
    pub user_id: i32,
    pub issued_at: DateTime<Utc>,
    pub nonce: String,
}

#[derive(Debug, Deserialize)]
struct StravaTokenResponse {
    access_token: String,
    refresh_token: String,
    expires_at: i64,
    scope: String,
    athlete: StravaAthleteSummary,
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
struct StravaActivitySummary {
    id: i64,
    name: String,
    distance: Option<f64>,
    moving_time: Option<i32>,
    elapsed_time: Option<i32>,
    max_speed: Option<f64>,
    average_heartrate: Option<f64>,
    max_heartrate: Option<f64>,
    average_cadence: Option<f64>,
    calories: Option<f64>,
    sport_type: Option<String>,
    #[serde(rename = "type")]
    legacy_type: Option<String>,
    start_date: DateTime<Utc>,
}

#[derive(Debug, Default, Deserialize)]
struct StravaActivityStreams {
    time: Option<StravaStream<i32>>,
    distance: Option<StravaStream<f64>>,
    latlng: Option<StravaLatLngStream>,
    altitude: Option<StravaStream<f64>>,
    velocity_smooth: Option<StravaStream<f64>>,
    heartrate: Option<StravaStream<i32>>,
    cadence: Option<StravaStream<f64>>,
}

#[derive(Debug, Deserialize)]
struct StravaStream<T> {
    data: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct StravaLatLngStream {
    data: Vec<[f64; 2]>,
}

#[derive(Debug, Deserialize)]
struct StravaFault {
    message: Option<String>,
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

    url.query_pairs_mut()
        .append_pair("client_id", config.strava_client_id.trim())
        .append_pair("redirect_uri", &build_redirect_uri(config))
        .append_pair("response_type", "code")
        .append_pair("approval_prompt", "auto")
        .append_pair("scope", &config.strava_oauth_scope_list().join(","))
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

    build_authorization_url(config, &state).map_err(|message| AppError::internal(message))
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

pub fn scopes_allow_activity_read(scopes: &str) -> bool {
    scopes
        .split([' ', ','])
        .map(str::trim)
        .any(|scope| matches!(scope, "activity:read" | "activity:read_all"))
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

    let verified_state = verify_state_token(config, state_token).map_err(AppError::bad_request)?;
    let client = StravaApiClient::new(config)?;
    let token_response = client.exchange_authorization_code(code).await?;

    if !scopes_allow_activity_read(&token_response.scope) {
        return Err(AppError::bad_request(
            "Strava did not grant activity access. Reconnect and approve activity:read.",
        ));
    }

    let connection =
        upsert_connection_from_token(db, verified_state.user_id, token_response).await?;

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

pub async fn queue_connection_sync(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    user_id: i32,
) -> Result<strava_connections::Model, AppError> {
    ensure_strava_configured(Config::get())?;
    let connection = load_connection(db, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("Connect Strava before starting a sync"))?;

    if matches!(
        connection.last_sync_status.as_str(),
        STRAVA_SYNC_STATUS_QUEUED | STRAVA_SYNC_STATUS_RUNNING
    ) {
        return Ok(connection);
    }

    queue_sync_task_for_connection(db, tasks, &connection, "Strava sync queued.").await
}

pub async fn disconnect_connection(db: &DatabaseConnection, user_id: i32) -> Result<(), AppError> {
    let Some(connection) = load_connection(db, user_id).await? else {
        return Ok(());
    };

    if matches!(
        connection.last_sync_status.as_str(),
        STRAVA_SYNC_STATUS_QUEUED | STRAVA_SYNC_STATUS_RUNNING
    ) {
        return Err(AppError::conflict(
            "Wait for the queued Strava sync to finish before disconnecting Strava",
        ));
    }

    if let Ok(client) = StravaApiClient::new(Config::get()) {
        if let Err(error) = client.deauthorize(&connection.access_token).await {
            tracing::warn!(message = %error.message, "failed to deauthorize Strava connection before delete");
        }
    }

    strava_connections::Entity::delete_by_id(connection.id)
        .exec(db)
        .await?;

    Ok(())
}

pub async fn process_strava_sync(
    db: &DatabaseConnection,
    uploads_dir: &str,
    connection_id: i32,
) -> Result<(), AppError> {
    let connection = strava_connections::Entity::find_by_id(connection_id)
        .one(db)
        .await?
        .ok_or_else(|| {
            AppError::not_found(format!("Strava connection {connection_id} was not found"))
        })?;
    mark_user_activity_import_lock_stage(
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
        let user = users::Entity::find_by_id(connection.user_id)
            .one(db)
            .await?
            .ok_or_else(|| AppError::internal(format!("User {} for Strava sync was not found", connection.user_id)))?;
        let user_storage_key = user.pid.to_string();
        let tasks = TaskQueue::new(db.clone());
        let training_profile = load_training_profile(db, connection.user_id).await?;
        let after_epoch = connection
            .last_synced_activity_started_at
            .map(|timestamp| (timestamp - Duration::minutes(5)).timestamp());

        let mut page = 1usize;
        let mut imported_count = 0i32;
        let mut duplicate_count = 0i32;
        let mut failed_count = 0i32;
        let mut affected_segment_ids = Vec::new();
        let mut latest_started_at = connection.last_synced_activity_started_at;

        loop {
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

                let upload = match build_activity_upload(activity, &streams) {
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

                match persist_activity_upload(
                    db,
                    uploads_dir,
                    &user_storage_key,
                    connection.user_id,
                    upload,
                    "strava_sync",
                    Some(&training_profile),
                )
                .await
                {
                    Ok(PersistActivityUploadOutcome::Imported(persisted)) => {
                        imported_count += 1;
                        affected_segment_ids.extend(persisted.affected_segment_ids);
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

        if imported_count > 0 {
            finalize_activity_import_batch(
                db,
                &tasks,
                connection.user_id,
                affected_segment_ids,
                Utc::now(),
            )
            .await?;
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
    ) -> Result<StravaTokenResponse, AppError> {
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
    ) -> Result<StravaTokenResponse, AppError> {
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
                        "time,distance,latlng,altitude,velocity_smooth,heartrate,cadence",
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

fn ensure_strava_configured(config: &Config) -> Result<(), AppError> {
    if config.strava_enabled() {
        Ok(())
    } else {
        Err(AppError::bad_request(
            "Strava integration is not configured on this Bike deployment",
        ))
    }
}

async fn upsert_connection_from_token(
    db: &DatabaseConnection,
    user_id: i32,
    token_response: StravaTokenResponse,
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
    let mut active_model: strava_connections::ActiveModel = connection.into();
    active_model.scopes = Set(token_response.scope);
    active_model.access_token = Set(token_response.access_token);
    active_model.refresh_token = Set(token_response.refresh_token);
    active_model.expires_at = Set(expires_at);
    active_model.athlete_username = Set(token_response.athlete.username);
    active_model.athlete_first_name = Set(token_response.athlete.firstname);
    active_model.athlete_last_name = Set(token_response.athlete.lastname);
    active_model.athlete_profile_medium_url = Set(token_response.athlete.profile_medium);

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
    active_model.update(db).await.map_err(AppError::from)
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
    active_model.update(db).await.map_err(AppError::from)
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
    active_model.update(db).await.map_err(AppError::from)
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
    active_model.update(db).await.map_err(AppError::from)
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

fn build_activity_upload(
    activity: &StravaActivitySummary,
    streams: &StravaActivityStreams,
) -> Result<ActivityUploadPayload, AppError> {
    let original_filename = format!(
        "{}.tcx",
        sanitize_title_for_filename(&activity.name, activity.id)
    );
    let bytes = build_tcx_document(activity, streams).into_bytes();

    Ok(ActivityUploadPayload {
        original_filename,
        format: "tcx".to_string(),
        mime_type: Some("application/vnd.garmin.tcx+xml".to_string()),
        bytes,
    })
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
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<TrainingCenterDatabase xmlns=\"http://www.garmin.com/xmlschemas/TrainingCenterDatabase/v2\">\n  <Activities>\n",
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

fn sign_state(config: &Config, payload: &str) -> Result<String, String> {
    let mut mac = HmacSha256::new_from_slice(config.jwt_secret.as_bytes())
        .map_err(|_| "Invalid JWT secret for Strava state signing".to_string())?;
    mac.update(payload.as_bytes());

    Ok(hex::encode(mac.finalize().into_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Config {
        Config {
            database_url: "postgres://localhost/test".to_string(),
            frontend_url: "http://localhost:3001".to_string(),
            cors_allowed_origins: vec!["http://localhost:3001".to_string()],
            api_url: "http://localhost:3000".to_string(),
            strava_client_id: "12345".to_string(),
            strava_client_secret: "secret".to_string(),
            strava_oauth_scopes: "activity:read profile:read_all".to_string(),
            uploads_dir: "./uploads".to_string(),
            max_upload_bytes: 1024,
            max_archive_fetch_bytes: 1024,
            archive_fetch_timeout_seconds: 60,
            jwt_secret: "test-secret".to_string(),
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
            .contains("scope=activity%3Aread%2Cprofile%3Aread_all"));
        assert!(url.as_str().contains("state=signed-state"));
    }

    #[test]
    fn recognizes_activity_read_scope_variants() {
        assert!(scopes_allow_activity_read("activity:read"));
        assert!(scopes_allow_activity_read("read activity:read_all"));
        assert!(!scopes_allow_activity_read("read profile:read_all"));
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
            time: Some(StravaStream {
                data: vec![0, 160, 320],
            }),
            distance: Some(StravaStream {
                data: vec![0.0, 500.0, 1000.0],
            }),
            latlng: Some(StravaLatLngStream {
                data: vec![[35.0, -82.0], [35.0005, -82.0005], [35.001, -82.001]],
            }),
            altitude: Some(StravaStream {
                data: vec![700.0, 720.0, 725.0],
            }),
            velocity_smooth: None,
            heartrate: Some(StravaStream {
                data: vec![140, 145, 150],
            }),
            cadence: Some(StravaStream {
                data: vec![86.0, 88.0, 90.0],
            }),
        };

        let tcx = build_tcx_document(&activity, &streams);

        assert!(tcx.contains("<Activity Sport=\"Ride\">"));
        assert!(tcx.contains("<DistanceMeters>1000.000</DistanceMeters>"));
        assert!(tcx.contains("<LatitudeDegrees>35.0000000</LatitudeDegrees>"));
        assert!(tcx.contains("<HeartRateBpm><Value>140</Value></HeartRateBpm>"));
        assert!(tcx.contains("<Cadence>86</Cadence>"));
    }
}
