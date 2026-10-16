//! Shared authenticated request fixture for controller tests.
use crate::storage::AppStorage;
use axum::body::{to_bytes, Body};
use axum::http::Request;
use bike_core::test_support as fixtures;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;

pub(super) async fn local_user_json(db: DatabaseConnection, path: &str, status: u16) -> Value {
    let mut state = AppStorage::for_test(db);
    state.local_admin_user_pid = Some(fixtures::customer().pid);
    let response = super::routes()
        .with_state(Arc::new(state))
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), status);
    serde_json::from_slice(&to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap()
}
