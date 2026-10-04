use super::entities::{oauth_states, refresh_tokens, users};
use super::services::oauth::OAuthUserInfo;
use super::tokens::verify_access_token;
use super::{OAuthService, SessionService};
use chrono::{Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, EntityTrait, Schema, Set,
};
use std::sync::Once;
use uuid::Uuid;

struct TestAuthStorage {
    db: DatabaseConnection,
    pid: Option<Uuid>,
    session: SessionService,
}

impl super::extractors::AuthStorage for TestAuthStorage {
    fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    fn jwt_secret(&self) -> &str {
        "session-test-secret"
    }

    fn local_admin_user_pid(&self) -> Option<Uuid> {
        self.pid
    }
}

impl super::controllers::auth::AuthRouteStorage for TestAuthStorage {
    fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    fn session_service(&self) -> &SessionService {
        &self.session
    }

    fn frontend_url(&self) -> &str {
        "https://bike.test"
    }
}

#[tokio::test]
async fn cookie_logout_revokes_the_current_session() {
    use axum::extract::{FromRequestParts, State};
    let db = database().await;
    let user = existing_user(&db).await;
    let storage = std::sync::Arc::new(TestAuthStorage {
        db: db.clone(),
        pid: None,
        session: SessionService::new(db.clone(), "session-test-secret".into()),
    });
    let token = storage
        .session
        .issue_tokens(&db, &user)
        .await
        .unwrap()
        .refresh_token;
    let request = axum::http::Request::builder()
        .header(axum::http::header::COOKIE, format!("refresh_token={token}"))
        .body(())
        .unwrap();
    let (mut parts, ()) = request.into_parts();
    let auth =
        super::extractors::AuthInfo::<TestAuthStorage>::from_request_parts(&mut parts, &storage)
            .await
            .unwrap();
    let response = super::controllers::auth::logout(State(storage.clone()), auth)
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    assert!(response.headers()[axum::http::header::SET_COOKIE]
        .to_str()
        .unwrap()
        .contains("Max-Age=0"));
    assert!(refresh_tokens::Model::find_by_token(&db, &token)
        .await
        .unwrap()
        .is_none());
}

async fn admin_context(
    storage: &std::sync::Arc<TestAuthStorage>,
) -> super::extractors::AdminUserContext<TestAuthStorage> {
    use axum::extract::FromRequestParts;
    let (mut parts, ()) = axum::http::Request::new(()).into_parts();
    super::extractors::AdminUserContext::from_request_parts(&mut parts, storage)
        .await
        .expect("enabled administrator")
}

#[tokio::test]
async fn admin_disable_persists_filters_and_prevents_authentication() {
    use super::controllers::admin;
    use super::extractors::UserContext;
    use axum::extract::{FromRequestParts, Path, State};
    use axum::Json;

    let db = database().await;
    let admin = existing_user(&db).await;
    let target = existing_user(&db).await;
    let storage = std::sync::Arc::new(TestAuthStorage {
        db: db.clone(),
        pid: Some(admin.pid),
        session: SessionService::new(db.clone(), "session-test-secret".into()),
    });
    let session = SessionService::new(db.clone(), "session-test-secret".into());
    let tokens = session
        .issue_tokens(&db, &target)
        .await
        .expect("session before disable");

    let disabled = admin::disable_user(
        admin_context(&storage).await,
        State(storage.clone()),
        Path(target.id),
        Json(admin::DisableUserRequest { disabled: None }),
    )
    .await
    .expect("disable target")
    .0;
    assert_eq!(serde_json::to_value(disabled).unwrap()["disabled"], true);

    let filter =
        serde_json::from_value(serde_json::json!({ "page": 1, "per_page": 20, "disabled": true }))
            .unwrap();
    let page = admin::list_users(
        admin_context(&storage).await,
        State(storage.clone()),
        axum::extract::Query(filter),
    )
    .await
    .expect("disabled-user page")
    .0;
    assert_eq!(page.total, 1);
    assert_eq!(page.data[0].id, target.id);
    let stored = users::Model::find_by_pid(&db, &target.pid)
        .await
        .unwrap()
        .unwrap();
    assert!(session.issue_tokens(&db, &stored).await.is_err());
    assert!(session.refresh(&db, tokens.refresh_token).await.is_err());

    let disabled_storage = std::sync::Arc::new(TestAuthStorage {
        db: db.clone(),
        pid: Some(target.pid),
        session: SessionService::new(db.clone(), "session-test-secret".into()),
    });
    let (mut parts, ()) = axum::http::Request::new(()).into_parts();
    assert_disabled_contexts(&disabled_storage).await;

    let enabled = admin::disable_user(
        admin_context(&storage).await,
        State(storage),
        Path(target.id),
        Json(admin::DisableUserRequest {
            disabled: Some(false),
        }),
    )
    .await
    .expect("enable target")
    .0;
    assert_eq!(serde_json::to_value(enabled).unwrap()["disabled"], false);
    let context = UserContext::<TestAuthStorage>::from_request_parts(&mut parts, &disabled_storage)
        .await
        .expect("enabled target");
    let response: super::services::UserResponse = context.user.into();
    assert_eq!(serde_json::to_value(response).unwrap()["disabled"], false);
}

