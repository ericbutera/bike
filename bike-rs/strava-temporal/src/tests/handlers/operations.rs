//! Exercise registered SDK handlers with the real domain facade and mocked I/O.
use super::*;
use bike_core::entities::{activities, integration_events, strava_activity_states, strava_connections};
use sea_orm::{MockExecResult, Value};
use std::collections::BTreeMap;

use bike_core::strava::test_fixtures as customer_fixture;

fn connection_mock() -> MockDatabase {
    MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![customer_fixture::connection()]])
        .append_query_results([vec![customer_fixture::customer()]])
}

fn mutation(fixture: &Fixture, status: ActivityStatus) -> bike_core::entities::strava_events::Model {
    let mut row = fixture.row(status);
    row.mutation = Some(serde_json::to_value(fixture.change(status)).unwrap());
    row
}

fn audit() -> integration_events::Model {
    integration_events::Model {
        id: 1,
        user_id: Some(7),
        provider: "strava".into(),
        event_type: "strava.event.accepted".into(),
        level: "info".into(),
        message: "fixture".into(),
        connection_id: Some(9),
        payload: None,
        created_at: chrono::Utc::now(),
    }
}

#[tokio::test]
async fn registered_fetch_handlers_return_provider_sources_for_the_owned_activity() {
    let fixture = Fixture::new(StravaOperation::Upsert);
    let storage = storage(&fixture);
    let db = connection_mock()
        .append_query_results([vec![customer_fixture::connection()]])
        .append_query_results([vec![customer_fixture::customer()]]);
    let (environment, provider) = environment_with_database(&fixture.event, storage.clone(), db);
    let activity = environment
        .run(StravaActivities::fetch_activity, fixture.event.clone())
        .await
        .unwrap();
    let sources = environment
        .run(StravaActivities::fetch_streams, activity)
        .await
        .unwrap();
    assert_eq!(sources.event.correlation_id, fixture.event.correlation_id);
    let data = storage.objects.lock().unwrap();
    let activity: serde_json::Value = serde_json::from_slice(&data[&sources.activity.path]).unwrap();
    assert_eq!(activity["id"], fixture.activity_id);
    assert_eq!(*provider.calls.lock().unwrap(), ["activity", "streams"]);
}

#[tokio::test]
async fn registered_reconciliation_fetch_captures_the_source_version_before_provider_io() {
    let fixture = Fixture::reconciling(StravaOperation::Upsert);
    let state = strava_activity_states::Model {
        activity_id: fixture.activity_id,
        ..bike_core::test_support::strava_activity_state()
    };
    let db = connection_mock().append_query_results([vec![state.clone()]]);
    let (environment, provider) = environment_with_database(&fixture.event, storage(&fixture), db);
    let activity = environment
        .run(StravaActivities::fetch_activity, fixture.event)
        .await
        .unwrap();
    assert_eq!(
        activity.event.source_version.unwrap().updated_at,
        Some(state.updated_at)
    );
    assert_eq!(*provider.calls.lock().unwrap(), ["activity"]);
}

#[tokio::test]
async fn registered_persist_and_delete_retries_return_the_committed_mutation() {
    let fixture = Fixture::new(StravaOperation::Upsert);
    let db = connection_mock().append_query_results([vec![mutation(&fixture, ActivityStatus::Applied)]]);
    let (environment, _) = environment_with_database(&fixture.event, Arc::new(Storage::default()), db);
    let change = environment
        .run(StravaActivities::persist_activity, fixture.normalized())
        .await
        .unwrap();
    assert_eq!(change.bike_activity_id, Some(42));
    assert_eq!(change.status, ActivityStatus::Applied);
    let fixture = Fixture::new(StravaOperation::Delete);
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![mutation(&fixture, ActivityStatus::Deleted)]]);
    let (environment, _) = environment_with_database(&fixture.event, Arc::new(Storage::default()), db);
    assert_eq!(
        environment
            .run(
                StravaActivities::delete_activity,
                fixture.change(ActivityStatus::Captured)
            )
            .await
            .unwrap()
            .status,
        ActivityStatus::Deleted
    );
}

