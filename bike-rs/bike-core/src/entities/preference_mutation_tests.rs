use super::user_preferences::{GoalBackfillChange, Model, PreferencesUpdate};
use crate::test_support as fixtures;
use sea_orm::DbErr;

fn goal_update() -> PreferencesUpdate {
    serde_json::from_value(serde_json::json!({
        "unit_system":"metric", "xc_goal_start_date":"2026-09-01", "xc_goal_target_date":"2026-10-01",
        "xc_goal_target_distance_meters":100_000.0, "xc_goal_target_elevation_gain_meters":1_000.0
    })).unwrap()
}

#[tokio::test]
async fn a_new_goal_queues_backfill_and_an_unchanged_goal_does_not() {
    for existing in [false, true] {
        let saved = Model {
            xc_goal_start_date: Some(fixtures::timestamp().date_naive()),
            ..fixtures::user_preferences()
        };
        let prior = if existing {
            vec![saved.clone()]
        } else {
            vec![]
        };
        let db = fixtures::postgres_mock()
            .append_query_results([prior, vec![saved]])
            .into_connection();
        let outcome = Model::save_preferences(&db, fixtures::customer().id, goal_update())
            .await
            .unwrap();
        assert_eq!(
            outcome.goal_backfill,
            if existing {
                GoalBackfillChange::Unchanged
            } else {
                GoalBackfillChange::Queue
            }
        );
        let log = db.into_transaction_log();
        let write = &log.last().unwrap().statements()[0];
        assert!(write.values.as_ref().unwrap().0.contains(&"metric".into()));
        assert_eq!(write.sql.starts_with("INSERT"), !existing);
    }
}

#[tokio::test]
async fn removing_a_goal_explicitly_clears_backfill_state_without_replacing_the_owner() {
    let before = Model {
        xc_goal_backfill_status: Some("complete".into()),
        ..fixtures::user_preferences()
    };
    let db = fixtures::postgres_mock()
        .append_query_results([vec![before], vec![fixtures::user_preferences()]])
        .into_connection();
    let update = serde_json::from_value(serde_json::json!({"unit_system":"mixed"})).unwrap();
    let outcome = Model::save_preferences(&db, fixtures::customer().id, update)
        .await
        .unwrap();
    assert_eq!(outcome.goal_backfill, GoalBackfillChange::Clear);
    let log = db.into_transaction_log();
    let write = &log.last().unwrap().statements()[0];
    for field in [
        "\"xc_goal_start_date\" =",
        "\"xc_goal_backfill_status\" =",
        "\"xc_goal_backfill_completed_at\" =",
    ] {
        assert!(write.sql.contains(field), "{write:?}");
    }
    assert!(!write
        .sql
        .split(" RETURNING ")
        .next()
        .unwrap()
        .contains("\"created_at\" ="));
}

#[tokio::test]
async fn invalid_preferences_fail_before_any_database_read_or_write() {
    let db = fixtures::postgres_mock().into_connection();
    let mut update = goal_update();
    update.estimated_ftp_watts = Some(-1);
    assert!(
        Model::save_preferences(&db, fixtures::customer().id, update)
            .await
            .is_err()
    );
    assert!(db.into_transaction_log().is_empty());
}

#[tokio::test]
async fn preference_write_failure_is_propagated_without_queueing_work() {
    let db = fixtures::postgres_mock()
        .append_query_results([Vec::<Model>::new()])
        .append_query_errors([DbErr::Custom("fixture preferences unavailable".into())])
        .into_connection();
    let error = Model::save_preferences(&db, fixtures::customer().id, goal_update())
        .await
        .unwrap_err();
    assert_eq!(error.status, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(error.message, "Database request failed");
}