async fn assert_disabled_contexts(storage: &std::sync::Arc<TestAuthStorage>) {
    use super::extractors::{AdminUserContext, UserContext, VerifiedUserContext};
    use axum::extract::FromRequestParts;
    let (mut parts, ()) = axum::http::Request::new(()).into_parts();
    let error = UserContext::<TestAuthStorage>::from_request_parts(&mut parts, storage)
        .await
        .err()
        .expect("disabled user");
    assert_eq!(error.code, axum::http::StatusCode::FORBIDDEN);
    assert_eq!(error.message, "this account is disabled");
    assert!(
        VerifiedUserContext::<TestAuthStorage>::from_request_parts(&mut parts, storage)
            .await
            .is_err()
    );
    assert!(
        AdminUserContext::<TestAuthStorage>::from_request_parts(&mut parts, storage)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn disabled_sso_account_is_not_linked_and_admin_cannot_disable_self() {
    let db = database().await;
    let existing = existing_user(&db).await;
    assert!(existing
        .clone()
        .set_disabled(&db, existing.id, true)
        .await
        .is_err());
    let disabled = existing.set_disabled(&db, 0, true).await.unwrap();
    let error = OAuthService::find_or_create_provider_user(&db, "sso", provider_user())
        .await
        .unwrap_err();
    assert_eq!(error.code, axum::http::StatusCode::FORBIDDEN);
    let stored = users::Model::find_by_pid(&db, &disabled.pid)
        .await
        .unwrap()
        .unwrap();
    assert!(stored.oauth_subject.is_none());
    stored.set_disabled(&db, 0, false).await.unwrap();
    assert!(
        OAuthService::find_or_create_provider_user(&db, "sso", provider_user())
            .await
            .is_ok()
    );
}

async fn database() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("test database");
    let schema = Schema::new(db.get_database_backend());
    db.execute(&schema.create_table_from_entity(users::Entity))
        .await
        .expect("users table");
    db.execute(&schema.create_table_from_entity(oauth_states::Entity))
        .await
        .expect("OAuth states table");
    db.execute_unprepared("CREATE TABLE refresh_tokens (token TEXT PRIMARY KEY, user_pid TEXT NOT NULL, expires_at INTEGER NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)")
        .await.expect("refresh tokens table");
    db.execute_unprepared("CREATE TABLE auth_events (id INTEGER PRIMARY KEY, ts TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, event_type TEXT NOT NULL, user_id INTEGER, email TEXT, ip TEXT, user_agent TEXT, reason TEXT, meta TEXT)")
        .await.expect("auth events table");
    static METRICS: Once = Once::new();
    METRICS.call_once(|| crate::platform::api_metrics::init_api_metrics("auth_tests"));
    db
}

async fn existing_user(db: &DatabaseConnection) -> users::Model {
    users::ActiveModel {
        pid: Set(Uuid::new_v4()),
        email: Set("existing@example.test".into()),
        password: Set(Some("retained-historical-password-data".into())),
        api_key: Set(Uuid::new_v4().to_string()),
        name: Set("Existing rider".into()),
        is_admin: Set(Some(true)),
        disabled: Set(false),
        email_verified_at: Set(Some(Utc::now())),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("existing rider")
}

fn provider_user() -> OAuthUserInfo {
    OAuthUserInfo {
        subject: "sso-subject".into(),
        email: "existing@example.test".into(),
        name: Some("Provider display name".into()),
        preferred_username: None,
        verified_email: Some(true),
    }
}

#[tokio::test]
async fn sso_binding_preserves_existing_identity_and_stored_data() {
    let db = database().await;
    let existing = existing_user(&db).await;
    let linked = OAuthService::find_or_create_provider_user(&db, "sso", provider_user())
        .await
        .expect("link existing account");
    let repeated = OAuthService::find_or_create_provider_user(&db, "sso", provider_user())
        .await
        .expect("resolve existing provider binding");
    assert_eq!(linked.id, existing.id);
    assert_eq!(linked.pid, existing.pid);
    assert_eq!(linked.password, existing.password);
    assert_eq!(linked.name, existing.name);
    assert_eq!(linked.is_admin, existing.is_admin);
    assert_eq!(repeated.pid, existing.pid);
    assert_eq!(repeated.oauth_subject.as_deref(), Some("sso-subject"));
    assert_eq!(repeated.oauth_provider.as_deref(), Some("sso"));
}

#[tokio::test]
async fn oauth_state_is_bound_to_browser_provider_expiry_and_single_use() {
    use super::cookies::validate_oauth_state;
    use axum::http::{header::COOKIE, HeaderMap};
    let db = database().await;
    oauth_states::Model::save(&db, "sso", "challenge", "nonce")
        .await
        .unwrap();
    let mut headers = HeaderMap::new();
    assert!(validate_oauth_state(None, &headers).is_err());
    assert!(validate_oauth_state(Some("challenge"), &headers).is_err());
    headers.insert(COOKIE, "oauth_state=challenge".parse().unwrap());
    assert!(validate_oauth_state(Some("wrong"), &headers).is_err());
    assert_eq!(
        validate_oauth_state(Some("challenge"), &headers).unwrap(),
        "challenge"
    );
    assert!(oauth_states::Model::consume(&db, "other", "challenge")
        .await
        .is_err());
    assert!(oauth_states::Model::consume(&db, "sso", "missing")
        .await
        .is_err());
    let (first, second) = tokio::join!(
        oauth_states::Model::consume(&db, "sso", "challenge"),
        oauth_states::Model::consume(&db, "sso", "challenge")
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    assert_eq!(first.or(second).unwrap(), "nonce");
    assert!(oauth_states::Model::consume(&db, "sso", "challenge")
        .await
        .is_err());
    oauth_states::Model::save(&db, "sso", "expired", "nonce")
        .await
        .unwrap();
    oauth_states::Entity::update_many()
        .col_expr(
            oauth_states::Column::ExpiresAt,
            sea_orm::sea_query::Expr::value(Utc::now() - Duration::minutes(11)),
        )
        .exec(&db)
        .await
        .unwrap();
    assert!(oauth_states::Model::consume(&db, "sso", "expired")
        .await
        .is_err());
}

#[tokio::test]
async fn oidc_exchange_validates_missing_mismatched_and_expired_nonce() {
    use super::services::oauth::oidc;
    use super::services::provider_settings::ProviderConfig;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let issuer = format!("http://{}", listener.local_addr().unwrap());
    let mode = Arc::new(AtomicUsize::new(0));
    let endpoint = issuer.clone();
    let token_mode = mode.clone();
    let app = axum::Router::new().route("/.well-known/openid-configuration", axum::routing::get(move || {
        let issuer = endpoint.clone();
        async move { axum::Json(serde_json::json!({
            "issuer": issuer, "authorization_endpoint": format!("{issuer}/authorize"),
            "token_endpoint": format!("{issuer}/token"), "jwks_uri": format!("{issuer}/keys"),
            "response_types_supported": ["code"], "subject_types_supported": ["public"],
            "id_token_signing_alg_values_supported": ["HS256"]
        })) }
    })).route("/keys", axum::routing::get(|| async { axum::Json(serde_json::json!({"keys": []})) }))
    .route("/token", axum::routing::post(move || {
        let issuer = issuer.clone();
        let mode = token_mode.load(Ordering::SeqCst);
        async move {
            let mut claims = serde_json::json!({"iss": issuer, "sub": "subject", "aud": "client",
                "iat": Utc::now().timestamp(), "exp": (Utc::now() + Duration::minutes(5)).timestamp(),
                "email": "rider@example.test", "email_verified": true, "nonce": "expected"});
            if mode == 2 { claims["nonce"] = serde_json::json!("wrong"); }
            if mode == 3 { claims.as_object_mut().unwrap().remove("nonce"); }
            if mode == 4 { claims["exp"] = serde_json::json!((Utc::now() - Duration::minutes(5)).timestamp()); }
            let token = jsonwebtoken::encode(&jsonwebtoken::Header::default(), &claims,
                &jsonwebtoken::EncodingKey::from_secret(b"oidc-test-client-secret")).unwrap();
            let mut response = serde_json::json!({"access_token": "access", "token_type": "Bearer", "id_token": token});
            if mode == 1 { response.as_object_mut().unwrap().remove("id_token"); }
            axum::Json(response)
        }
    }));
    let address = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let cfg = ProviderConfig {
        provider: "sso".into(),
        client_id: "client".into(),
        client_secret: "oidc-test-client-secret".into(),
        redirect_url: "http://bike.test/callback".into(),
        auth_url: format!("{address}/authorize"),
        token_url: format!("{address}/token"),
        userinfo_url: format!("{address}/userinfo"),
        scopes: vec!["openid".into()],
        issuer_url: Some(address),
    };
    assert_eq!(
        oidc::exchange(&cfg, "code".into(), "expected".into())
            .await
            .unwrap()
            .subject,
        "subject"
    );
    for invalid in 1..=4 {
        mode.store(invalid, Ordering::SeqCst);
        assert!(
            oidc::exchange(&cfg, "code".into(), "expected".into())
                .await
                .is_err(),
            "invalid mode {invalid}"
        );
    }
    server.abort();
}

#[tokio::test]
async fn concurrent_refresh_consumes_a_token_once() {
    let db = database().await;
    let user = existing_user(&db).await;
    let service = SessionService::new(db.clone(), "session-test-secret".into());
    let token = service
        .issue_tokens(&db, &user)
        .await
        .unwrap()
        .refresh_token;
    let (first, second) = tokio::join!(
        service.refresh(&db, token.clone()),
        service.refresh(&db, token.clone())
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    let rejected = first.as_ref().err().or(second.as_ref().err()).unwrap();
    assert_eq!(rejected.code, axum::http::StatusCode::UNAUTHORIZED);
    assert!(service.refresh(&db, token).await.is_err());
    let next = first.or(second).unwrap().refresh_token;
    assert!(service.refresh(&db, next).await.is_ok());
}

#[tokio::test]
async fn sessions_rotate_refresh_tokens_and_reject_replay_and_expiry() {
    let db = database().await;
    let user = existing_user(&db).await;
    let service = SessionService::new(db.clone(), "session-test-secret".into());
    let first = service
        .issue_tokens(&db, &user)
        .await
        .expect("issue SSO session");
    let claims = verify_access_token(&first.access_token, "session-test-secret")
        .expect("valid access token")
        .claims;
    assert_eq!(claims.sub, user.pid.to_string());
    assert!(claims.exp > Utc::now().timestamp());

    let rotated = service
        .refresh(&db, first.refresh_token.clone())
        .await
        .expect("rotate session");
    assert_ne!(rotated.refresh_token, first.refresh_token);
    assert_eq!(rotated.pid, first.pid);
    assert_eq!(rotated.is_admin, first.is_admin);
    assert!(service.refresh(&db, first.refresh_token).await.is_err());

    let stored = refresh_tokens::Entity::find_by_id(rotated.refresh_token.clone())
        .one(&db)
        .await
        .expect("query current token")
        .expect("persisted token");
    let mut expired: refresh_tokens::ActiveModel = stored.into();
    expired.expires_at = Set((Utc::now() - Duration::seconds(1)).timestamp());
    expired.update(&db).await.expect("expire token");
    assert!(service
        .refresh(&db, rotated.refresh_token.clone())
        .await
        .is_err());
    assert!(
        refresh_tokens::Model::find_by_token(&db, &rotated.refresh_token)
            .await
            .expect("query expired token")
            .is_none()
    );
}
