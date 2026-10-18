use crate::auth::{
    controllers::admin::{AdminUserDetailsResponse, AdminUserResponse},
    entities::{auth_events, users},
};
use crate::test_support::{self as fixtures, count_row, postgres_mock};
use sea_orm::{DbErr, Value};
use std::collections::BTreeMap;

fn login_row() -> BTreeMap<String, Value> {
    [("last_login_at".into(), fixtures::timestamp().into())].into()
}

#[tokio::test]
async fn admin_page_does_not_read_audit_events_or_issue_per_user_queries() {
    let other = users::Model {
        id: 8,
        ..fixtures::customer()
    };
    let db = postgres_mock()
        .append_query_results([vec![count_row(2)]])
        .append_query_results([vec![fixtures::customer(), other]])
        .into_connection();
    let (users, total) = users::Model::admin_page(&db, None, None, 1, 20).await.unwrap();
    assert_eq!(total, 2);
    assert_eq!(users.len(), 2);
    let response = serde_json::to_value(AdminUserResponse::from(users.into_iter().next().unwrap())).unwrap();
    assert!(response.get("last_login_at").is_none());
    let log = db.into_transaction_log();
    assert_eq!(log.len(), 2, "the grid must only count and page user records");
    assert!(!format!("{log:?}").contains("auth_events"));
}

#[tokio::test]
async fn empty_admin_pages_skip_login_metadata_queries() {
    let db = postgres_mock()
        .append_query_results([vec![count_row(0)]])
        .append_query_results([Vec::<users::Model>::new()])
        .into_connection();
    let (users, total) = users::Model::admin_page(&db, None, None, 1, 20).await.unwrap();
    assert!(users.is_empty());
    assert_eq!(total, 0);
    assert_eq!(db.into_transaction_log().len(), 2);
}

#[tokio::test]
async fn individual_user_summaries_include_the_same_last_login_and_preserve_missing_history() {
    for rows in [vec![login_row()], vec![]] {
        let expected = (!rows.is_empty()).then(fixtures::timestamp);
        let db = postgres_mock().append_query_results([rows]).into_connection();
        let summary = fixtures::customer().admin_details(&db).await.unwrap();
        let response = AdminUserDetailsResponse::from(summary);
        assert_eq!(response.last_login_at, expected.map(|at| at.to_rfc3339()));
        assert_eq!(response.user.created_at, fixtures::customer().created_at.to_rfc3339());
        assert_eq!(db.into_transaction_log().len(), 1);
    }
}

#[tokio::test]
async fn login_metadata_database_errors_propagate() {
    let db = postgres_mock()
        .append_query_errors([DbErr::Custom("audit unavailable".into())])
        .into_connection();
    assert_eq!(
        auth_events::Model::last_login(&db, 7).await.unwrap_err(),
        DbErr::Custom("audit unavailable".into()),
    );
}
