use super::*;
use crate::test_support as fixtures;
use sea_orm::{DatabaseBackend, DbErr, MockDatabase, MockExecResult};

fn written() -> MockExecResult {
    MockExecResult {
        rows_affected: 1,
        ..Default::default()
    }
}

fn ranked_input() -> MockDatabase {
    MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixtures::activity_id_row()]])
        .append_query_results([vec![fixtures::activity_analytics()]])
        .append_query_results([vec![segment_efforts::Model {
            overall_rank: Some(1),
            user_rank: Some(1),
            ..fixtures::segment_effort()
        }]])
        .append_query_results([vec![fixtures::activity_award_output()]])
        .append_query_results([vec![fixtures::activity_earned_awards()]])
}

#[tokio::test]
async fn shared_activity_projection_counts_saved_awards_without_overwriting_legacy_json() {
    let db = ranked_input().append_exec_results([written()]).into_connection();
    rebuild_activity_analytics_cache(&db, &[99, 42, 99, 0]).await.unwrap();
    let log = db.into_transaction_log();
    let statements = log[0].statements();
    assert_eq!(statements.first().unwrap().sql, "BEGIN");
    let lock = &statements[1];
    assert!(lock.sql.contains("ORDER BY \"activities\".\"id\" ASC FOR UPDATE"));
    assert_eq!(lock.values.as_ref().unwrap().0, vec![42.into(), 99.into()]);
    assert!(statements[3].sql.contains("FROM \"segment_efforts\""));
    assert!(!statements[2].sql.contains("achievement_highlights_json"));
    let insert = statements
        .iter()
        .position(|statement| statement.sql.starts_with("INSERT INTO \"activity_analytics\""))
        .unwrap();
    assert!(statements[insert].sql.contains("ON CONFLICT"));
    assert!(!statements[insert].sql.contains("achievement_highlights_json"));
    assert!(!statements.iter().any(|s| s.sql.starts_with("DELETE")));
    assert_eq!(
        &statements[insert].values.as_ref().unwrap().0[..7],
        &[42.into(), 7.into(), 1.into(), 1.into(), 1.into(), 0.into(), 0.into()],
    );
    assert_eq!(statements.last().unwrap().sql, "COMMIT");
}

#[tokio::test]
async fn projection_replacement_failure_rolls_back_instead_of_losing_the_previous_output() {
    let db = ranked_input()
        .append_exec_errors([DbErr::Custom("fixture replacement failure".into())])
        .into_connection();
    let error = rebuild_activity_analytics_cache(&db, &[42]).await.unwrap_err();
    assert_eq!(error, DbErr::Custom("fixture replacement failure".into()));
    let log = db.into_transaction_log();
    assert_eq!(log[0].statements().last().unwrap().sql, "ROLLBACK");
}

#[tokio::test]
async fn no_remaining_efforts_clear_the_previous_projection_in_the_same_transaction() {
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixtures::activity_id_row()]])
        .append_query_results([vec![fixtures::activity_analytics()]])
        .append_query_results([Vec::<segment_efforts::Model>::new()])
        .append_query_results([Vec::<activity_award_outputs::Model>::new()])
        .append_query_results([Vec::<activity_earned_awards::Model>::new()])
        .append_exec_results([written()])
        .into_connection();
    rebuild_activity_analytics_cache(&db, &[42]).await.unwrap();
    let log = db.into_transaction_log();
    let statements = log[0].statements();
    assert_eq!(statements.first().unwrap().sql, "BEGIN");
    assert!(statements[1].sql.ends_with("FOR UPDATE"));
    assert!(statements[3].sql.contains("FROM \"segment_efforts\""));
    let insert = statements
        .iter()
        .find(|s| s.sql.starts_with("INSERT INTO \"activity_analytics\""))
        .unwrap();
    assert_eq!(
        &insert.values.as_ref().unwrap().0[..7],
        &[42.into(), 7.into(), 0.into(), 0.into(), 0.into(), 0.into(), 0.into()]
    );
    assert_eq!(statements.last().unwrap().sql, "COMMIT");
}

