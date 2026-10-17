use super::*;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use bike_core::{
    auth::entities::oauth_states,
    entities::{integration_events as audit, strava_events},
    workflow_error::WorkflowError,
};
use integrations::strava::{
    events::{StravaEvent, StravaOperation},
    oauth_state::*,
};
use sea_orm::{DatabaseBackend, MockDatabase, MockExecResult};
use serde_json::{json, Value};
use tower::ServiceExt;

use bike_core::test_support as fixture;

fn config() -> Config {
    let mut config = Config::get().clone();
    config.strava_client_id = "fixture-client".into();
    config.strava_client_secret = "fixture-secret".into();
    config.strava_webhook_verify_token = "fixture-verify".into();
    config.jwt_secret = "fixture-signing-key".into();
    config.api_url = "https://bike.example.test".into();
    config.frontend_url = "https://bike.example.test".into();
    config
}

fn audit() -> audit::Model {
    fixture::integration_event()
}

fn state(db: sea_orm::DatabaseConnection) -> Arc<AppStorage> {
    let mut state = AppStorage::for_test(db);
    state.local_admin_user_pid = Some(fixture::customer().pid);
    Arc::new(state)
}

fn acceptance(operation: StravaOperation) -> strava_events::Model {
    let event = StravaEvent::new(&fixture::customer(), &fixture::connection(), operation);
    fixture::strava_event_record(&event)
}

fn acceptance_mock(operation: StravaOperation) -> MockDatabase {
    MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixture::customer()]])
        .append_query_results([vec![fixture::connection()]])
        .append_query_results([vec![acceptance(operation)]])
        .append_query_results([vec![audit()]])
        .append_exec_results([MockExecResult {
            rows_affected: 1,
            last_insert_id: 0,
        }])
}

async fn request(db: MockDatabase, method: &str, path: &str, body: Value) -> Response {
    request_with_state(state(db.into_connection()), method, path, body).await
}

async fn request_with_state(state: Arc<AppStorage>, method: &str, path: &str, body: Value) -> Response {
    routes()
        .with_state(state)
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn json_body(response: Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

#[tokio::test]
async fn connection_response_exposes_saved_customer_sync_details_and_disconnected_defaults() {
    let connected = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixture::customer()]])
        .append_query_results([vec![fixture::connection()]]);
    let response = request(connected, "GET", "/connection", Value::Null).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["athlete_id"], 123);
    assert_eq!(body["athlete_name"], "Fixture Rider");
    assert_eq!(body["scopes"], json!(["read", "activity:read_all"]));
    let disconnected = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixture::customer()]])
        .append_query_results([Vec::<strava_connections::Model>::new()]);
    let body = json_body(request(disconnected, "GET", "/connection", Value::Null).await).await;
    assert_eq!(body["connected"], false);
    assert_eq!(body["last_sync_status"], "never");
}

#[tokio::test]
async fn sync_and_backfill_accept_durable_events_and_return_the_connection() {
    for (operation, path, body) in collection_requests() {
        let state = state(acceptance_mock(operation).into_connection());
        let response = request_with_state(state.clone(), "POST", path, body).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            json_body(response).await["athlete_id"],
            fixture::connection().athlete_id
        );
        let log = state.db.clone().into_transaction_log();
        let connection_reads = log
            .iter()
            .flat_map(|transaction| transaction.statements())
            .filter(|statement| statement.sql.contains("FROM \"strava_connections\""))
            .count();
        assert_eq!(connection_reads, 1);
        assert!(format!("{log:?}").contains("COMMIT"));
    }
}

