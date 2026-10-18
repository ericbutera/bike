use crate::auth::{entities::refresh_tokens, services::SessionService};
use crate::test_support::{self as fixtures, postgres_mock, written};
use chrono::{Duration, Utc};
use sea_orm::{DbErr, Value};
use std::collections::BTreeMap;

fn inserted_event() -> BTreeMap<String, Value> {
    [("id".into(), 1_i64.into())].into()
}

#[tokio::test]
async fn successful_login_records_the_user_after_session_creation() {
    let db = postgres_mock()
        .append_query_results([vec![fixtures::refresh_token()]])
        .append_query_results([vec![inserted_event()]])
        .into_connection();
    let session = SessionService::new(db.clone(), "fixture-session-secret".into());
    let user = fixtures::customer();
    let response = session.login(&db, &user).await.unwrap();
    assert_eq!(response.pid, user.pid.to_string());
    assert!(!response.access_token.is_empty());
    let log = db.into_transaction_log();
    assert_eq!(log.len(), 2);
    assert!(log[0].statements()[0].sql.contains("refresh_tokens"));
    assert!(log[1].statements()[0].sql.contains("auth_events"));
    let event = format!("{:?}", log[1]);
    assert!(event.contains("login_succeeded"));
    assert!(event.contains(&user.email));
}

#[tokio::test]
async fn failed_or_disabled_logins_do_not_record_success() {
    let user = fixtures::customer();
    let db = postgres_mock()
        .append_query_errors([DbErr::Custom("session unavailable".into())])
        .into_connection();
    let session = SessionService::new(db.clone(), "fixture-session-secret".into());
    assert!(session.login(&db, &user).await.is_err());
    assert_eq!(db.into_transaction_log().len(), 1);

    let db = postgres_mock().into_connection();
    let session = SessionService::new(db.clone(), "fixture-session-secret".into());
    let disabled = crate::auth::entities::users::Model { disabled: true, ..user };
    assert!(session.login(&db, &disabled).await.is_err());
    assert!(db.into_transaction_log().is_empty());
}

#[tokio::test]
async fn token_refresh_records_refresh_without_advancing_last_login() {
    crate::platform::api_metrics::init_api_metrics("auth_test");
    let token = refresh_tokens::Model {
        expires_at: (Utc::now() + Duration::minutes(5)).timestamp(),
        ..fixtures::refresh_token()
    };
    let db = postgres_mock()
        .append_query_results([vec![token.clone()]])
        .append_query_results([vec![fixtures::customer()]])
        .append_query_results([vec![fixtures::refresh_token()]])
        .append_query_results([vec![inserted_event()]])
        .append_exec_results([written()])
        .into_connection();
    let session = SessionService::new(db.clone(), "fixture-session-secret".into());
    session.refresh(&db, token.token).await.unwrap();
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("token_refresh"));
    assert!(!log.contains("login_succeeded"));
}
