use super::*;
use crate::test_support as fixtures;
use sea_orm::{ActiveValue, MockExecResult};

fn matched(effort: &Model) -> MatchedSegmentEffort {
    MatchedSegmentEffort {
        start_route_point_index: effort.start_route_point_index,
        end_route_point_index: effort.end_route_point_index,
        start_elapsed_seconds: effort.start_elapsed_seconds,
        end_elapsed_seconds: effort.end_elapsed_seconds,
        duration_seconds: effort.duration_seconds,
        distance_meters: effort.distance_meters,
    }
}

#[test]
fn removing_an_earlier_attempt_retains_the_later_traversals_identity() {
    let scenario = fixtures::shared_segments();
    let efforts: Vec<_> = scenario
        .efforts_for(1)
        .into_iter()
        .filter(|row| row.user_id == 7)
        .collect();
    let later = efforts[1].clone();
    let plan = Reconciliation::for_activity(7, 42, efforts, vec![(1, vec![matched(&later)])]);
    assert!(plan.new.is_empty());
    assert_eq!(plan.removed, vec![1]);
    assert_eq!(plan.changed.len(), 1);
    assert_eq!(plan.changed[0].id, Set(later.id));
    assert_eq!(plan.changed[0].effort_index, Set(1));
    assert_eq!(plan.changed[0].created_at, ActiveValue::NotSet);
}

#[test]
fn unchanged_traversals_in_overlapping_segments_keep_their_own_ids() {
    let first = fixtures::segment_effort();
    let overlapping = Model {
        id: 2,
        segment_id: 2,
        ..first.clone()
    };
    let matches = vec![(1, vec![matched(&first)]), (2, vec![matched(&overlapping)])];
    let plan = Reconciliation::for_activity(7, 42, vec![first, overlapping], matches);
    assert!(plan.new.is_empty());
    assert!(plan.changed.is_empty());
    assert!(plan.removed.is_empty());
}

#[test]
fn a_different_recording_window_gets_a_new_id_even_with_the_same_attempt_number() {
    let old = fixtures::segment_effort();
    let new = Model {
        start_route_point_index: 3,
        end_route_point_index: 4,
        ..old.clone()
    };
    let plan = Reconciliation::for_activity(7, 42, vec![old], vec![(1, vec![matched(&new)])]);
    assert_eq!(plan.removed, vec![1]);
    assert!(plan.changed.is_empty());
    assert_eq!(plan.new.len(), 1);
    assert_eq!(plan.new[0].id, ActiveValue::NotSet);
}

#[tokio::test]
async fn changed_metrics_update_the_saved_id_and_invalidate_only_its_ranks() {
    let old = fixtures::segment_effort();
    let changed = Model {
        duration_seconds: 55,
        ..old.clone()
    };
    let db = fixtures::postgres_mock()
        .append_query_results([vec![old]])
        .append_exec_results([fixtures::written()])
        .into_connection();
    Entity::reconcile_activity_matches(&db, 7, 42, vec![(1, vec![matched(&changed)])])
        .await
        .unwrap();
    let log = db.into_transaction_log();
    assert_eq!(log.len(), 2);
    let statement = &log[1].statements()[0];
    assert!(statement.sql.starts_with("UPDATE \"segment_efforts\""));
    assert!(statement.sql.contains("\"overall_rank\" ="));
    assert!(statement.sql.contains("\"user_rank\" ="));
    let cleared_ranks = statement
        .values
        .as_ref()
        .unwrap()
        .0
        .iter()
        .filter(|value| **value == sea_orm::Value::Int(None))
        .count();
    assert_eq!(cleared_ranks, 2);
    assert!(!statement.sql.contains("\"created_at\" ="));
    assert!(statement.sql.contains("\"user_id\" ="));
    assert!(statement.sql.contains("\"activity_id\" ="));
    assert!(!statement.sql.contains("INSERT"));
}

#[tokio::test]
async fn a_concurrently_removed_effort_fails_instead_of_being_resurrected() {
    let old = fixtures::segment_effort();
    let changed = Model {
        duration_seconds: 55,
        ..old.clone()
    };
    let db = fixtures::postgres_mock()
        .append_query_results([vec![old]])
        .append_exec_results([MockExecResult::default()])
        .into_connection();
    let error = Entity::reconcile_activity_matches(&db, 7, 42, vec![(1, vec![matched(&changed)])])
        .await
        .unwrap_err();
    assert!(matches!(error, DbErr::RecordNotFound(_)));
}

#[tokio::test]
async fn new_traversals_use_bounded_inserts_without_replacing_existing_rows() {
    let input = matched(&fixtures::segment_effort());
    let matches = (0..501)
        .map(|index| MatchedSegmentEffort {
            start_route_point_index: index * 2,
            end_route_point_index: index * 2 + 1,
            ..input.clone()
        })
        .collect();
    let db = fixtures::postgres_mock()
        .append_query_results([Vec::<Model>::new()])
        .append_exec_results([500, 1].map(|rows_affected| MockExecResult {
            rows_affected,
            ..Default::default()
        }))
        .into_connection();
    Entity::reconcile_activity_matches(&db, 7, 42, vec![(1, matches)])
        .await
        .unwrap();
    let log = db.into_transaction_log();
    assert_eq!(log.len(), 3);
    for transaction in &log[1..] {
        assert!(transaction.statements()[0]
            .sql
            .starts_with("INSERT INTO \"segment_efforts\""));
        assert!(!transaction.statements()[0].sql.contains("ON CONFLICT"));
    }
}

#[tokio::test]
async fn empty_results_remove_only_the_processed_athletes_activity_efforts() {
    for (user_id, activity_id) in [(7, 42), (8, 43)] {
        let existing: Vec<_> = fixtures::shared_segments()
            .efforts
            .into_iter()
            .filter(|effort| effort.activity_id == activity_id)
            .collect();
        let db = fixtures::postgres_mock()
            .append_query_results([existing.clone()])
            .append_exec_results([fixtures::written()])
            .into_connection();
        Entity::reconcile_activity_matches(&db, user_id, activity_id, Vec::new())
            .await
            .unwrap();
        let log = db.into_transaction_log();
        let statement = &log[1].statements()[0];
        assert!(statement.sql.starts_with("DELETE FROM \"segment_efforts\" WHERE"));
        assert!(statement.sql.contains("\"user_id\" = $1"));
        assert!(statement.sql.contains("\"activity_id\" = $2"));
        let values = &statement.values.as_ref().unwrap().0;
        assert_eq!(&values[..2], &[user_id.into(), activity_id.into()]);
        assert_eq!(
            values[2..],
            existing
                .iter()
                .map(|effort| effort.id.into())
                .collect::<Vec<sea_orm::Value>>()
        );
    }
}

#[tokio::test]
async fn a_spectators_empty_match_set_does_not_write_other_riders_efforts() {
    let db = fixtures::postgres_mock()
        .append_query_results([Vec::<Model>::new()])
        .into_connection();
    Entity::reconcile_activity_matches(&db, 9, 44, Vec::new())
        .await
        .unwrap();
    let log = db.into_transaction_log();
    assert_eq!(log.len(), 1);
    let values = &log[0].statements()[0].values.as_ref().unwrap().0;
    assert_eq!(values, &vec![9.into(), 44.into()]);
}