fn collection_requests() -> [(StravaOperation, &'static str, Value); 2] {
    [
        (StravaOperation::Sync, "/sync", Value::Null),
        (
            StravaOperation::Backfill,
            "/backfill",
            json!({"start_date":"2026-09-01T00:00:00Z","end_date":"2026-10-01T00:00:00Z"}),
        ),
    ]
}

#[tokio::test]
async fn connection_extractors_preserve_authentication_for_reads_and_commands() {
    for (method, path, body) in [
        ("GET", "/connection", Value::Null),
        ("POST", "/sync", Value::Null),
        (
            "POST",
            "/backfill",
            json!({"start_date":"2026-09-01T00:00:00Z","end_date":"2026-10-01T00:00:00Z"}),
        ),
        ("DELETE", "/connection", Value::Null),
    ] {
        let storage = Arc::new(AppStorage::for_test(
            MockDatabase::new(DatabaseBackend::Postgres).into_connection(),
        ));
        let response = request_with_state(storage.clone(), method, path, body).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(storage.db.clone().into_transaction_log().is_empty());
    }
}

#[tokio::test]
async fn connection_extractor_rejects_missing_connections_before_accepting_commands() {
    for (method, path, body) in [
        ("POST", "/sync", Value::Null),
        (
            "POST",
            "/backfill",
            json!({"start_date":"2026-09-01T00:00:00Z","end_date":"2026-10-01T00:00:00Z"}),
        ),
        ("DELETE", "/connection", Value::Null),
    ] {
        let storage = state(
            MockDatabase::new(DatabaseBackend::Postgres)
                .append_query_results([vec![fixture::customer()]])
                .append_query_results([Vec::<strava_connections::Model>::new()])
                .into_connection(),
        );
        let response = request_with_state(storage.clone(), method, path, body).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(!format!("{:?}", storage.db.clone().into_transaction_log()).contains("INSERT INTO"));
    }
}

#[tokio::test]
async fn disconnect_accepts_an_event_without_calling_a_remote_provider() {
    let response = request(
        acceptance_mock(StravaOperation::Disconnect),
        "DELETE",
        "/connection",
        Value::Null,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_body(response).await["message"], "Strava disconnect queued.");
}

#[tokio::test]
async fn connection_removed_after_acceptance_does_not_reject_sync_or_backfill() {
    for (operation, path, body) in collection_requests() {
        let db = acceptance_mock(operation).append_query_results([Vec::<strava_connections::Model>::new()]);
        let response = request(db, "POST", path, body).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            json_body(response).await["athlete_id"],
            fixture::connection().athlete_id
        );
    }
}

#[tokio::test]
async fn connection_read_failure_after_acceptance_does_not_reject_sync_or_backfill() {
    for (operation, path, body) in collection_requests() {
        let db = acceptance_mock(operation)
            .append_query_errors([sea_orm::DbErr::Custom("connection became unavailable".into())]);
        let response = request(db, "POST", path, body).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            json_body(response).await["athlete_id"],
            fixture::connection().athlete_id
        );
    }
}

#[tokio::test]
async fn webhook_persists_the_request_id_before_acknowledgement() {
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixture::connection()]])
        .append_query_results([vec![fixture::customer()]])
        .append_query_results([vec![acceptance(StravaOperation::Create)]])
        .append_query_results([vec![audit()]])
        .append_exec_results([MockExecResult {
            rows_affected: 1,
            last_insert_id: 0,
        }])
        .into_connection();
    let state = state(db);
    let webhook = json!({"aspect_type":"create","object_type":"activity","object_id":42,"owner_id":123,"event_time":1790856000,"subscription_id":1,"updates":{}});
    let response = routes()
        .with_state(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhook")
                .header("x-request-id", "fixture-api-request")
                .body(Body::from(webhook.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let log = format!("{:?}", state.db.clone().into_transaction_log());
    assert!(log.contains("fixture-api-request"));
    assert!(log.contains("COMMIT"));
}

#[tokio::test]
async fn missing_connection_invalid_window_and_persistence_errors_are_http_errors() {
    let missing = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixture::customer()]])
        .append_query_results([Vec::<strava_connections::Model>::new()]);
    assert_eq!(
        request(missing, "POST", "/sync", Value::Null).await.status(),
        StatusCode::NOT_FOUND
    );
    let invalid = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixture::customer()]])
        .append_query_results([vec![fixture::connection()]]);
    let dates = json!({"start_date":"2026-10-01T00:00:00Z","end_date":"2026-09-01T00:00:00Z"});
    assert_eq!(
        request(invalid, "POST", "/backfill", dates).await.status(),
        StatusCode::BAD_REQUEST
    );
    let failure = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixture::customer()]])
        .append_query_errors([sea_orm::DbErr::Custom("fixture persistence failed".into())]);
    assert_eq!(
        request(failure, "DELETE", "/connection", Value::Null).await.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn malformed_webhook_is_rejected_and_unknown_athlete_is_acknowledged() {
    let db = MockDatabase::new(DatabaseBackend::Postgres);
    assert_eq!(
        request(db, "POST", "/webhook", json!({"invalid":true})).await.status(),
        StatusCode::BAD_REQUEST
    );
    let db =
        MockDatabase::new(DatabaseBackend::Postgres).append_query_results([Vec::<strava_connections::Model>::new()]);
    let webhook = json!({"aspect_type":"create","object_type":"activity","object_id":42,"owner_id":123,"event_time":1790856000,"subscription_id":1,"updates":{}});
    let response = request(db, "POST", "/webhook", webhook).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_body(response).await["message"], "ok");
}

