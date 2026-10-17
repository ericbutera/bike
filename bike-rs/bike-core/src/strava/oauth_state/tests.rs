use super::*;
use std::{collections::HashSet, sync::Mutex};

#[derive(Default)]
struct StateStore {
    tokens: Mutex<HashSet<String>>,
    consumes: Mutex<usize>,
}

#[async_trait::async_trait]
impl OAuthStateStore for StateStore {
    async fn save(&self, state: &AuthorizationState) -> Result<(), WorkflowError> {
        self.tokens.lock().unwrap().insert(state.token.clone());
        Ok(())
    }

    async fn consume(&self, token: &str) -> Result<(), WorkflowError> {
        *self.consumes.lock().unwrap() += 1;
        if !self.tokens.lock().unwrap().remove(token) {
            return Err(invalid_state());
        }
        Ok(())
    }
}

fn now() -> DateTime<Utc> {
    DateTime::from_timestamp(1790856000, 0).unwrap()
}

#[derive(Default)]
struct Exchange {
    users: Mutex<Vec<i32>>,
    fail: bool,
}

#[async_trait::async_trait]
impl AuthorizationExchange for Exchange {
    async fn exchange(
        &self,
        code: &str,
        state: VerifiedStravaState,
    ) -> Result<strava_connections::Model, WorkflowError> {
        assert_eq!(code, "fixture-code");
        self.users.lock().unwrap().push(state.user_id);
        if self.fail {
            return Err(WorkflowError::internal("Fixture exchange failed"));
        }
        Ok(crate::strava::native::tests::fixture::Fixture::new().connection)
    }
}

fn callback(state: &AuthorizationState) -> AuthorizationCallback {
    AuthorizationCallback {
        code: Some("fixture-code".into()),
        state: Some(state.token.clone()),
        scope: None,
        error: None,
    }
}

#[tokio::test]
async fn valid_callback_exchanges_once_for_the_signed_bike_user() {
    let store = StateStore::default();
    let exchange = Exchange::default();
    let signer = OAuthState::new("fixture-secret");
    let state = signer.issue(&store, 7, now()).await.unwrap();
    let callback = callback(&state);
    let connected = signer
        .callback(&store, &exchange, &callback, &state.browser_session, now())
        .await
        .unwrap();
    assert_eq!(connected.user_id, 7);
    assert!(signer
        .callback(&store, &exchange, &callback, &state.browser_session, now())
        .await
        .is_err());
    assert_eq!(*exchange.users.lock().unwrap(), [7]);
}

#[tokio::test]
async fn invalid_state_or_browser_does_not_exchange_a_code() {
    let store = StateStore::default();
    let exchange = Exchange::default();
    let signer = OAuthState::new("fixture-secret");
    let state = signer.issue(&store, 7, now()).await.unwrap();
    let mut callback = callback(&state);
    assert!(signer
        .callback(&store, &exchange, &callback, "wrong-browser", now())
        .await
        .is_err());
    callback.state = Some("invalid".into());
    assert!(signer
        .callback(&store, &exchange, &callback, &state.browser_session, now())
        .await
        .is_err());
    assert!(exchange.users.lock().unwrap().is_empty());
    assert_eq!(*store.consumes.lock().unwrap(), 0);
}

