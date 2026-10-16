use super::*;
use super::{fixture::audit, handlers_fixture::HandlerFixture, imports};
use crate::entities::{
    activity_import_artifacts, analytics_user_states, strava_activity_states, user_preferences,
};
use sea_orm::MockExecResult;

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
    let input = normalization
        .normalize_activity(fixture.sources().await)
        .await
        .unwrap();
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
    assert_eq!(change.dirty_from.is_some(), sport == "Ride" || update);
    assert_eq!(
        change.event.strava_activity_id,
        fixture.domain.event.strava_activity_id
    );
    let operation = if update { "UPDATE" } else { "INSERT INTO" };
    assert!(sql.contains(&format!("{operation} \"activities\"")));
    assert!(sql.contains(&format!("{operation} \"activity_imports\"")));
    assert!(sql.contains("activity_import_artifacts"));
    assert!(format!("{transactions:?}").contains(&source_hash));
    assert!(sql.contains("COMMIT"));
    assert!(!sql.contains("ROLLBACK"));
    assert!(fixture.provider.calls.lock().unwrap().is_empty());
}

fn upsert_mock(fixture: &HandlerFixture, update: bool) -> MockDatabase {
    let domain = &fixture.domain;
    let mut activity = domain.activity();
    activity.activity_import_id = Some(18);
    let existing = if update {
        vec![activity.clone()]
    } else {
        vec![]
    };
    let mut mock = fixture
        .connection_mock()
        .append_query_results([vec![domain.row(ActivityStatus::Pending)]])
        .append_query_results([Vec::<user_preferences::Model>::new()])
        .append_query_results([vec![domain.connection.clone()]])
        .append_query_results([Vec::<strava_activity_states::Model>::new()])
        .append_query_results([existing.clone()]);
    if update {
        mock = mock.append_query_results([Vec::<strava_activity_states::Model>::new()]);
    }
    mock = mock.append_query_results([vec![domain.row(ActivityStatus::Pending)]]);
    if update {
        mock = mock.append_query_results([vec![imports::import(domain)]]);
    }
    let mock = mock
        .append_query_results([vec![imports::import(domain)]])
        .append_query_results([Vec::<activity_import_artifacts::Model>::new()])
        .append_query_results([vec![imports::artifact(domain)]])
        .append_query_results([existing])
        .append_query_results([vec![activity]])
        .append_query_results([vec![state(domain)]]);
    let mock = if crate::strava::provider_payload::strava_activity_is_bike(
        &fixture.provider.payload.activity,
    ) || update
    {
        mock.append_query_results([Vec::<analytics_user_states::Model>::new()])
            .append_query_results([vec![fitness_state(domain)]])
    } else {
        mock
    };
    mock.append_query_results([vec![audit(domain)], vec![audit(domain)]])
        .append_exec_results([executed(), executed()])
}

fn state(fixture: &Fixture) -> strava_activity_states::Model {
    strava_activity_states::Model {
        athlete_id: fixture.connection.athlete_id,
        activity_id: fixture.event.strava_activity_id.unwrap(),
        connection_generation: fixture.connection.created_at,
        event_time: fixture.event.event_time,
        applied_hash: None,
        deleted: false,
        applied_at: fixture.connection.created_at,
    }
}

fn fitness_state(fixture: &Fixture) -> analytics_user_states::Model {
    analytics_user_states::Model {
        user_id: fixture.customer.id,
        last_activity_change_at: fixture.connection.created_at,
        fitness_dirty_from_day: Some(fixture.connection.created_at.date_naive()),
        last_fitness_rebuild_at: None,
        created_at: fixture.connection.created_at,
        updated_at: fixture.connection.created_at,
    }
}

fn executed() -> MockExecResult {
    MockExecResult {
        rows_affected: 1,
        last_insert_id: 0,
    }
}
