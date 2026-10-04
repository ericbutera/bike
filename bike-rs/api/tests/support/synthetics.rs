use super::support;
use axum::{body::Body, http::Request, Router};
use bike_core::{
    auth::{entities::users, services::oauth::OAuthUserInfo, OAuthService},
    entities::activities,
    synthetics,
};
use sea_orm::{EntityTrait, PaginatorTrait};
use serde_json::Value;
use tower::ServiceExt;

async fn request(
    app: &Router,
    method: &str,
    path: &str,
    key: &str,
    forwarded: bool,
    status: u16,
) -> Value {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(synthetics::AUTH_HEADER, key);
    if forwarded {
        request = request.header("x-forwarded-for", "");
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let actual_status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 1_048_576)
        .await
        .unwrap();
    // Request credentials never appear in assertion diagnostics.
    let value: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value.get("token"), None);
    assert_eq!(actual_status, status, "{method} {path}");
    value
}

#[tokio::test]
async fn synthetic_journey_discovers_owned_records_and_provisions_once() {
    let (app, db, owner) = support::synthetic_platform_app().await;
    assert!(owner.disabled && owner.is_synthetic());
    assert_eq!(owner.is_admin, Some(false));
    assert!(owner.ensure_enabled().is_err());
    let ordinary_token = bike_core::auth::tokens::generate_access_token(
        &owner,
        &bike_core::config::Config::get().jwt_secret,
    )
    .unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/auth/current")
                .header("authorization", format!("Bearer {ordinary_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        403,
        "ordinary tokens must not activate the synthetic owner"
    );
    let original = activities::Entity::find_by_id(1109)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let repeated = synthetics::ensure_scenario(&db).await.unwrap();
    assert_eq!(owner.pid, repeated.pid);
    assert_eq!(users::Entity::find().count(&db).await.unwrap(), 2);
    assert_eq!(activities::Entity::find().count(&db).await.unwrap(), 4);
    assert_eq!(
        activities::Entity::find_by_id(1109)
            .one(&db)
            .await
            .unwrap()
            .unwrap(),
        original
    );
    let manifest = request(
        &app,
        "GET",
        "/api/synthetics/scenario",
        support::SYNTHETIC_KEY,
        false,
        200,
    )
    .await;
    assert_eq!(manifest["name"], synthetics::SCENARIO);
    assert_ne!(manifest["activity_id"], 1109);
    assert_ne!(manifest["segment_id"], 5);
    for path in [
        "/api/auth/current".into(),
        "/api/activities".into(),
        format!("/api/activities/{}", manifest["activity_id"]),
        "/api/segments".into(),
        format!("/api/segments/{}", manifest["segment_id"]),
        format!("/api/segments/{}/comparison", manifest["segment_id"]),
    ] {
        request(&app, "GET", &path, support::SYNTHETIC_KEY, false, 200).await;
    }
    let comparison = request(
        &app,
        "GET",
        &format!("/api/segments/{}/comparison", manifest["segment_id"]),
        support::SYNTHETIC_KEY,
        false,
        200,
    )
    .await;
    assert_eq!(comparison["efforts"].as_array().unwrap().len(), 2);
    assert!(comparison["route_points"].as_array().unwrap().len() >= 2);
}

#[tokio::test]
async fn synthetic_auth_rejects_public_credentials_writes_and_other_owners() {
    let (app, db, owner) = support::synthetic_platform_app().await;
    request(&app, "GET", "/api/auth/current", "wrong-key", false, 401).await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/auth/current")
                .header(synthetics::AUTH_HEADER, support::SYNTHETIC_KEY)
                .header("x-forwarded-for", "203.0.113.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    for (method, path, status) in [
        ("PUT", "/api/preferences", 403),
        ("GET", "/api/auth/logout", 403),
        ("GET", "/api/admin/users", 403),
        ("GET", "/api/activities/1109", 404),
        ("GET", "/api/segments/5", 404),
    ] {
        request(&app, method, path, support::SYNTHETIC_KEY, false, status).await;
    }
    let login = OAuthService::find_or_create_provider_user(
        &db,
        "test-oidc",
        OAuthUserInfo {
            subject: "external-subject".into(),
            email: owner.email.clone(),
            name: Some("External".into()),
            preferred_username: None,
            verified_email: Some(true),
        },
    )
    .await;
    assert!(
        login.is_err(),
        "synthetic owner must not be linked through OAuth email matching"
    );
}
