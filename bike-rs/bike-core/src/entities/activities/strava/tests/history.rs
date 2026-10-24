use super::*;
use crate::test_support::strava_fixture::{audit, Fixture};
use sea_orm::{DatabaseBackend, MockDatabase, MockExecResult};

async fn archive(db: &DatabaseTransaction, fixture: &Fixture) -> Result<(), WorkflowError> {
    let change = fixture.change(ActivityStatus::Unchanged);
    Model::record_strava_change(
        db,
        &change,
        fixture.normalized_activity().source.sha256,
        Some(&fixture.activity()),
        &fixture.activity(),
    )
    .await
}

#[tokio::test]
async fn unchanged_content_still_archives_its_provider_state_transition() {
    let fixture = Fixture::new();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixture.activity_state()]])
        .append_query_results([vec![audit(&fixture)], vec![audit(&fixture)]])
        .append_exec_results((0..2).map(|_| MockExecResult {
            last_insert_id: 0,
            rows_affected: 1,
        }))
        .into_connection();
    let transaction = db.begin().await.unwrap();
    archive(&transaction, &fixture).await.unwrap();
    transaction.commit().await.unwrap();
    let log = db.into_transaction_log();
    assert!(log[0]
        .statements()
        .iter()
        .any(|statement| statement.sql.contains("activity_event_history")));
    assert!(log[0]
        .statements()
        .iter()
        .any(|statement| statement.sql.contains("strava_events")));
}

#[tokio::test]
async fn failed_archive_does_not_acknowledge_an_unchanged_event() {
    let fixture = Fixture::new();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixture.activity_state()]])
        .append_exec_errors([sea_orm::DbErr::Custom("fixture archive unavailable".into())])
        .into_connection();
    let transaction = db.begin().await.unwrap();
    assert!(archive(&transaction, &fixture).await.is_err());
    transaction.rollback().await.unwrap();
    let log = db.into_transaction_log();
    assert_eq!(log[0].statements().last().unwrap().sql, "ROLLBACK");
    assert!(!log[0]
        .statements()
        .iter()
        .any(|statement| statement.sql.starts_with("UPDATE \"strava_events\"")));
}
