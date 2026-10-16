use super::*;
use crate::test_support as fixtures;
use sea_orm::{DatabaseBackend, MockDatabase, MockExecResult};

fn write_result(rows_affected: u64) -> MockExecResult {
    MockExecResult {
        last_insert_id: 0,
        rows_affected,
    }
}

#[tokio::test]
async fn native_attempt_is_recorded_on_the_owned_import() {
    let attempt = fixtures::activity_import_attempt();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([Vec::<Model>::new()])
        .append_query_results([vec![fixtures::activity_import()]])
        .append_query_results([vec![attempt.clone()]])
        .append_exec_results([write_result(1)])
        .into_connection();
    assert_eq!(
        Model::record_temporal(&db, &fixtures::import_execution())
            .await
            .unwrap(),
        attempt
    );
}

#[tokio::test]
async fn retried_recording_returns_the_existing_domain_attempt() {
    let attempt = fixtures::activity_import_attempt();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![attempt.clone()]])
        .into_connection();
    assert_eq!(
        Model::record_temporal(&db, &fixtures::import_execution())
            .await
            .unwrap(),
        attempt
    );
    assert_eq!(db.into_transaction_log().len(), 1);
}

#[tokio::test]
async fn command_reuse_for_different_work_is_rejected() {
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixtures::activity_import_attempt()]])
        .into_connection();
    let mut execution = fixtures::import_execution();
    execution.command.source.object.sha256 = "different-source".into();
    let error = Model::record_temporal(&db, &execution).await.unwrap_err();
    assert_eq!(error.status, axum::http::StatusCode::CONFLICT);
}

#[tokio::test]
async fn recording_failure_remains_an_error_for_native_activity_retry() {
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_errors([DbErr::Custom("fixture recording unavailable".into())])
        .into_connection();
    let error = Model::record_temporal(&db, &fixtures::import_execution())
        .await
        .unwrap_err();
    assert_eq!(error.status, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn completion_requires_the_expected_domain_attempt() {
    for rows in [1, 0] {
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_exec_results([write_result(rows)])
            .into_connection();
        assert_eq!(
            Model::complete_temporal(
                &db,
                fixtures::activity_import_attempt().id,
                &fixtures::import_result()
            )
            .await
            .is_ok(),
            rows == 1,
        );
    }
}
