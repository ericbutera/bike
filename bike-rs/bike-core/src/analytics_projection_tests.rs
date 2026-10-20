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
        .append_query_results([vec![segment_efforts::Model {
            overall_rank: Some(1),
            user_rank: Some(1),
            ..fixtures::segment_effort()
        }]])
        .append_query_results([vec![fixtures::segment()]])
        .append_query_results([Vec::<segment_user_summaries::Model>::new()])
        .append_exec_results([written()])
}

#[tokio::test]
async fn shared_activity_projection_locks_before_reading_ranks_and_replaces_atomically() {
    let db = ranked_input().append_exec_results([written()]).into_connection();
    rebuild_activity_analytics_cache(&db, &[99, 42, 99, 0]).await.unwrap();
    let log = db.into_transaction_log();
    let statements = log[0].statements();
    assert_eq!(statements.first().unwrap().sql, "BEGIN");
    let lock = &statements[1];
    assert!(lock.sql.contains("ORDER BY \"activities\".\"id\" ASC FOR UPDATE"));
    assert_eq!(lock.values.as_ref().unwrap().0, vec![42.into(), 99.into()]);
    assert!(statements[2].sql.contains("FROM \"segment_efforts\""));
    let delete = statements
        .iter()
        .position(|statement| statement.sql.starts_with("DELETE FROM \"activity_analytics\""))
        .unwrap();
    let insert = statements
        .iter()
        .position(|statement| statement.sql.starts_with("INSERT INTO \"activity_analytics\""))
        .unwrap();
    assert!(delete > 2 && insert > delete);
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
async fn no_remaining_efforts_store_completed_empty_in_the_same_transaction() {
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixtures::activity_id_row()]])
        .append_query_results([Vec::<segment_efforts::Model>::new()])
        .append_exec_results([written(), written()])
        .into_connection();
    rebuild_activity_analytics_cache(&db, &[42]).await.unwrap();
    let log = db.into_transaction_log();
    let statements = log[0].statements();
    assert_eq!(statements.first().unwrap().sql, "BEGIN");
    assert!(statements[1].sql.ends_with("FOR UPDATE"));
    assert!(statements[2].sql.contains("FROM \"segment_efforts\""));
    assert!(statements[3].sql.starts_with("DELETE FROM \"activity_analytics\""));
    let insert = &statements[4];
    assert!(insert.sql.starts_with("INSERT INTO \"activity_analytics\""));
    assert_eq!(
        &insert.values.as_ref().unwrap().0[..8],
        &[
            42.into(),
            7.into(),
            0.into(),
            0.into(),
            0.into(),
            0.into(),
            0.into(),
            serde_json::json!({"v": 1}).into()
        ],
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
        .append_query_results([vec![fixtures::activity_id_row()]])
        .append_query_results([Vec::<segment_efforts::Model>::new()])
        .append_exec_results((0..6).map(|_| written()))
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
async fn segment_title_changes_lock_affected_activities_before_the_segment() {
    let db = segment_mutation_scope()
        .append_query_results([vec![segments::Model {
            title: "Renamed segment".into(),
            ..fixtures::segment()
        }]])
        .append_query_results([vec![fixtures::activity_id_row()]])
        .append_query_results([Vec::<segment_efforts::Model>::new()])
        .append_exec_results([written(), written()])
        .into_connection();
    let segment = fixtures::segment()
        .update_details(&db, Some("Renamed segment"), None, None)
        .await
        .unwrap();
    assert_eq!(segment.title, "Renamed segment");
    let log = db.into_transaction_log();
    let statements: Vec<_> = log.iter().flat_map(|txn| txn.statements()).collect();
    let lock = statements
        .iter()
        .position(|statement| statement.sql.ends_with("FOR UPDATE"))
        .unwrap();
    let update = statements
        .iter()
        .position(|statement| statement.sql.starts_with("UPDATE \"segments\""))
        .unwrap();
    assert!(lock < update);
    assert_eq!(statements.last().unwrap().sql, "COMMIT");
}

#[tokio::test]
async fn segment_refresh_commits_ranks_before_locking_shared_activity_projections() {
    let ranked = segment_efforts::Model {
        overall_rank: Some(1),
        user_rank: Some(1),
        ..fixtures::segment_effort()
    };
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixtures::segment()]])
        .append_query_results([vec![fixtures::segment_effort()]])
        .append_query_results([vec![fixtures::activity()]])
        .append_query_results([vec![fixtures::segment()]])
        .append_query_results([vec![ranked.clone()]])
        .append_query_results([vec![fixtures::segment_summary()]])
        .append_query_results([vec![fixtures::segment_user_summary()]])
        .append_query_results([vec![fixtures::activity_id_row()]])
        .append_query_results([vec![ranked]])
        .append_query_results([vec![fixtures::segment()]])
        .append_query_results([vec![fixtures::segment_user_summary()]])
        // Clear both segment summaries, update effort ranks, replace activity output.
        .append_exec_results((0..5).map(|_| written()))
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

