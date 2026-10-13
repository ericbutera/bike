use super::handlers_fixture::HandlerFixture;
use super::*;
use crate::strava::events::StravaOperation;
use sea_orm::{MockExecResult, Value};
use std::collections::BTreeMap;

#[tokio::test]
async fn incremental_window_includes_the_cursor_second_and_has_a_fixed_end() {
    let mut fixture = HandlerFixture::new();
    fixture.domain.event.operation = StravaOperation::Sync;
    fixture.domain.connection.last_synced_activity_started_at =
        Some(fixture.domain.connection.created_at);
    let window = fixture
        .operations(fixture.connection_mock())
        .collection_window(fixture.domain.event.clone())
        .await
        .unwrap()
        .window;
    assert_eq!(
        window.after,
        fixture.domain.connection.created_at.timestamp() - 1
    );
    assert_eq!(window.before, Some(fixture.domain.event.event_time + 1));
}

#[tokio::test]
async fn backfill_requires_and_preserves_an_explicit_date_window() {
    let mut fixture = HandlerFixture::new();
    fixture.domain.event.operation = StravaOperation::Backfill;
    assert!(fixture
        .operations(fixture.connection_mock())
        .collection_window(fixture.domain.event.clone())
        .await
        .is_err());
    fixture.domain.event.window = Some(ActivityWindow {
        after: 100,
        before: Some(200),
    });
    let window = fixture
        .operations(fixture.connection_mock())
        .collection_window(fixture.domain.event.clone())
        .await
        .unwrap()
        .window;
    assert_eq!(window.after, 100);
    assert_eq!(window.before, Some(200));
    fixture.domain.event.window = Some(ActivityWindow {
        after: 200,
        before: Some(100),
    });
    assert!(fixture
        .operations(fixture.connection_mock())
        .collection_window(fixture.domain.event.clone())
        .await
        .is_err());
}

#[tokio::test]
async fn completion_recounts_persisted_children_and_only_sync_changes_the_cursor() {
    for operation in [StravaOperation::Sync, StravaOperation::Backfill] {
        let mut fixture = HandlerFixture::new();
        fixture.domain.event.operation = operation;
        let db = completion_mock(&fixture);
        let operations = fixture.operations(db);
        let input = CollectionResult {
            event: fixture.domain.event.clone(),
            imported: 999,
            unchanged: 999,
            last_started_at: Some(fixture.domain.connection.created_at),
        };
        let result = if operation == StravaOperation::Sync {
            operations.complete_sync(input).await.unwrap()
        } else {
            operations.complete_backfill(input).await.unwrap()
        };
        assert_eq!((result.imported, result.unchanged), (2, 3));
        let transactions = operations.db.into_transaction_log();
        let update = transactions
            .iter()
            .flat_map(|txn| txn.statements())
            .find(|statement| statement.sql.starts_with("UPDATE \"strava_connections\""))
            .unwrap();
        assert_eq!(
            update
                .sql
                .split(" RETURNING ")
                .next()
                .unwrap()
                .contains("last_synced_activity_started_at"),
            operation == StravaOperation::Sync
        );
        assert!(format!("{transactions:?}").contains("COMMIT"));
    }
}

fn completion_mock(fixture: &HandlerFixture) -> MockDatabase {
    let counts: BTreeMap<String, Value> =
        [("applied".into(), 2.into()), ("other".into(), 3.into())].into();
    fixture
        .connection_mock()
        .append_query_results([vec![counts]])
        .append_query_results([
            vec![fixture.domain.connection.clone()],
            vec![fixture.domain.connection.clone()],
        ])
}

#[tokio::test]
async fn removing_a_connection_is_idempotent_and_generation_fenced() {
    let fixture = HandlerFixture::new();
    let mut event = fixture.domain.event.clone();
    event.operation = StravaOperation::Disconnect;
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixture.domain.connection.clone()]])
        .append_exec_results([MockExecResult {
            rows_affected: 1,
            last_insert_id: 0,
        }]);
    let operations = fixture.operations(db);
    operations.remove_connection(event.clone()).await.unwrap();
    let log = format!("{:?}", operations.db.into_transaction_log());
    assert!(log.contains("DELETE FROM"));
    assert!(log.contains("created_at"));
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([Vec::<strava_connections::Model>::new()]);
    fixture
        .operations(db)
        .remove_connection(event.clone())
        .await
        .unwrap();
    let mut reconnected = fixture.domain.connection.clone();
    reconnected.created_at += chrono::Duration::seconds(1);
    let operations = fixture.operations(
        MockDatabase::new(DatabaseBackend::Postgres).append_query_results([vec![reconnected]]),
    );
    assert!(operations.remove_connection(event).await.is_err());
    assert!(!format!("{:?}", operations.db.into_transaction_log()).contains("DELETE"));
}

#[tokio::test]
async fn capturing_deletion_keeps_local_activity_import_segment_and_dirty_date() {
    let fixture = HandlerFixture::new();
    let segments: BTreeMap<String, Value> = [("segment_id".into(), 3.into())].into();
    let db = fixture
        .connection_mock()
        .append_query_results([vec![fixture.domain.activity()]])
        .append_query_results([vec![segments]]);
    let change = fixture
        .operations(db)
        .capture_deletion(fixture.domain.event.clone())
        .await
        .unwrap();
    assert_eq!(change.bike_activity_id, Some(42));
    assert_eq!(change.event.bike_activity_id, Some(42));
    assert_eq!(change.segment_ids, [3]);
    assert_eq!(change.status, ActivityStatus::Captured);
    assert_eq!(
        change.dirty_from,
        Some(fixture.domain.connection.created_at.date_naive())
    );
}
