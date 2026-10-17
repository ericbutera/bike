//! Shared authenticated request fixture for controller tests.
use crate::storage::AppStorage;
use axum::body::{to_bytes, Body};
use axum::http::Request;
use bike_core::test_support as fixtures;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;

pub(super) fn json_request(method: &str, path: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

pub(super) async fn temporal_state(
    db: DatabaseConnection,
    requests: Arc<std::sync::Mutex<fixtures::temporal_client::Requests>>,
) -> AppStorage {
    let mut state = AppStorage::for_test(db.clone());
    state.tasks = bike_core::jobs::JobQueue::with_client(db, fixtures::temporal_client::client(requests).await);
    state
}

pub(super) fn activity_upload_request() -> Request<Body> {
    let mut body = b"--bike-fixture\r\nContent-Disposition: form-data; name=\"file\"; filename=\"outdoor.tcx\"\r\nContent-Type: application/xml\r\n\r\n".to_vec();
    body.extend_from_slice(fixtures::REPROCESS_SOURCE);
    body.extend_from_slice(b"\r\n--bike-fixture--\r\n");
    Request::builder()
        .method("POST")
        .uri("/api/activity-imports")
        .header("content-type", "multipart/form-data; boundary=bike-fixture")
        .body(Body::from(body))
        .unwrap()
}

pub(super) async fn local_user_json(db: DatabaseConnection, path: &str, status: u16) -> Value {
    let state = AppStorage::for_test(db);
    local_user_request(state, Request::builder().uri(path).body(Body::empty()).unwrap(), status).await
}

pub(super) async fn local_user_request(mut state: AppStorage, request: Request<Body>, status: u16) -> Value {
    state.local_admin_user_pid = Some(fixtures::customer().pid);
    let response = super::routes()
        .with_state(Arc::new(state))
        .oneshot(request)
        .await
        .unwrap();
    assert_eq!(response.status(), status);
    serde_json::from_slice(&to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap()
}
