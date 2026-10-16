use super::super::strava_native_unit::fixture::Fixture;
use super::*;
use crate::entities::strava_activity_states;
use sea_orm::{DatabaseBackend, MockDatabase, MockExecResult, Value};
use std::collections::BTreeMap;

#[tokio::test]
async fn deletion_returns_the_stored_change_without_repeating_writes() {
    let fixture = Fixture::new();
    let mut row = fixture.row(ActivityStatus::Deleted);
    let mut stored = fixture.change(ActivityStatus::Deleted);
    stored.removed_source = Some("fixture-retired-source".into());
    row.mutation = Some(serde_json::to_value(&stored).unwrap());
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![row]])
        .into_connection();
    let operations = StravaOperations::new(db);
    let change = operations
        .delete_activity(fixture.change(ActivityStatus::Captured))
        .await
        .unwrap();
    assert_eq!(change.status, ActivityStatus::Deleted);
    assert_eq!(change.removed_source, stored.removed_source);
    let log = format!("{:?}", operations.db.into_transaction_log());
    assert!(!log.contains("DELETE"));
    assert!(!log.contains("UPDATE"));
}

#[tokio::test]
async fn superseded_deletion_records_its_status_and_keeps_activity_records() {
    let fixture = Fixture::new();
    let mut state = state(&fixture);
    state.event_time += 1;
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixture.row(ActivityStatus::Pending)]])
        .append_query_results([vec![fixture.connection.clone()]])
        .append_query_results([vec![fixture.row(ActivityStatus::Pending)]])
        .append_query_results([vec![state]])
        .append_exec_results([executed()])
        .into_connection();
    let operations = StravaOperations::new(db);
    let change = operations
        .delete_activity(fixture.change(ActivityStatus::Captured))
        .await
        .unwrap();
    assert_eq!(change.status, ActivityStatus::Superseded);
    let log = format!("{:?}", operations.db.into_transaction_log());
    assert!(!log.contains("DELETE"));
    assert!(log.contains("superseded"));
    assert!(log.contains("COMMIT"));
}

#[tokio::test]
async fn absent_activity_commits_a_deletion_marker_and_explicit_status() {
    let fixture = Fixture::new();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixture.row(ActivityStatus::Pending)]])
        .append_query_results([vec![fixture.connection.clone()]])
        .append_query_results([vec![fixture.row(ActivityStatus::Pending)]])
        .append_query_results([Vec::<strava_activity_states::Model>::new()])
        .append_query_results([Vec::<activities::Model>::new()])
        .append_query_results([vec![state(&fixture)]])
        .append_exec_results([executed(), executed()])
        .into_connection();
    let operations = StravaOperations::new(db);
    let change = operations
        .delete_activity(fixture.change(ActivityStatus::AlreadyAbsent))
        .await
        .unwrap();
    assert_eq!(change.status, ActivityStatus::AlreadyAbsent);
    let log = format!("{:?}", operations.db.into_transaction_log());
    assert!(log.contains("strava_activity_states"));
    assert!(log.contains("already_absent"));
    assert!(log.contains("COMMIT"));
}

#[tokio::test]
async fn deletion_removes_only_the_owned_remote_activity_and_retains_dependencies() {
    let fixture = Fixture::new();
    let segments: BTreeMap<String, Value> = [("segment_id".into(), 3.into())].into();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixture.activity()]])
        .append_query_results([vec![segments]])
        .append_exec_results((0..4).map(|_| executed()))
        .into_connection();
    let mut change = fixture.change(ActivityStatus::Captured);
    StravaOperations::delete_owned_activity(&db, &mut change)
        .await
        .unwrap();
    assert_eq!(change.status, ActivityStatus::Deleted);
    assert_eq!(change.bike_activity_id, Some(42));
    assert_eq!(change.segment_ids, vec![3]);
    assert_eq!(
        change.dirty_from,
        Some(fixture.activity().started_at.date_naive())
    );
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("source_correlation_id"));
    assert!(log.contains("user_id"));
    assert!(log.contains("DELETE FROM"));
}

#[tokio::test]
async fn non_cycling_capture_and_deletion_do_not_invalidate_fitness() {
    let fixture = Fixture::new();
    let mut activity = fixture.activity();
    activity.sport = "walk".into();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixture.connection.clone()]])
        .append_query_results([vec![fixture.customer.clone()]])
        .append_query_results([vec![activity.clone()]])
        .append_query_results([Vec::<BTreeMap<String, Value>>::new()])
        .append_query_results([vec![activity]])
        .append_query_results([Vec::<BTreeMap<String, Value>>::new()])
        .append_exec_results((0..4).map(|_| executed()))
        .into_connection();
    let operations = StravaOperations::new(db);
    let mut change = operations
        .capture_deletion(fixture.event.clone())
        .await
        .unwrap();
    assert_eq!(change.dirty_from, None);
    StravaOperations::delete_owned_activity(&operations.db, &mut change)
        .await
        .unwrap();
    assert_eq!(change.status, ActivityStatus::Deleted);
    assert_eq!(change.dirty_from, None);
    let log = format!("{:?}", operations.db.into_transaction_log());
    assert!(!log.contains("fitness"));
}

#[tokio::test]
async fn deletion_storage_failure_rolls_back_without_recording_a_success() {
    let fixture = Fixture::new();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixture.row(ActivityStatus::Pending)]])
        .append_query_results([vec![fixture.connection.clone()]])
        .append_query_results([vec![fixture.row(ActivityStatus::Pending)]])
        .append_query_results([Vec::<strava_activity_states::Model>::new()])
        .append_query_results([Vec::<activities::Model>::new()])
        .append_query_errors([sea_orm::DbErr::Custom("fixture marker failure".into())])
        .into_connection();
    let operations = StravaOperations::new(db);
    assert!(operations
        .delete_activity(fixture.change(ActivityStatus::AlreadyAbsent))
        .await
        .is_err());
    let transactions = operations.db.into_transaction_log();
    assert!(format!("{transactions:?}").contains("ROLLBACK"));
    assert!(!transactions
        .iter()
        .flat_map(|transaction| transaction.statements())
        .any(|statement| statement.sql.trim_start().starts_with("UPDATE")));
}

fn state(fixture: &Fixture) -> strava_activity_states::Model {
    strava_activity_states::Model {
        deleted: true,
        deleted_at: Some(fixture.connection.created_at),
        ..fixture.activity_state()
    }
}

fn executed() -> MockExecResult {
    MockExecResult {
        last_insert_id: 0,
        rows_affected: 1,
    }
}
