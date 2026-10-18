use super::*;
use crate::controllers::test_support::{json_request, local_user_request, temporal_state};
use axum::body::Body;
use axum::http::Request;
use bike_core::{auth::entities::users, test_support as fixtures};
use sea_orm::{DatabaseBackend, MockDatabase};
use serde_json::json;
use tower::ServiceExt;

fn body() -> serde_json::Value {
    json!({"command_id": "00000000-0000-4000-8000-000000000065", "task": "rebuild_fitness"})
}

#[tokio::test]
async fn only_an_administrator_can_enqueue_manual_work() {
    for path in ["/api/admin/users/7/tasks", "/api/admin/activities/42/tasks"] {
        let state = AppStorage::for_test(MockDatabase::new(DatabaseBackend::Postgres).into_connection());
        let response = crate::controllers::routes()
            .with_state(Arc::new(state))
            .oneshot(Request::builder().method("POST").uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([vec![fixtures::customer()]])
            .into_connection();
        let requests = Default::default();
        let state = temporal_state(db, Arc::clone(&requests)).await;
        local_user_request(state, json_request("POST", path, body()), 403).await;
        assert!(requests.lock().unwrap().starts.is_empty());
    }
}

#[tokio::test]
async fn admin_json_request_returns_202_after_the_native_start_and_records_the_actor() {
    let admin = users::Model {
        is_admin: Some(true),
        ..fixtures::customer()
    };
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![admin], vec![fixtures::customer()]])
        .append_query_results([vec![fixtures::integration_event()]])
        .into_connection();
    let requests = Default::default();
    let state = temporal_state(db.clone(), Arc::clone(&requests)).await;
    let response = local_user_request(state, json_request("POST", "/api/admin/users/7/tasks", body()), 202).await;
    assert_eq!(response["status"], "accepted");
    assert_eq!(response["job"]["run_id"], "fixture-run");
    assert_eq!(requests.lock().unwrap().starts.len(), 1);
    assert!(format!("{:?}", db.into_transaction_log()).contains("actor_user_id"));
}