#[tokio::test]
async fn registered_capture_acknowledges_an_already_deleted_activity() {
    let fixture = Fixture::new(StravaOperation::Delete);
    let db = connection_mock().append_query_results([Vec::<activities::Model>::new()]);
    let (environment, _) = environment_with_database(&fixture.event, Arc::new(Storage::default()), db);
    let change = environment
        .run(StravaActivities::capture_deletion, fixture.event.clone())
        .await
        .unwrap();
    assert_eq!(change.status, ActivityStatus::AlreadyAbsent);
    assert_eq!(change.bike_activity_id, None);
}

#[tokio::test]
async fn registered_source_deletion_removes_only_the_captured_path() {
    let fixture = Fixture::new(StravaOperation::Delete);
    let storage = storage(&fixture);
    let environment = environment(&fixture.event, storage.clone());
    let mut change = fixture.change(ActivityStatus::Deleted);
    change.removed_source = Some(fixture.sources().streams.path);
    environment
        .run(StravaActivities::delete_source, change.clone())
        .await
        .unwrap();
    assert_eq!(*storage.removed.lock().unwrap(), [change.removed_source.unwrap()]);
    assert!(storage
        .objects
        .lock()
        .unwrap()
        .contains_key(&fixture.activity().activity.path));
}

#[tokio::test]
async fn registered_publication_handlers_preserve_an_unchanged_activity() {
    let fixture = Fixture::new(StravaOperation::Upsert);
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![customer_fixture::connection()]])
        .append_query_results([Vec::<strava_activity_states::Model>::new()]);
    let (environment, _) = environment_with_database(&fixture.event, Arc::new(Storage::default()), db);
    let change = ActivityChange::unchanged(fixture.event.clone(), ActivityStatus::Unchanged);
    let change = environment
        .run(StravaActivities::rebuild_activity, change)
        .await
        .unwrap();
    let change = environment
        .run(StravaActivities::rebuild_segments, change)
        .await
        .unwrap();
    let change = environment
        .run(StravaActivities::rebuild_fitness, change)
        .await
        .unwrap();
    let change = environment
        .run(StravaActivities::publish_heatmap, change)
        .await
        .unwrap();
    let change = environment
        .run(StravaActivities::complete_activity, change)
        .await
        .unwrap();
    assert_eq!(change.status, ActivityStatus::Unchanged);
    assert_eq!(change.event.correlation_id, fixture.event.correlation_id);
}

#[tokio::test]
async fn registered_collection_window_and_page_keep_the_offset_and_provider_activity_id() {
    let fixture = Fixture::new(StravaOperation::Sync);
    let db = connection_mock()
        .append_query_results([vec![customer_fixture::connection()]])
        .append_query_results([vec![customer_fixture::customer()]]);
    let (environment, provider) = environment_with_database(&fixture.event, Arc::new(Storage::default()), db);
    let collection = environment
        .run(StravaActivities::collection_window, fixture.event.clone())
        .await
        .unwrap();
    assert_eq!(
        collection.window.after,
        customer_fixture::connection()
            .last_synced_activity_started_at
            .unwrap()
            .timestamp()
            - 1
    );
    let page = environment
        .run(StravaActivities::list_activity_page, PageInput { collection, page: 1 })
        .await
        .unwrap();
    assert_eq!(page.activity_ids, [fixture.activity_id]);
    assert_eq!(page.next_page, Some(2));
    assert_eq!(*provider.calls.lock().unwrap(), ["page"]);
}

#[tokio::test]
async fn registered_child_acceptance_preserves_root_ids_and_marks_child_submitted() {
    let parent = Fixture::new(StravaOperation::Sync);
    let mut child = Fixture::new(StravaOperation::Upsert);
    child.event = parent.event.for_activity(child.activity_id, StravaOperation::Upsert);
    let db = connection_mock()
        .append_query_results([vec![child.row(ActivityStatus::Pending)]])
        .append_query_results([vec![audit()]])
        .append_query_results([vec![child.row(ActivityStatus::Pending)]])
        .append_exec_results([MockExecResult {
            rows_affected: 1,
            last_insert_id: 0,
        }]);
    let (environment, _) = environment_with_database(&parent.event, Arc::new(Storage::default()), db);
    let accepted = environment
        .run(
            StravaActivities::accept_child,
            ChildEvent {
                parent: parent.event.clone(),
                child: child.event,
            },
        )
        .await
        .unwrap();
    assert_eq!(accepted.correlation_id, parent.event.correlation_id);
    assert_eq!(accepted.parent_event_id, Some(parent.event.strava_event_id));
    assert_eq!(accepted.strava_activity_id, Some(child.activity_id));
}

