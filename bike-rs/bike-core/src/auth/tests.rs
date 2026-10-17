use chrono::{Duration, Utc};

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
    let app = axum::Router::new()
        .route(
            "/.well-known/openid-configuration",
            axum::routing::get(move || {
                let issuer = endpoint.clone();
                async move {
                    axum::Json(serde_json::json!({
                        "issuer": issuer, "authorization_endpoint": format!("{issuer}/authorize"),
                        "token_endpoint": format!("{issuer}/token"), "jwks_uri": format!("{issuer}/keys"),
                        "response_types_supported": ["code"], "subject_types_supported": ["public"],
                        "id_token_signing_alg_values_supported": ["HS256"]
                    }))
                }
            }),
        )
        .route(
            "/keys",
            axum::routing::get(|| async { axum::Json(serde_json::json!({"keys": []})) }),
        )
        .route(
            "/token",
            axum::routing::post(move || {
                let issuer = issuer.clone();
                let mode = token_mode.load(Ordering::SeqCst);
                async move { oidc_token_response(&issuer, mode) }
            }),
        );
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
            oidc::exchange(&cfg, "code".into(), "expected".into()).await.is_err(),
            "invalid mode {invalid}"
        );
    }
    server.abort();
}

fn oidc_token_response(issuer: &str, mode: usize) -> axum::Json<serde_json::Value> {
    let mut claims = serde_json::json!({"iss": issuer, "sub": "subject", "aud": "client",
        "iat": Utc::now().timestamp(), "exp": (Utc::now() + Duration::minutes(5)).timestamp(),
        "email": "rider@example.test", "email_verified": true, "nonce": "expected"});
    if mode == 2 {
        claims["nonce"] = serde_json::json!("wrong");
    }
    if mode == 3 {
        claims.as_object_mut().unwrap().remove("nonce");
    }
    if mode == 4 {
        claims["exp"] = serde_json::json!((Utc::now() - Duration::minutes(5)).timestamp());
    }
    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(b"oidc-test-client-secret"),
    )
    .unwrap();
    let mut response = serde_json::json!({"access_token": "access", "token_type": "Bearer", "id_token": token});
    if mode == 1 {
        response.as_object_mut().unwrap().remove("id_token");
    }
    axum::Json(response)
}