#[tokio::test]
async fn an_empty_projection_scope_does_not_start_a_transaction() {
    let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
    rebuild_activity_analytics_cache(&db, &[]).await.unwrap();
    assert!(db.into_transaction_log().is_empty());
}

fn segment_mutation_scope() -> MockDatabase {
    MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![BTreeMap::from([("activity_id", 42.into())])]])
        .append_query_results([vec![fixtures::activity_id_row()]])
}

#[tokio::test]
async fn segment_deletion_locks_affected_activities_before_deleting_efforts() {
    let db = segment_mutation_scope()
        .append_query_results([vec![fixtures::segment()]])
        .append_query_results([vec![BTreeMap::from([("activity_id", 42.into())])]])
        .append_query_results([vec![fixtures::activity_id_row()]])
        .append_query_results([vec![fixtures::activity_analytics()]])
        .append_query_results([Vec::<segment_efforts::Model>::new()])
        .append_query_results([Vec::<activity_award_outputs::Model>::new()])
        .append_query_results([Vec::<activity_earned_awards::Model>::new()])
        .append_exec_results((0..5).map(|_| written()))
        .into_connection();
    fixtures::segment().delete_with_derived_state(&db).await.unwrap();
    let log = db.into_transaction_log();
    let statements: Vec<_> = log.iter().flat_map(|txn| txn.statements()).collect();
    let lock = statements
        .iter()
        .position(|statement| statement.sql.ends_with("FOR UPDATE"))
        .unwrap();
    let delete = statements
        .iter()
        .position(|statement| statement.sql.starts_with("DELETE FROM \"segment_efforts\""))
        .unwrap();
    assert!(lock < delete);
    assert_eq!(statements.last().unwrap().sql, "COMMIT");
}

#[tokio::test]
async fn segment_title_changes_do_not_refresh_activity_projections() {
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![segments::Model {
            title: "Renamed segment".into(),
            ..fixtures::segment()
        }]])
        .into_connection();
    let segment = fixtures::segment()
        .update_details(&db, Some("Renamed segment"), None, None)
        .await
        .unwrap();
    assert_eq!(segment.title, "Renamed segment");
    let log = db.into_transaction_log();
    let statements: Vec<_> = log.iter().flat_map(|txn| txn.statements()).collect();
    assert_eq!(statements.len(), 1);
    assert!(statements[0].sql.starts_with("UPDATE \"segments\""));
}

#[tokio::test]
async fn segment_refresh_commits_current_results_before_locking_shared_activity_projections() {
    let db = segment_result_refresh(fixtures::postgres_mock(), 1)
        .append_query_results([vec![fixtures::activity_id_row()]])
        .append_query_results([vec![fixtures::activity_analytics()]])
        .append_query_results([vec![fixtures::segment_effort()]])
        .append_query_results([vec![fixtures::activity_award_output()]])
        .append_query_results([vec![fixtures::activity_earned_awards()]])
        .append_exec_results((0..9).map(|_| written()))
        .into_connection();
    rebuild_segment_analytics_cache(&db, &[1]).await.unwrap();
    let log = db.into_transaction_log();
    let statements: Vec<_> = log.iter().flat_map(|txn| txn.statements()).collect();
    let commit = statements
        .iter()
        .position(|statement| statement.sql == "COMMIT")
        .unwrap();
    let projection_lock = statements
        .iter()
        .position(|statement| statement.sql.contains("FROM \"activities\"") && statement.sql.ends_with("FOR UPDATE"))
        .unwrap();
    assert!(commit < projection_lock);
    assert_eq!(
        statements.iter().filter(|statement| statement.sql == "COMMIT").count(),
        2
    );
}

#[test]
fn regenerated_efforts_and_changed_ranks_do_not_change_saved_awards() {
    let saved = fixtures::activity_earned_awards();
    let rows = build_activity_analytics(ActivityAnalyticsInput {
        previous: Vec::new(),
        efforts: vec![
            segment_efforts::Model {
                id: 99,
                ..fixtures::segment_effort()
            },
            segment_efforts::Model {
                id: 100,
                effort_index: 2,
                ..fixtures::segment_effort()
            },
        ],
        outputs: vec![fixtures::activity_award_output()],
        awards: vec![saved],
    });
    let summary = &rows[&42];
    assert_eq!(summary.segment_effort_count, 2);
    assert_eq!(summary.achievement_count, 1);
    assert_eq!(summary.kom_count, 1);
}

