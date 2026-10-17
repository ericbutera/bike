use super::*;
use crate::{test_support as fixtures, test_support::strava_fixture::Fixture};
use sea_orm::{DatabaseBackend, MockDatabase, MockExecResult, TransactionTrait};

#[test]
fn manual_edits_archive_before_and_after_with_the_actor() {
    let before = fixtures::activity();
    let after = activities::Model {
        title: "Renamed activity".into(),
        ..before.clone()
    };
    let event = Event::manual(&before, &after).unwrap();
    assert_eq!(event.source, "manual");
    assert_eq!(event.before.unwrap()["title"], before.title);
    let snapshot = event.after.unwrap();
    assert_eq!(snapshot["title"], after.title);
    assert!(snapshot.get("derived_data_json").is_none());
    assert_eq!(event.context["actor_user_id"], after.user_id);
}

#[test]
fn strava_history_links_the_event_remote_state_and_bike_activity() {
    let fixture = Fixture::new();
    let activity = fixture.activity();
    let change = ActivityChange::applied(
        fixture.event.clone(),
        activity.id,
        fixtures::activity_import().id,
        activity.started_at.date_naive(),
    );
    let event = Event::strava(&change, &fixture.activity_state(), None, Some(&activity)).unwrap();
    assert_eq!(event.id, format!("strava:{}", change.event.strava_event_id));
    assert_eq!(event.activity_id, Some(activity.id));
    assert_eq!(event.occurred_at.timestamp(), fixture.event.event_time);
    assert_eq!(
        event.context["strava_state"]["athlete_id"],
        fixture.event.strava_athlete_id
    );
    assert_eq!(
        event.context["strava_state"]["activity_id"],
        fixture.event.strava_activity_id.unwrap()
    );
    assert!(event.before.is_none());
}

#[test]
fn native_import_history_keeps_the_command_and_full_causal_context() {
    let execution = fixtures::import_execution();
    let event = Event::import(&execution, None, &fixtures::activity()).unwrap();
    assert_eq!(event.source, "import");
    assert_eq!(
        event.context["workflow"],
        serde_json::to_value(&execution.workflow).unwrap()
    );
    assert_eq!(
        event.context["command"]["source"]["cause"],
        serde_json::to_value(fixtures::task_cause()).unwrap()
    );
    let repeated = Event::import(&execution, None, &fixtures::activity()).unwrap();
    assert_ne!(event.id, repeated.id);
    assert!(event.context.get("refresh_scope").is_none());
}

#[tokio::test]
async fn history_appends_in_the_callers_transaction_and_accepts_a_duplicate_key() {
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_exec_results([1, 0].map(|rows_affected| MockExecResult {
            last_insert_id: 0,
            rows_affected,
        }))
        .into_connection();
    let transaction = db.begin().await.unwrap();
    Model::append(&transaction, fixtures::activity_history_event())
        .await
        .unwrap();
    Model::append(&transaction, fixtures::activity_history_event())
        .await
        .unwrap();
    transaction.commit().await.unwrap();
    assert_eq!(db.into_transaction_log().len(), 1);
}

#[tokio::test]
async fn archive_errors_propagate_to_the_mutation_transaction() {
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_exec_errors([DbErr::Custom("fixture archive unavailable".into())])
        .into_connection();
    let transaction = db.begin().await.unwrap();
    assert!(Model::append(&transaction, fixtures::activity_history_event())
        .await
        .is_err());
    transaction.rollback().await.unwrap();
}

#[tokio::test]
async fn retained_history_can_be_loaded_after_activity_deletion() {
    let history = fixtures::activity_history();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![history.clone()]])
        .into_connection();
    assert_eq!(
        Model::recent(&db, history.user_id, history.activity_id.unwrap())
            .await
            .unwrap(),
        vec![history]
    );
}