#[tokio::test]
async fn public_aliases_reach_native_handlers_with_no_running_services() {
    for path in ["/api/strava/webhook", "/webhooks/strava-gateway"] {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let response = crate::controllers::routes()
            .with_state(state(db))
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .body(Body::from("invalid JSON"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    for path in ["/api/strava/callback", "/oauth/gateway/callback"] {
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([vec![audit()]])
            .into_connection();
        let response = crate::controllers::routes()
            .with_state(state(db))
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert!(response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .contains("Max-Age=0"));
    }
}

#[tokio::test]
async fn subscription_challenge_is_returned_and_invalid_token_is_rejected() {
    for (token, valid) in [("fixture-verify", true), ("wrong", false)] {
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([vec![audit()]])
            .into_connection();
        let query = strava::StravaWebhookSubscriptionQuery {
            mode: Some("subscribe".into()),
            challenge: Some("fixture-challenge".into()),
            verify_token: Some(token.into()),
        };
        let response = verify_webhook_response(&db, &config(), query).await;
        assert_eq!(response.is_ok(), valid);
        if valid {
            assert_eq!(response.unwrap().0.challenge, "fixture-challenge");
        }
    }
}

fn oauth_row() -> oauth_states::Model {
    let now = chrono::Utc::now();
    oauth_states::Model {
        id: 1,
        state_hash: "fixture".into(),
        provider: "strava".into(),
        nonce: "fixture".into(),
        expires_at: now + chrono::Duration::minutes(10),
        created_at: now,
    }
}

#[tokio::test]
async fn authorization_sets_secure_browser_cookie_and_signs_the_same_url_state() {
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![oauth_row()]])
        .append_query_results([vec![audit()]])
        .append_exec_results([MockExecResult {
            rows_affected: 1,
            last_insert_id: 0,
        }])
        .into_connection();
    let config = config();
    let response = authorize_response(&db, &config, 7).await.unwrap();
    let cookie = response.headers()[header::SET_COOKIE].to_str().unwrap().to_owned();
    assert!(cookie.starts_with("__Host-strava_oauth_session="));
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Lax"));
    assert!(cookie.ends_with("; Secure"));
    let url = json_body(response).await["authorization_url"]
        .as_str()
        .unwrap()
        .to_owned();
    let url = reqwest::Url::parse(&url).unwrap();
    let state = url
        .query_pairs()
        .find(|(key, _)| key == "state")
        .unwrap()
        .1
        .into_owned();
    let browser = cookie.split(';').next().unwrap().split('=').nth(1).unwrap();
    assert!(state.contains(browser));
    assert!(format!("{:?}", db.into_transaction_log()).contains("oauth.connect_started"));
}

struct Exchange {
    fail: bool,
}
#[async_trait::async_trait]
impl AuthorizationExchange for Exchange {
    async fn exchange(
        &self,
        code: &str,
        verified: VerifiedStravaState,
    ) -> Result<strava_connections::Model, WorkflowError> {
        assert_eq!(code, "fixture-code");
        assert_eq!(verified.user_id, 7);
        if self.fail {
            Err(WorkflowError::bad_request("fixture exchange rejected"))
        } else {
            Ok(fixture::connection())
        }
    }
}

struct StateStore;
#[async_trait::async_trait]
impl OAuthStateStore for StateStore {
    async fn save(&self, _: &AuthorizationState) -> Result<(), WorkflowError> {
        Ok(())
    }
    async fn consume(&self, _: &str) -> Result<(), WorkflowError> {
        panic!("State consumption uses mocked persistence")
    }
}

#[tokio::test]
async fn callback_consumes_browser_state_clears_cookie_and_redirects_for_success_or_exchange_error() {
    let config = config();
    for fail in [false, true] {
        let issued = OAuthState::new(&config.jwt_secret)
            .issue(&StateStore, 7, chrono::Utc::now())
            .await
            .unwrap();
        let query = StravaCallbackQuery {
            code: Some("fixture-code".into()),
            scope: None,
            state: Some(issued.token),
            error: None,
        };
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            format!("__Host-strava_oauth_session={}", issued.browser_session)
                .parse()
                .unwrap(),
        );
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([vec![oauth_row()]])
            .append_exec_results([MockExecResult {
                rows_affected: 1,
                last_insert_id: 0,
            }])
            .into_connection();
        let response = callback_response(&db, &config, &Exchange { fail }, &query, &headers).await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        let location = response.headers()[header::LOCATION].to_str().unwrap();
        assert!(location.contains(if fail { "strava=error" } else { "strava=connected" }));
        assert!(response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .contains("Max-Age=0"));
        assert!(db
            .into_transaction_log()
            .iter()
            .flat_map(|transaction| transaction.statements())
            .any(|statement| statement.sql.starts_with("DELETE FROM \"oauth_states\"")));
    }
}

#[tokio::test]
async fn disabled_configuration_records_connect_failure_without_provider_work() {
    let mut config = config();
    config.strava_client_secret.clear();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![audit()]])
        .into_connection();
    assert_eq!(
        authorize_response(&db, &config, 7).await.unwrap_err().status,
        StatusCode::BAD_REQUEST
    );
    assert!(format!("{:?}", db.into_transaction_log()).contains("oauth.connect_failed"));
}