#[test]
fn saved_awards_survive_missing_efforts_and_stay_with_their_activity_owner() {
    let first = fixtures::activity_award_output();
    let empty = activity_award_outputs::Model {
        activity_id: 77,
        user_id: 8,
    };
    let rows = build_activity_analytics(ActivityAnalyticsInput {
        previous: Vec::new(),
        efforts: Vec::new(),
        outputs: vec![first, empty],
        awards: vec![fixtures::activity_earned_awards()],
    });
    assert_eq!(rows[&42].user_id, Some(7));
    assert_eq!(rows[&42].kom_count, 1);
    assert_eq!(rows[&42].segment_effort_count, 0);
    assert_eq!(rows[&77].user_id, Some(8));
    assert_eq!(rows[&77].achievement_count, 0);
}

#[test]
fn analytics_refresh_does_not_award_unevaluated_activities_from_effort_ranks() {
    let rows = build_activity_analytics(ActivityAnalyticsInput {
        previous: Vec::new(),
        efforts: vec![segment_efforts::Model {
            overall_rank: Some(1),
            user_rank: Some(1),
            ..fixtures::segment_effort()
        }],
        outputs: Vec::new(),
        awards: Vec::new(),
    });
    assert_eq!(rows[&42].segment_effort_count, 1);
    assert_eq!(rows[&42].achievement_count, 0);
}

#[tokio::test]
async fn null_legacy_highlights_preserve_awarded_and_empty_outputs_during_refresh() {
    let empty = activity_award_outputs::Model {
        activity_id: 77,
        user_id: 8,
    };
    let legacy_null = activity_analytics::Model {
        achievement_highlights_json: None,
        ..fixtures::activity_analytics()
    };
    let db = fixtures::postgres_mock()
        .append_query_results([vec![fixtures::activity_id_row()]])
        .append_query_results([vec![
            legacy_null.clone(),
            activity_analytics::Model {
                activity_id: empty.activity_id,
                user_id: empty.user_id,
                ..legacy_null
            },
        ]])
        .append_query_results([Vec::<segment_efforts::Model>::new()])
        .append_query_results([vec![fixtures::activity_award_output(), empty]])
        .append_query_results([vec![fixtures::activity_earned_awards()]])
        .append_exec_results([written()])
        .into_connection();
    rebuild_activity_analytics_cache(&db, &[42, 77]).await.unwrap();
    let log = db.into_transaction_log();
    let statements = log[0].statements();
    assert!(!statements.iter().any(|s| s.sql.contains("achievement_highlights_json")));
    let insert = statements
        .iter()
        .find(|s| s.sql.starts_with("INSERT INTO \"activity_analytics\""))
        .unwrap();
    let values = &insert.values.as_ref().unwrap().0;
    assert_eq!(values.len(), 14);
    let awarded = values.chunks(7).find(|row| row[0] == 42.into()).unwrap();
    let empty = values.chunks(7).find(|row| row[0] == 77.into()).unwrap();
    assert_eq!(
        &awarded[1..],
        &[7.into(), 0.into(), 1.into(), 1.into(), 0.into(), 0.into()]
    );
    assert_eq!(
        &empty[1..],
        &[8.into(), 0.into(), 0.into(), 0.into(), 0.into(), 0.into()]
    );
}

fn segment_result_refresh(mock: MockDatabase, segment_id: i32) -> MockDatabase {
    let segment = segments::Model {
        id: segment_id,
        ..fixtures::segment()
    };
    let effort = segment_efforts::Model {
        segment_id,
        id: segment_id,
        ..fixtures::segment_effort()
    };
    mock.append_query_results([vec![segment.clone()]])
        .append_query_results([vec![effort.clone()]])
        .append_query_results([vec![fixtures::activity()]])
        .append_query_results([vec![segment.clone()]])
        .append_query_results([vec![segment_summaries::Model {
            segment_id,
            ..fixtures::segment_summary()
        }]])
}

