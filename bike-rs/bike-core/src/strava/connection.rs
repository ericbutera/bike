use crate::activity_import_pipeline::ActivityUploadPayload;
use crate::config::Config;
use crate::entities::strava_connections;
use crate::integration_events_service::{
    self, NewIntegrationEvent, INTEGRATION_LEVEL_ERROR, INTEGRATION_LEVEL_SUCCESS, INTEGRATION_PROVIDER_STRAVA,
};
use crate::strava::client::{StravaApiClient, StravaAuthorizationTokenResponse};
use crate::strava::provider_payload::{StoredStravaProviderPayload, StravaActivityStreams, StravaActivitySummary};
use crate::workflow_error::WorkflowError as AppError;
#[cfg(test)]
use chrono::DateTime;
use chrono::{Duration, Utc};
use reqwest::Url;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const STRAVA_AUTHORIZE_URL: &str = "https://www.strava.com/oauth/authorize";
pub const STRAVA_SYNC_STATUS_NEVER: &str = "never";
pub const STRAVA_SYNC_STATUS_QUEUED: &str = "queued";
pub const STRAVA_SYNC_STATUS_RUNNING: &str = "running";
pub const STRAVA_SYNC_STATUS_FAILED: &str = "failed";
const STRAVA_REQUIRED_ACTIVITY_SCOPE: &str = "activity:read_all";

#[derive(Debug, Clone)]
pub struct ResolvedStravaConnectionSyncState {
    pub connection: strava_connections::Model,
    pub active_sync_status: Option<&'static str>,
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
    #[serde(default, flatten)]
    pub additional_fields: std::collections::BTreeMap<String, serde_json::Value>,
}

pub fn build_redirect_uri(config: &Config) -> String {
    format!("{}/api/strava/callback", config.api_url.trim_end_matches('/'))
}

pub fn build_authorization_url(config: &Config, state: &str) -> Result<Url, String> {
    let mut url = Url::parse(STRAVA_AUTHORIZE_URL).map_err(|error| format!("Invalid Strava authorize URL: {error}"))?;
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

pub struct StravaAuthorization {
    pub url: Url,
    pub browser_session: String,
}

pub async fn create_authorization_url_for_user(
    db: &DatabaseConnection,
    config: &Config,
    user_id: i32,
) -> Result<StravaAuthorization, AppError> {
    ensure_strava_configured(config)?;
    let state = super::oauth_state::OAuthState::new(&config.jwt_secret)
        .issue(db, user_id, Utc::now())
        .await?;
    Ok(StravaAuthorization {
        url: build_authorization_url(config, &state.token).map_err(AppError::internal)?,
        browser_session: state.browser_session,
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

pub fn build_frontend_account_redirect(config: &Config, status: &str, message: Option<&str>) -> String {
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
    strava_connections::Model::for_user(db, user_id)
        .await
        .map_err(AppError::from)
}

pub async fn exchange_code_for_connection(
    db: &DatabaseConnection,
    code: &str,
    verified_state: super::oauth_state::VerifiedStravaState,
) -> Result<strava_connections::Model, AppError> {
    let config = Config::get();
    ensure_strava_configured(config)?;

    let result = async {
        let client = StravaApiClient::new(db, config)?;
        let token_response = client.exchange_authorization_code(code).await?;

        if !scopes_allow_activity_import(&token_response.scope) {
            return Err(AppError::bad_request(missing_activity_scope_message()));
        }

        let connection = upsert_connection_from_token(db, verified_state.user_id, token_response).await?;

        crate::strava::events::request_sync(db, connection.user_id).await?;
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

pub async fn forget_local_connection(db: &DatabaseConnection, user_id: i32) -> Result<(), AppError> {
    let Some(connection) = load_connection(db, user_id).await? else {
        return Ok(());
    };
    connection.remove(db).await?;
    Ok(())
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
    strava_connections::Model::connect_from_token(db, user_id, token_response).await
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
    let token_response = archive.scope(client.refresh_access_token(&refresh_token)).await?;
    connection.refresh_credentials(db, token_response).await
}

pub(crate) fn build_activity_upload(
    activity: &StravaActivitySummary,
    streams: &StravaActivityStreams,
) -> Result<ActivityUploadPayload, AppError> {
    let bytes = serde_json::to_vec(&StoredStravaProviderPayload::new(activity.clone(), streams.clone()))
        .map_err(|error| AppError::internal(format!("Failed to serialize Strava activity {}: {error}", activity.id)))?;
    Ok(ActivityUploadPayload {
        original_filename: format!("strava_activity_{}.json", activity.id),
        format: "json".into(),
        mime_type: Some("application/json".into()),
        source_correlation_id: Some(activity.id.to_string()),
        bytes,
    })
}

pub async fn resolve_connection_sync_state(
    _db: &DatabaseConnection,
    connection: &strava_connections::Model,
) -> Result<ResolvedStravaConnectionSyncState, AppError> {
    Ok(ResolvedStravaConnectionSyncState {
        connection: connection.clone(),
        active_sync_status: None,
    })
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
    if !scopes.iter().any(|scope| scope == STRAVA_REQUIRED_ACTIVITY_SCOPE) {
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

#[cfg(test)]
mod tests {

    use super::*;
    use crate::strava::provider_payload::StravaStream;
    use axum::http::StatusCode;

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
            worker_recovery_timeout_seconds: 300,
            uploads_dir: "./uploads".to_string(),
            map_renderer_grpc_address: None,
            map_service_token: String::new(),
            map_image_cache_dir: "./map-image-cache".into(),
            map_image_cache_ttl_seconds: 604800,
            max_upload_bytes: 1024,
            max_archive_fetch_bytes: 1024,
            archive_fetch_timeout_seconds: 60,
            jwt_secret: "test-secret".to_string(),
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
    fn builds_authorization_url_with_expected_query() {
        let config = test_config();
        let url = build_authorization_url(&config, "signed-state").unwrap();

        assert_eq!(url.origin().unicode_serialization(), "https://www.strava.com");
        assert_eq!(url.path(), "/oauth/authorize");
        assert!(url.as_str().contains("client_id=12345"));
        assert!(url.as_str().contains("scope=activity%3Aread_all%2Cprofile%3Aread_all"));
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
        let raw_json: serde_json::Value = serde_json::from_slice(&payload.bytes).expect("provider payload json");
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
}
