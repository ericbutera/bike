use super::*;
use crate::strava::native::strava_native_unit::fixture::{audit, Fixture};
use sea_orm::{DatabaseBackend, MockDatabase, MockExecResult};

fn unchanged_state(fixture: &Fixture) -> strava_activity_states::Model {
    strava_activity_states::Model {
        applied_hash: Some(fixture.normalized_activity().source.sha256),
        ..fixture.activity_state()
    }
}

#[tokio::test]
async fn unchanged_content_still_archives_its_provider_state_transition() {
    let fixture = Fixture::new();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![unchanged_state(&fixture)]])
        .append_query_results([vec![audit(&fixture)], vec![audit(&fixture)]])
        .append_exec_results((0..2).map(|_| MockExecResult {
            last_insert_id: 0,
            rows_affected: 1,
        }))
        .into_connection();
    let transaction = db.begin().await.unwrap();
    let change = StravaOperations::persist_unchanged(
        &transaction,
        &fixture.normalized_activity(),
        &fixture.activity(),
    )
    .await
    .unwrap();
    assert_eq!(change.status, ActivityStatus::Unchanged);
    assert_eq!(change.bike_activity_id, Some(fixture.activity().id));
    transaction.commit().await.unwrap();
    let log = db.into_transaction_log();
    assert_eq!(log.len(), 1);
    assert!(log[0]
        .statements()
        .iter()
        .any(|statement| statement.sql.contains("activity_event_history")));
}

#[tokio::test]
async fn failed_archive_does_not_acknowledge_an_unchanged_event() {
    let fixture = Fixture::new();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![unchanged_state(&fixture)]])
        .append_exec_errors([sea_orm::DbErr::Custom("fixture archive unavailable".into())])
        .into_connection();
    let transaction = db.begin().await.unwrap();
    assert!(StravaOperations::persist_unchanged(
        &transaction,
        &fixture.normalized_activity(),
        &fixture.activity(),
    )
    .await
    .is_err());
    transaction.rollback().await.unwrap();
    let log = db.into_transaction_log();
    assert_eq!(log[0].statements().last().unwrap().sql, "ROLLBACK");
}