#[tokio::test]
async fn denial_or_missing_code_consumes_valid_state_without_provider_exchange() {
    let signer = OAuthState::new("fixture-secret");
    for denied in [true, false] {
        let store = StateStore::default();
        let exchange = Exchange::default();
        let state = signer.issue(&store, 7, now()).await.unwrap();
        let mut callback = callback(&state);
        callback.code = None;
        callback.error = denied.then(|| "access_denied".into());
        assert!(signer
            .callback(&store, &exchange, &callback, &state.browser_session, now())
            .await
            .is_err());
        assert!(store.tokens.lock().unwrap().is_empty());
        assert!(exchange.users.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn failed_exchange_does_not_restore_the_consumed_state() {
    let store = StateStore::default();
    let exchange = Exchange {
        fail: true,
        ..Default::default()
    };
    let signer = OAuthState::new("fixture-secret");
    let state = signer.issue(&store, 7, now()).await.unwrap();
    let callback = callback(&state);
    assert!(signer
        .callback(&store, &exchange, &callback, &state.browser_session, now())
        .await
        .is_err());
    assert!(signer
        .callback(&store, &exchange, &callback, &state.browser_session, now())
        .await
        .is_err());
    assert_eq!(*exchange.users.lock().unwrap(), [7]);
}

#[tokio::test]
async fn persistence_adapter_rejects_the_callback_that_loses_one_time_consumption() {
    use sea_orm::{DatabaseBackend, MockDatabase, MockExecResult};
    let store = StateStore::default();
    let signer = OAuthState::new("fixture-secret");
    let state = signer.issue(&store, 7, now()).await.unwrap();
    let stored = oauth_states::Model {
        id: 1,
        state_hash: oauth_states::Model::hash(&state.token),
        provider: PROVIDER.into(),
        nonce: state.browser_session.clone(),
        expires_at: Utc::now() + Duration::minutes(10),
        created_at: Utc::now(),
    };
    for rows_affected in [0, 1] {
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([vec![stored.clone()]])
            .append_exec_results([MockExecResult {
                rows_affected,
                last_insert_id: 0,
            }])
            .into_connection();
        let result = signer.consume(&db, &state.token, &state.browser_session, now()).await;
        assert_eq!(result.is_ok(), rows_affected == 1);
        let log = format!("{:?}", db.into_transaction_log());
        assert!(log.contains("DELETE FROM"));
        assert!(log.contains("expires_at"));
        assert!(!log.contains(&state.token));
    }
}

#[tokio::test]
async fn accepts_the_initiating_browser_once() {
    let store = StateStore::default();
    let signer = OAuthState::new("fixture-secret");
    let state = signer.issue(&store, 7, now()).await.unwrap();
    let verified = signer
        .consume(&store, &state.token, &state.browser_session, now())
        .await
        .unwrap();
    assert_eq!(verified.user_id, 7);
    assert_eq!(verified.issued_at, now());
    assert_eq!(verified.nonce, state.browser_session);
    assert!(signer
        .consume(&store, &state.token, &state.browser_session, now())
        .await
        .is_err());
}

#[tokio::test]
async fn concurrent_callbacks_have_one_winner() {
    let store = StateStore::default();
    let signer = OAuthState::new("fixture-secret");
    let state = signer.issue(&store, 7, now()).await.unwrap();
    let (first, second) = tokio::join!(
        signer.consume(&store, &state.token, &state.browser_session, now()),
        signer.consume(&store, &state.token, &state.browser_session, now()),
    );
    assert_ne!(first.is_ok(), second.is_ok());
}

#[tokio::test]
async fn wrong_or_missing_browser_cannot_consume_a_valid_state() {
    let store = StateStore::default();
    let signer = OAuthState::new("fixture-secret");
    let state = signer.issue(&store, 7, now()).await.unwrap();
    for browser in ["", "another-browser"] {
        assert!(signer.consume(&store, &state.token, browser, now()).await.is_err());
    }
    assert_eq!(*store.consumes.lock().unwrap(), 0);
    assert!(signer
        .consume(&store, &state.token, &state.browser_session, now())
        .await
        .is_ok());
}

#[tokio::test]
async fn rejects_every_tampered_field_before_persistence() {
    let store = StateStore::default();
    let signer = OAuthState::new("fixture-secret");
    let state = signer.issue(&store, 7, now()).await.unwrap();
    let parts: Vec<_> = state.token.split(':').collect();
    for (index, value) in [(0, "8"), (1, "1790856001"), (2, "different"), (3, "00")] {
        let mut tampered = parts.clone();
        tampered[index] = value;
        assert!(signer
            .consume(&store, &tampered.join(":"), &state.browser_session, now())
            .await
            .is_err());
    }
    assert!(OAuthState::new("wrong-key")
        .consume(&store, &state.token, &state.browser_session, now())
        .await
        .is_err());
    assert_eq!(*store.consumes.lock().unwrap(), 0);
}

#[tokio::test]
async fn rejects_expiration_boundary_future_and_malformed_tokens() {
    let store = StateStore::default();
    let signer = OAuthState::new("fixture-secret");
    let state = signer.issue(&store, 7, now()).await.unwrap();
    for at in [
        now() - Duration::seconds(1),
        now() + Duration::seconds(STATE_TTL_SECONDS),
    ] {
        assert!(signer
            .consume(&store, &state.token, &state.browser_session, at)
            .await
            .is_err());
    }
    for token in ["", "7", "7:bad:nonce:xyz", "7:0:nonce:00:extra"] {
        assert!(signer
            .consume(&store, token, &state.browser_session, now())
            .await
            .is_err());
    }
    assert_eq!(*store.consumes.lock().unwrap(), 0);
    assert!(signer
        .consume(
            &store,
            &state.token,
            &state.browser_session,
            now() + Duration::seconds(599)
        )
        .await
        .is_ok());
}

#[tokio::test]
async fn issuing_again_rotates_the_browser_binding() {
    let store = StateStore::default();
    let signer = OAuthState::new("fixture-secret");
    let first = signer.issue(&store, 7, now()).await.unwrap();
    let second = signer.issue(&store, 7, now()).await.unwrap();
    assert_ne!(first.token, second.token);
    assert!(signer
        .consume(&store, &first.token, &second.browser_session, now())
        .await
        .is_err());
    assert!(signer
        .consume(&store, &second.token, &second.browser_session, now())
        .await
        .is_ok());
}

#[test]
fn browser_cookie_is_private_and_covers_both_callback_aliases() {
    let cookie = session_cookie("fixture", "https://bike.example.test");
    assert!(cookie.starts_with("__Host-strava_oauth_session="));
    for attribute in ["HttpOnly", "Path=/;", "SameSite=Lax", "Max-Age=600", "; Secure"] {
        assert!(cookie.contains(attribute));
    }
    assert!(!cookie.contains("Domain="));
    assert!(!session_cookie("fixture", "http://localhost:3000").contains("Secure"));
    assert!(clear_session_cookie("https://bike.example.test").contains("Max-Age=0"));
}

#[test]
fn production_requires_the_host_cookie_and_ignores_an_unprefixed_cookie() {
    use axum::http::header::COOKIE;
    let mut headers = HeaderMap::new();
    headers.insert(COOKIE, "strava_oauth_session=attacker".parse().unwrap());
    assert!(browser_session(&headers, "https://bike.example.test").is_err());
    headers.append(COOKIE, "__Host-strava_oauth_session=fixture".parse().unwrap());
    assert_eq!(
        browser_session(&headers, "https://bike.example.test").unwrap(),
        "fixture"
    );
}

#[test]
fn reads_cookie_headers_and_rejects_missing_empty_and_duplicate_binding() {
    use axum::http::header::COOKIE;
    let mut headers = HeaderMap::new();
    assert!(browser_session(&headers, "http://localhost:3000").is_err());
    headers.append(COOKIE, "other=value; strava_oauth_session=fixture".parse().unwrap());
    assert_eq!(browser_session(&headers, "http://localhost:3000").unwrap(), "fixture");
    headers.append(COOKIE, "strava_oauth_session=second".parse().unwrap());
    assert!(browser_session(&headers, "http://localhost:3000").is_err());
    headers.clear();
    headers.insert(COOKIE, "strava_oauth_session=".parse().unwrap());
    assert!(browser_session(&headers, "http://localhost:3000").is_err());
}
