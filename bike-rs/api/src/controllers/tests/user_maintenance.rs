use super::{
    routes,
    test_support::{json_request, temporal_state},
};
use crate::storage::AppStorage;
use bike_core::{
    auth::entities::users,
    test_support::{self as fixtures, temporal_client::Requests},
};
use serde_json::json;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

const PATH: &str = "/api/admin/users/7/archive-imports";

fn admin() -> users::Model {
    users::Model {
        id: 1,
        pid: uuid::Uuid::from_u128(1),
        is_admin: Some(true),
        ..fixtures::customer()
    }
}

#[tokio::test]
async fn archive_import_uses_the_selected_user_and_storage_key() {
    let user = fixtures::customer();
    let job = fixtures::archive_import_job();
    let db = fixtures::postgres_mock()
        .append_query_results([vec![admin()], vec![user.clone()]])
        .append_query_results([vec![job.clone()]])
        .into_connection();
    let requests = Arc::new(Mutex::new(Requests::default()));
    let mut state = temporal_state(db.clone(), requests.clone()).await;
    state.local_admin_user_pid = Some(admin().pid);
    let response = routes()
        .with_state(Arc::new(state))
        .oneshot(json_request("POST", PATH, json!({"archive_url": job.archive_url})))
        .await
        .unwrap();
    assert_eq!(response.status(), 202);
    let statements = format!("{:?}", db.into_transaction_log());
    assert!(statements.contains(&user.pid.to_string()));
    let requests = requests.lock().unwrap();
    assert_eq!(requests.starts.len(), 1);
    assert_eq!(
        serde_json::from_slice::<String>(
            &requests.starts[0].search_attributes.as_ref().unwrap().indexed_fields["BikeUserId"].data
        )
        .unwrap(),
        user.id.to_string()
    );
    let input: serde_json::Value =
        serde_json::from_slice(&requests.starts[0].input.as_ref().unwrap().payloads[0].data).unwrap();
    assert_eq!(input["job_id"], job.id);
}

#[tokio::test]
async fn archive_import_rejects_non_admin_and_missing_or_disabled_users() {
    for target in [
        vec![],
        vec![users::Model {
            disabled: true,
            ..fixtures::customer()
        }],
    ] {
        let expected = if target.is_empty() { 404 } else { 403 };
        let db = fixtures::postgres_mock()
            .append_query_results([vec![admin()], target])
            .into_connection();
        let mut state = AppStorage::for_test(db);
        state.local_admin_user_pid = Some(admin().pid);
        let response = routes()
            .with_state(Arc::new(state))
            .oneshot(json_request(
                "POST",
                PATH,
                json!({"archive_url": "https://example.com/export.zip"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }
    let db = fixtures::postgres_mock()
        .append_query_results([vec![fixtures::customer()]])
        .into_connection();
    let mut state = AppStorage::for_test(db);
    state.local_admin_user_pid = Some(fixtures::customer().pid);
    let response = routes()
        .with_state(Arc::new(state))
        .oneshot(json_request(
            "POST",
            PATH,
            json!({"archive_url": "https://example.com/export.zip"}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
}