#[tokio::test]
async fn segment_batch_refreshes_shared_activity_award_counts_only_once() {
    let mock = segment_result_refresh(segment_result_refresh(fixtures::postgres_mock(), 1), 2);
    let db = mock
        .append_query_results([vec![fixtures::activity_id_row()]])
        .append_query_results([vec![fixtures::activity_analytics()]])
        .append_query_results([vec![
            fixtures::segment_effort(),
            segment_efforts::Model {
                id: 2,
                segment_id: 2,
                ..fixtures::segment_effort()
            },
        ]])
        .append_query_results([vec![fixtures::activity_award_output()]])
        .append_query_results([vec![fixtures::activity_earned_awards()]])
        .append_exec_results((0..17).map(|_| written()))
        .into_connection();
    rebuild_segment_analytics_cache(&db, &[2, 1, 2]).await.unwrap();
    let log = db.into_transaction_log();
    let statements: Vec<_> = log.iter().flat_map(|txn| txn.statements()).collect();
    for table in ["activity_award_outputs", "activity_earned_awards"] {
        assert_eq!(
            statements
                .iter()
                .filter(|s| s.sql.contains(&format!("FROM \"{table}\"")))
                .count(),
            1
        );
    }
    let writes: Vec<_> = statements
        .iter()
        .filter(|s| s.sql.starts_with("INSERT INTO \"activity_analytics\""))
        .collect();
    assert_eq!(writes.len(), 1);
    assert_eq!(
        &writes[0].values.as_ref().unwrap().0[..7],
        &[42.into(), 7.into(), 2.into(), 1.into(), 1.into(), 0.into(), 0.into()]
    );
    let locks: Vec<_> = statements
        .iter()
        .filter(|s| s.sql.contains("FROM \"activities\"") && s.sql.ends_with("FOR UPDATE"))
        .collect();
    assert_eq!(locks.len(), 1);
    assert_eq!(locks[0].values.as_ref().unwrap().0, vec![42.into()]);
    assert_eq!(statements.iter().filter(|s| s.sql == "COMMIT").count(), 3);
}

#[tokio::test]
async fn failed_segment_batch_leaves_award_summary_refresh_for_retry() {
    let db = segment_result_refresh(fixtures::postgres_mock(), 1)
        .append_query_errors([DbErr::Custom("segment lookup failed".into())])
        .append_exec_results((0..8).map(|_| written()))
        .into_connection();
    let error = rebuild_segment_analytics_cache(&db, &[1, 2]).await.unwrap_err();
    assert_eq!(error, DbErr::Custom("segment lookup failed".into()));
    let log = db.into_transaction_log();
    let statements: Vec<_> = log.iter().flat_map(|txn| txn.statements()).collect();
    assert!(!statements.iter().any(|s| s.sql.contains("activity_earned_awards")));
    assert!(!statements.iter().any(|s| s.sql.contains("activity_analytics")));
    assert_eq!(statements.iter().filter(|s| s.sql == "COMMIT").count(), 1);
}

#[tokio::test]
async fn segment_deletion_retries_if_an_import_adds_an_activity_outside_its_locked_scope() {
    let db = segment_mutation_scope()
        .append_query_results([vec![fixtures::segment()]])
        .append_query_results([vec![
            BTreeMap::from([("activity_id", 42.into())]),
            BTreeMap::from([("activity_id", 43.into())]),
        ]])
        .into_connection();
    let error = fixtures::segment().delete_with_derived_state(&db).await.unwrap_err();
    assert!(error.to_string().contains("changed during deletion; retry"));
    let log = db.into_transaction_log();
    let statements = log.last().unwrap().statements();
    assert_eq!(statements.last().unwrap().sql, "ROLLBACK");
    assert!(!statements.iter().any(|statement| statement.sql.starts_with("DELETE")));
}
