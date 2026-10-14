mod collections;
pub(crate) mod fixture;
mod handlers;
mod handlers_fixture;
mod imports;
mod persistence;
mod upsert;
mod workflow_spans;
use super::*;
use fixture::Fixture;
use sea_orm::{DatabaseBackend, MockDatabase};

#[tokio::test]
async fn connection_checks_customer_and_generation_with_mocked_storage() {
    let fixture = Fixture::new();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixture.connection.clone()]])
        .append_query_results([vec![fixture.customer.clone()]])
        .into_connection();
    let connection = StravaOperations::new(db)
        .connection(&fixture.event)
        .await
        .unwrap();
    assert_eq!(connection.id, fixture.connection.id);
}

#[tokio::test]
async fn connection_rejects_a_different_customer() {
    let fixture = Fixture::new();
    let mut customer = fixture.customer.clone();
    customer.pid = uuid::Uuid::from_u128(99);
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixture.connection.clone()]])
        .append_query_results([vec![customer]])
        .into_connection();
    let error = StravaOperations::new(db)
        .connection(&fixture.event)
        .await
        .unwrap_err();
    assert_eq!(error.status, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn connection_rejects_a_reconnected_athlete_before_loading_the_customer() {
    let fixture = Fixture::new();
    let mut connection = fixture.connection.clone();
    connection.created_at += chrono::Duration::seconds(1);
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![connection]])
        .into_connection();
    let error = StravaOperations::new(db)
        .connection(&fixture.event)
        .await
        .unwrap_err();
    assert_eq!(error.status, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn connection_preserves_not_found_and_database_failure_statuses() {
    let fixture = Fixture::new();
    let missing = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([Vec::<strava_connections::Model>::new()])
        .into_connection();
    assert_eq!(
        StravaOperations::new(missing)
            .connection(&fixture.event)
            .await
            .unwrap_err()
            .status,
        axum::http::StatusCode::NOT_FOUND
    );
    let failure = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixture.connection.clone()]])
        .append_query_errors([sea_orm::DbErr::Custom("fixture storage failure".into())])
        .into_connection();
    assert_eq!(
        StravaOperations::new(failure)
            .connection(&fixture.event)
            .await
            .unwrap_err()
            .status,
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    );
}