fn completion_mock() -> MockDatabase {
    let counts: BTreeMap<String, Value> = [("applied".into(), 2.into()), ("other".into(), 3.into())].into();
    connection_mock()
        .append_query_results([vec![counts]])
        .append_query_results([
            vec![customer_fixture::connection()],
            vec![customer_fixture::connection()],
        ])
}

#[tokio::test]
async fn registered_sync_and_backfill_completion_return_persisted_counts() {
    let fixture = Fixture::new(StravaOperation::Sync);
    let input = CollectionResult {
        event: fixture.event.clone(),
        imported: 99,
        unchanged: 99,
        last_started_at: None,
    };
    let (environment, _) = environment_with_database(&fixture.event, Arc::new(Storage::default()), completion_mock());
    let result = environment.run(StravaActivities::complete_sync, input).await.unwrap();
    assert_eq!((result.imported, result.unchanged), (2, 3));
    let fixture = Fixture::new(StravaOperation::Backfill);
    let input = CollectionResult {
        event: fixture.event.clone(),
        imported: 99,
        unchanged: 99,
        last_started_at: None,
    };
    let (environment, _) = environment_with_database(&fixture.event, Arc::new(Storage::default()), completion_mock());
    let result = environment
        .run(StravaActivities::complete_backfill, input)
        .await
        .unwrap();
    assert_eq!((result.imported, result.unchanged), (2, 3));
}

#[tokio::test]
async fn registered_disconnect_blocks_writes_revokes_and_removes_the_owned_connection() {
    let mut fixture = Fixture::new(StravaOperation::Disconnect);
    fixture.event.revoke_remote = true;
    let ids: BTreeMap<String, Value> =
        [("source_correlation_id".into(), fixture.activity_id.to_string().into())].into();
    let db = connection_mock()
        .append_query_results([
            vec![customer_fixture::connection()],
            vec![customer_fixture::connection()],
        ])
        .append_query_results([vec![ids]])
        .append_query_results([vec![customer_fixture::connection()]])
        .append_query_results([vec![customer_fixture::customer()]])
        .append_query_results([vec![customer_fixture::connection()]])
        .append_exec_results([MockExecResult {
            rows_affected: 1,
            last_insert_id: 0,
        }]);
    let (environment, provider) = environment_with_database(&fixture.event, Arc::new(Storage::default()), db);
    let ids = environment
        .run(StravaActivities::begin_disconnect, fixture.event.clone())
        .await
        .unwrap();
    assert_eq!(ids, [fixture.activity_id]);
    let event = environment
        .run(StravaActivities::revoke_connection, fixture.event.clone())
        .await
        .unwrap();
    let event = environment
        .run(StravaActivities::remove_connection, event)
        .await
        .unwrap();
    assert_eq!(event.strava_athlete_id, 123);
    assert_eq!(*provider.calls.lock().unwrap(), ["revoke"]);
}

#[tokio::test]
async fn registered_scan_returns_an_empty_page_when_no_customers_are_connected() {
    let fixture = Fixture::new(StravaOperation::Sync);
    let db =
        MockDatabase::new(DatabaseBackend::Postgres).append_query_results([Vec::<strava_connections::Model>::new()]);
    let (environment, _) = environment_with_database(&fixture.event, Arc::new(Storage::default()), db);
    let page = environment
        .run(
            StravaActivities::scan_connections,
            ScanRequest {
                correlation_id: fixture.event.correlation_id,
                after_connection_id: 0,
                failed_customers: 0,
            },
        )
        .await
        .unwrap();
    assert!(page.events.is_empty());
    assert_eq!(page.next_connection_id, None);
}
