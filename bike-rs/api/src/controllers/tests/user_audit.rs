use super::test_support::local_user_json as request;
use bike_core::{auth::entities::users, test_support as fixtures};
use sea_orm::DbErr;

fn admin() -> users::Model {
    users::Model {
        is_admin: Some(true),
        ..fixtures::customer()
    }
}

#[tokio::test]
async fn user_audit_history_is_scoped_paginated_and_omits_internal_metadata() {
    let event = fixtures::auth_event();
    let db = fixtures::postgres_mock()
        .append_query_results([vec![admin()], vec![fixtures::customer()]])
        .append_query_results([vec![fixtures::count_row(21)]])
        .append_query_results([vec![event.clone()]])
        .into_connection();
    let response = request(db.clone(), "/api/admin/users/7/audit-events?page=2", 200).await;
    assert_eq!(response["metadata"]["page"], 2);
    assert_eq!(response["metadata"]["total"], 21);
    assert_eq!(response["data"][0]["event_type"], event.event_type);
    assert_eq!(response["data"][0]["occurred_at"], event.ts.to_rfc3339());
    assert!(response["data"][0].get("meta").is_none());
    assert!(response["data"][0].get("email").is_none());
    let log = db.into_transaction_log();
    let query = &log.last().unwrap().statements()[0];
    assert!(query.sql.contains("\"user_id\" = $1"));
    assert!(query.sql.contains("\"ts\" DESC"));
    assert!(query.sql.contains("\"id\" DESC"));
    assert_eq!(
        query.values.as_ref().unwrap().0,
        vec![7_i32.into(), 20_u64.into(), 20_u64.into()]
    );
}

#[tokio::test]
async fn user_audit_requires_admin_and_reports_missing_users_before_reading_events() {
    let db = fixtures::postgres_mock()
        .append_query_results([vec![fixtures::customer()]])
        .into_connection();
    request(db.clone(), "/api/admin/users/7/audit-events", 403).await;
    assert_eq!(db.into_transaction_log().len(), 1);
    let db = fixtures::postgres_mock()
        .append_query_results([vec![admin()], vec![]])
        .into_connection();
    request(db.clone(), "/api/admin/users/7/audit-events", 404).await;
    assert_eq!(db.into_transaction_log().len(), 2);
}

#[tokio::test]
async fn user_audit_database_failures_do_not_look_like_empty_history() {
    let db = fixtures::postgres_mock()
        .append_query_results([vec![admin()], vec![fixtures::customer()]])
        .append_query_errors([DbErr::Custom("audit unavailable".into())])
        .into_connection();
    request(db, "/api/admin/users/7/audit-events", 500).await;
}