#[tokio::test]
async fn deleted_activities_do_not_get_completion_rows() {
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([Vec::<activities::Model>::new()])
        .append_query_results([Vec::<segment_efforts::Model>::new()])
        .append_exec_results([written()])
        .into_connection();
    rebuild_activity_analytics_cache(&db, &[42]).await.unwrap();
    let log = db.into_transaction_log();
    assert!(!log
        .iter()
        .flat_map(|txn| txn.statements())
        .any(|statement| statement.sql.starts_with("INSERT")));
}

#[tokio::test]
async fn changed_segment_input_requests_retry_without_publishing_stale_ranks() {
    let original = fixtures::segment();
    let changed = segments::Model {
        last_activity_change_at: original.updated_at + chrono::Duration::seconds(1),
        ..original.clone()
    };
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![original]])
        .append_query_results([Vec::<segment_efforts::Model>::new()])
        .append_query_results([vec![changed]])
        .into_connection();
    let error = rebuild_segment_analytics_cache(&db, &[1]).await.unwrap_err();
    assert!(error.to_string().contains("changed during rank rebuild"));
    let log = db.into_transaction_log();
    let statements: Vec<_> = log.iter().flat_map(|txn| txn.statements()).collect();
    assert_eq!(statements.last().unwrap().sql, "ROLLBACK");
    assert!(!statements.iter().any(|statement| statement.sql.starts_with("DELETE")));
}

#[test]
fn repeated_attempts_keep_ranks_pr_context_and_achievement_identity_on_rebuild() {
    let first = segment_efforts::Model {
        duration_seconds: 60,
        ..fixtures::segment_effort()
    };
    let second = segment_efforts::Model {
        id: first.id + 1,
        effort_index: 2,
        duration_seconds: 90,
        ..first.clone()
    };
    for _ in 0..2 {
        let rebuild = build_segment_analytics(SegmentAnalyticsInput {
            efforts: vec![first.clone(), second.clone()],
            activity_ids: vec![42],
            activity_started_at_by_id: HashMap::from([(42, fixtures::timestamp())]),
        });
        let personal_best = &rebuild.segment_user_summary_by_key[&(first.segment_id, first.user_id)];
        assert_eq!(personal_best.personal_best_duration_seconds, Some(60));
        let efforts = rebuild
            .effort_updates
            .into_iter()
            .map(|update| segment_efforts::Model {
                overall_rank: Some(update.overall_rank),
                user_rank: Some(update.user_rank),
                ..update.effort
            })
            .collect();
        let result = build_activity_analytics(
            vec![(42, 7)],
            ActivityAnalyticsInput {
                efforts,
                segment_title_by_id: HashMap::from([(first.segment_id, "Climb".into())]),
                personal_best_by_key: HashMap::from([((first.segment_id, 7), Some(60))]),
            },
        );
        let activity = &result[&42];
        assert_eq!((activity.segment_effort_count, activity.achievement_count), (2, 2));
        assert_eq!((activity.kom_count, activity.top_10_count), (1, 1));
        let highlights = &activity.achievement_highlights;
        assert_eq!(highlights.len(), 2);
        assert_eq!(
            (
                highlights[0].effort_index,
                highlights[0].overall_rank,
                highlights[0].personal_rank
            ),
            (1, Some(1), Some(1))
        );
        assert_eq!(
            (
                highlights[1].effort_index,
                highlights[1].overall_rank,
                highlights[1].personal_rank
            ),
            (2, Some(2), Some(2))
        );
        assert!(highlights.iter().all(|h| h.personal_best_duration_seconds == Some(60)));
    }
}

#[tokio::test]
async fn a_deleted_segment_needs_no_retry_or_rank_writes() {
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixtures::segment()]])
        .append_query_results([Vec::<segment_efforts::Model>::new()])
        .append_query_results([Vec::<segments::Model>::new()])
        .into_connection();
    rebuild_segment_analytics_cache(&db, &[1]).await.unwrap();
    let log = db.into_transaction_log();
    assert!(!log
        .iter()
        .flat_map(|txn| txn.statements())
        .any(|statement| statement.sql.starts_with("DELETE")));
}
