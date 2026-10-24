use super::*;
use super::{fixture::audit, handlers_fixture::HandlerFixture, imports};
use bike_core::entities::{activity_import_artifacts, strava_activity_states, user_preferences};
use sea_orm::{MockExecResult, Value};

#[tokio::test]
async fn persist_creates_a_bike_activity_import_and_provider_artifact_atomically() {
    verify_upsert(false, "Ride").await;
}

#[tokio::test]
async fn persist_updates_the_owned_activity_and_import_without_creating_duplicates() {
    verify_upsert(true, "Ride").await;
}

#[tokio::test]
async fn non_cycling_summary_creates_a_feed_activity_without_marking_cycling_fitness_dirty() {
    verify_upsert(false, "Walk").await;
}

#[tokio::test]
async fn changing_a_ride_to_a_walk_invalidates_its_previous_cycling_fitness() {
    verify_upsert(true, "Walk").await;
}

async fn verify_upsert(update: bool, sport: &str) {
    let mut fixture = HandlerFixture::new();
    std::sync::Arc::get_mut(&mut fixture.provider)
        .unwrap()
        .payload
        .activity
        .sport_type = Some(sport.into());
    let normalization = fixture.operations(MockDatabase::new(DatabaseBackend::Postgres));
    let input = normalization.normalize_activity(fixture.sources().await).await.unwrap();
    let source_hash = input.source.sha256.clone();
    let operations = fixture.operations(upsert_mock(&fixture, update));
    let result = operations.persist_activity(input).await;
    let transactions = operations.db.into_transaction_log();
    let sql = transactions
        .iter()
        .flat_map(|txn| txn.statements())
        .map(|statement| statement.sql.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let change = result.unwrap_or_else(|error| panic!("{error:?}\n{sql}"));
    assert_eq!(change.status, ActivityStatus::Applied);
    assert_eq!(change.bike_activity_id, Some(42));
    assert_eq!(change.import_id, Some(18));
    assert_eq!(change.segment_ids, if update { vec![1, 2] } else { vec![] });
    assert_saved_segment_scope(&transactions, &change);
    assert_eq!(change.dirty_from.is_some(), sport == "Ride" || update);
    assert_eq!(change.event.strava_activity_id, fixture.domain.event.strava_activity_id);
    let operation = if update { "UPDATE" } else { "INSERT INTO" };
    assert!(sql.contains(&format!("{operation} \"activities\"")));
    assert!(sql.contains(&format!("{operation} \"activity_imports\"")));
    assert!(sql.contains("activity_import_artifacts"));
    assert!(sql.contains("activity_recordings"));
    assert_recording_output(&fixture);
    assert!(format!("{transactions:?}").contains(&source_hash));
    assert!(sql.contains("COMMIT"));
    assert!(!sql.contains("ROLLBACK"));
    assert!(fixture.provider.calls.lock().unwrap().is_empty());
}

fn upsert_mock(fixture: &HandlerFixture, update: bool) -> MockDatabase {
    let domain = &fixture.domain;
    let mut activity = domain.activity();
    activity.activity_import_id = Some(18);
    let bytes = serde_json::to_vec(&fixture.provider.payload).unwrap();
    let parsed = bike_core::activity_parser::strava::parse_strava_provider_payload(&bytes).unwrap();
    activity.derived_data_json = Some((&parsed.derived_data).into());
    let existing = if update { vec![activity.clone()] } else { vec![] };
    let mut mock = fixture
        .connection_mock()
        .append_query_results([vec![domain.row(ActivityStatus::Pending)]])
        .append_query_results([Vec::<user_preferences::Model>::new()])
        .append_query_results([vec![domain.connection.clone()]])
        .append_query_results([vec![domain.row(ActivityStatus::Pending)]])
        .append_query_results([Vec::<strava_activity_states::Model>::new()])
        .append_query_results([existing.clone()]);
    if update {
        mock = mock.append_query_results([Vec::<strava_activity_states::Model>::new()]);
    }
    if update {
        mock = mock
            .append_query_results([bike_core::test_support::segment_id_rows(&[1, 2])])
            .append_query_results([vec![imports::import(domain)]]);
    }
    let mock = mock
        .append_query_results([vec![imports::import(domain)]])
        .append_query_results([Vec::<activity_import_artifacts::Model>::new()])
        .append_query_results([vec![imports::artifact(domain)]])
        .append_query_results([existing])
        .append_query_results([vec![activity]])
        .append_query_results([Vec::<bike_core::entities::activity_recordings::Model>::new()])
        .append_query_results([vec![domain.activity_state()]])
        .append_exec_results([executed()]);
    let mock = if crate::strava::provider_payload::strava_activity_is_bike(&fixture.provider.payload.activity) || update
    {
        mock.append_exec_results([executed()])
    } else {
        mock
    };
    mock.append_query_results([vec![audit(domain)], vec![audit(domain)]])
        .append_exec_results([executed(), executed(), executed()])
}

fn executed() -> MockExecResult {
    MockExecResult {
        rows_affected: 1,
        last_insert_id: 0,
    }
}

fn assert_recording_output(fixture: &HandlerFixture) {
    let retained = fixture.storage.objects.lock().unwrap();
    let recording = retained
        .iter()
        .find(|(key, _)| key.starts_with("strava/recordings/7/"))
        .unwrap();
    let restored: bike_core::entities::activity_recordings::Recording = serde_json::from_slice(recording.1).unwrap();
    let bytes = serde_json::to_vec(&fixture.provider.payload).unwrap();
    let parsed = bike_core::activity_parser::strava::parse_strava_provider_payload(&bytes).unwrap();
    assert_eq!(
        restored,
        bike_core::entities::activity_recordings::Recording::from(parsed.derived_data)
    );
}

fn assert_saved_segment_scope(transactions: &[sea_orm::Transaction], change: &ActivityChange) {
    let stored_scope = transactions
        .iter()
        .flat_map(|txn| txn.statements())
        .filter(|statement| statement.sql.starts_with("UPDATE \"strava_events\""))
        .flat_map(|statement| &statement.values.as_ref().unwrap().0)
        .find_map(|value| match value {
            Value::Json(Some(value)) => value.get("segment_ids"),
            _ => None,
        })
        .unwrap();
    assert_eq!(*stored_scope, serde_json::json!(change.segment_ids));
}
