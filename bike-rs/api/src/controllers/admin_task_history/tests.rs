use super::*;
use crate::controllers::test_support::{json_request, local_user_request, temporal_state};
use axum::{body::Body, http::Request};
use bike_core::{
    auth::entities::users,
    test_support::{self as fixtures, temporal_client::Requests},
};
use sea_orm::{DatabaseBackend, MockDatabase};
use serde_json::json;
use std::sync::Mutex;
use tower::ServiceExt;

const INPUT_PATH: &str = "/api/admin/tasks/fitness/runs/00000000-0000-4000-8000-000000000001/input";

async fn state(admin: bool, requests: Arc<Mutex<Requests>>) -> AppStorage {
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![users::Model {
            is_admin: Some(admin),
            ..fixtures::customer()
        }]])
        .into_connection();
    temporal_state(db, requests).await
}

#[tokio::test]
async fn execution_history_and_parameters_require_an_administrator() {
    for path in ["/api/admin/tasks", INPUT_PATH] {
        let response = crate::controllers::routes()
            .with_state(Arc::new(AppStorage::for_test(
                MockDatabase::new(DatabaseBackend::Postgres).into_connection(),
            )))
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), 401);
        let requests = Arc::new(Mutex::new(Requests::default()));
        local_user_request(
            state(false, requests.clone()).await,
            json_request("GET", path, json!(null)),
            403,
        )
        .await;
        assert!(requests.lock().unwrap().lists.is_empty());
        assert!(requests.lock().unwrap().histories.is_empty());
    }
}

#[tokio::test]
async fn admin_reads_native_history_and_original_parameters() {
    let requests = Arc::new(Mutex::new(Requests {
        history_response: fixtures::temporal_client::execution_input(json!({"user_id": 7})),
        ..Default::default()
    }));
    let page = local_user_request(
        state(true, requests.clone()).await,
        json_request("GET", "/api/admin/tasks?status=Completed&page_token=0102", json!(null)),
        200,
    )
    .await;
    assert_eq!(page["executions"], json!([]));
    assert_eq!(requests.lock().unwrap().lists[0].next_page_token, [1, 2]);
    let input = local_user_request(
        state(true, requests).await,
        json_request("GET", INPUT_PATH, json!(null)),
        200,
    )
    .await;
    assert_eq!(input["parameters"], json!([{"user_id": 7}]));
}

#[tokio::test]
async fn invalid_tokens_and_run_ids_never_reach_temporal() {
    for path in [
        "/api/admin/tasks?page_token=invalid",
        "/api/admin/tasks/fitness/runs/invalid/input",
    ] {
        let requests = Arc::new(Mutex::new(Requests::default()));
        local_user_request(
            state(true, requests.clone()).await,
            json_request("GET", path, json!(null)),
            400,
        )
        .await;
        assert!(requests.lock().unwrap().lists.is_empty());
        assert!(requests.lock().unwrap().histories.is_empty());
    }
}

#[tokio::test]
async fn temporal_errors_are_visible_without_exposing_internal_details() {
    let requests = Arc::new(Mutex::new(Requests {
        fail_reads: true,
        ..Default::default()
    }));
    let response = local_user_request(
        state(true, requests).await,
        json_request("GET", "/api/admin/tasks", json!(null)),
        502,
    )
    .await;
    assert_eq!(
        response["message"],
        "Task history is unavailable. Try again or open Temporal."
    );
}
