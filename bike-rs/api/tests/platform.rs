//! HTTP integration coverage for authentication, preferences, and ride read paths.
//! The real Axum router and SeaORM queries run against an isolated SQLite fixture.

mod support;
#[path = "support/synthetics.rs"]
mod synthetic_tests;

use axum::{body::Body, http::Request, Router};
use serde_json::Value;
use tower::ServiceExt;

async fn get_json(app: &Router, path: &str) -> Value {
    request_json(app, "GET", path, None, 200).await
}

async fn request_json(
    app: &Router,
    method: &str,
    path: &str,
    body: Option<&Value>,
    expected_status: u16,
) -> Value {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .body(body.map_or_else(Body::empty, |body| Body::from(body.to_string())))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    assert!(response.headers()["content-type"]
        .to_str()
        .unwrap()
        .starts_with("application/json"));
    let body = axum::body::to_bytes(response.into_body(), 1_048_576)
        .await
        .unwrap();
    assert_eq!(
        status,
        expected_status,
        "{method} {path}: {}",
        String::from_utf8_lossy(&body)
    );
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn protected_reads_reject_invalid_bearer_tokens() {
    let app = support::protected_platform_app().await;
    for path in [
        "/api/auth/current",
        "/api/activities",
        "/api/activities/1109",
        "/api/segments",
        "/api/segments/5",
        "/api/segments/5/comparison",
    ] {
        let request = Request::builder()
            .uri(path)
            .header("authorization", "Bearer platform-invalid-token")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), 401, "invalid bearer on {path}");
    }
}

#[tokio::test]
async fn preference_updates_persist_and_invalid_profiles_leave_state_unchanged() {
    let app = support::platform_app().await;
    let original = get_json(&app, "/api/preferences").await;
    let contract: Value =
        serde_json::from_str(include_str!("../../docs/openapi/openapi.json")).unwrap();
    let example = &contract["paths"]["/preferences"]["put"]["requestBody"]["content"]
        ["application/json"]["example"];
    assert!(example.is_object(), "preferences OpenAPI example");

    let mut invalid = example.clone();
    invalid["xc_goal_event_profile"] = "xc".into();
    request_json(&app, "PUT", "/api/preferences", Some(&invalid), 400).await;
    assert_eq!(get_json(&app, "/api/preferences").await, original);

    let updated = request_json(&app, "PUT", "/api/preferences", Some(example), 200).await;
    assert_eq!(get_json(&app, "/api/preferences").await, updated);
    for (field, value) in example.as_object().unwrap() {
        assert_eq!(updated[field], *value, "saved preference {field}");
    }

    request_json(&app, "PUT", "/api/preferences", Some(&original), 200).await;
    assert_eq!(get_json(&app, "/api/preferences").await, original);
}

fn assert_geometry(points: &Value) {
    let points = points.as_array().expect("route points");
    assert!(points.len() >= 2, "route must contain usable geometry");
    for point in points {
        assert!(point["latitude"].as_f64().unwrap().is_finite());
        assert!(point["longitude"].as_f64().unwrap().is_finite());
    }
}

fn assert_activity(activity: &Value) {
    assert_eq!(activity["id"], 1109);
    assert_eq!(activity["title"], "Synthetic northbound A");
    assert_eq!(activity["distance_meters"], 1000.0);
    assert_eq!(activity["moving_time_seconds"], 120);
}

fn assert_segment(segment: &Value) {
    assert_eq!(segment["id"], 5);
    assert_eq!(segment["title"], "Synthetic northbound segment");
    assert_eq!(segment["distance_meters"], 1000.0);
    assert_eq!(segment["effort_count"], 2);
}

#[tokio::test]
async fn activity_segment_and_race_reads_return_the_seeded_data() {
    let app = support::platform_app().await;

    let health = get_json(&app, "/api/health").await;
    assert_eq!(health["status"], "healthy");

    let user = get_json(&app, "/api/auth/current").await;
    assert_eq!(user["pid"], support::USER_PID);
    assert_eq!(user["verified"], true);
    assert_eq!(user["disabled"], false);

    let activities = get_json(&app, "/api/activities?page=1&per_page=10").await;
    assert_eq!(activities["metadata"]["total"], 2);
    let activity = activities["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|activity| activity["id"] == 1109)
        .expect("known ride on activity page");
    assert_activity(activity);

    let detail = get_json(&app, "/api/activities/1109").await;
    assert_activity(&detail);
    assert_geometry(&detail["route_points"]);

    let segments = get_json(&app, "/api/segments").await;
    let segment = segments
        .as_array()
        .unwrap()
        .iter()
        .find(|segment| segment["id"] == 5)
        .expect("known segment in list");
    assert_segment(segment);
    assert_segment(&get_json(&app, "/api/segments/5").await);

    let comparison = get_json(&app, "/api/segments/5/comparison").await;
    assert_eq!(comparison["segment_id"], 5);
    assert_geometry(&comparison["route_points"]);
    let efforts = comparison["efforts"].as_array().unwrap();
    assert_eq!(efforts.len(), 2);
    for (id, duration) in [(5895, 120), (5912, 100)] {
        let effort = efforts
            .iter()
            .find(|effort| effort["id"] == id)
            .expect("both seeded race efforts");
        assert_eq!(effort["duration_seconds"], duration);
        assert_geometry(&effort["route_points"]);
    }
}
