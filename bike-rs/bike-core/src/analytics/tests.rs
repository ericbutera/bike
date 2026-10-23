use super::*;
use crate::test_support as fixtures;
use sea_orm::{DbErr, MockDatabase, MockExecResult};

fn effort(id: i32, user_id: i32, segment_id: i32) -> segment_efforts::Model {
    segment_efforts::Model {
        id,
        user_id,
        segment_id,
        ..fixtures::segment_effort()
    }
}

#[test]
fn equal_duration_efforts_retain_identity_order_and_independent_rider_ranks_and_bests() {
    let rebuild = build_segment_analytics(SegmentAnalyticsInput {
        efforts: vec![effort(1, 7, 1), effort(2, 8, 1), effort(3, 7, 1), effort(4, 8, 2)],
        activity_ids: vec![42],
        activity_started_at_by_id: HashMap::from([(42, fixtures::timestamp())]),
    });
    let ranks: Vec<_> = rebuild
        .effort_updates
        .iter()
        .map(|row| (row.effort_id, row.overall_rank, row.user_rank))
        .collect();
    assert_eq!(ranks, vec![(1, 1, 1), (2, 2, 1), (3, 3, 2), (4, 1, 1)]);
    for (key, count, best) in [((1, 7), 2, 1), ((1, 8), 1, 2), ((2, 8), 1, 4)] {
        let summary = &rebuild.segment_user_summary_by_key[&key];
        assert_eq!(summary.effort_count, count);
        assert_eq!(summary.personal_best_effort_id, Some(best));
        assert_eq!(summary.personal_best_duration_seconds, Some(60));
    }
    assert_eq!(rebuild.segment_summary_by_id[&1].leader_effort_id, Some(1));
    assert_eq!(rebuild.segment_summary_by_id[&1].effort_count, 3);
    assert_eq!(rebuild.segment_summary_by_id[&2].leader_effort_id, Some(4));
}

fn segment_input(efforts: Vec<segment_efforts::Model>) -> MockDatabase {
    fixtures::postgres_mock()
        .append_query_results([vec![fixtures::segment()]])
        .append_query_results([efforts])
        .append_query_results([vec![fixtures::activity()]])
        .append_query_results([vec![fixtures::segment()]])
}

#[tokio::test]
async fn a_later_rank_batch_failure_rolls_back_all_rank_writes_and_summary_clearing() {
    let db = segment_input((1..=501).map(|id| effort(id, 7, 1)).collect())
        .append_exec_results([
            fixtures::written(),
            fixtures::written(),
            MockExecResult {
                rows_affected: 500,
                ..Default::default()
            },
        ])
        .append_exec_errors([DbErr::Custom("rank batch failed".into())])
        .into_connection();
    let error = rebuild_one_segment_cache(&db, 1).await.unwrap_err();
    assert_eq!(error, DbErr::Custom("rank batch failed".into()));
    let log = db.into_transaction_log();
    let statements: Vec<_> = log.iter().flat_map(|txn| txn.statements()).collect();
    assert_eq!(statements.last().unwrap().sql, "ROLLBACK");
    assert!(!statements.iter().any(|s| s.sql == "COMMIT"));
    assert!(!statements.iter().any(|s| s.sql.starts_with("INSERT")));
}

#[tokio::test]
async fn a_rider_summary_failure_rolls_back_the_saved_ranks_and_both_summary_replacements() {
    let db = segment_input(vec![effort(1, 7, 1), effort(2, 8, 1)])
        .append_query_results([vec![fixtures::segment_summary()]])
        .append_exec_results([
            fixtures::written(),
            fixtures::written(),
            MockExecResult {
                rows_affected: 2,
                ..Default::default()
            },
        ])
        .append_exec_errors([DbErr::Custom("rider summary batch failed".into())])
        .into_connection();
    let error = rebuild_one_segment_cache(&db, 1).await.unwrap_err();
    assert_eq!(error, DbErr::Custom("rider summary batch failed".into()));
    let log = db.into_transaction_log();
    let statements: Vec<_> = log.iter().flat_map(|txn| txn.statements()).collect();
    assert_eq!(statements.last().unwrap().sql, "ROLLBACK");
    assert!(!statements.iter().any(|s| s.sql == "COMMIT"));
    assert!(statements
        .iter()
        .any(|s| s.sql.starts_with("UPDATE \"segment_efforts\"")));
    assert!(statements
        .iter()
        .any(|s| s.sql.starts_with("INSERT INTO \"segment_summaries\"")));
}

#[tokio::test]
async fn a_segment_without_efforts_clears_riders_and_keeps_an_empty_segment_summary() {
    let db = fixtures::postgres_mock()
        .append_query_results([vec![fixtures::segment()]])
        .append_query_results([Vec::<segment_efforts::Model>::new()])
        .append_query_results([vec![fixtures::segment()]])
        .append_query_results([vec![segment_summaries::Model {
            effort_count: 0,
            leader_user_id: None,
            leader_effort_id: None,
            best_duration_seconds: None,
            ..fixtures::segment_summary()
        }]])
        .append_query_results([vec![fixtures::segment()]])
        .append_query_results([Vec::<segment_efforts::Model>::new()])
        .append_query_results([vec![fixtures::segment()]])
        .append_query_results([Vec::<segment_efforts::Model>::new()])
        .append_exec_results((0..4).map(|_| fixtures::written()))
        .into_connection();
    assert!(rebuild_one_segment_cache(&db, 1).await.unwrap().is_empty());
    let log = db.into_transaction_log();
    let transaction = log
        .iter()
        .find(|txn| txn.statements().first().unwrap().sql == "BEGIN")
        .unwrap();
    let statements = transaction.statements();
    assert!(statements[1].sql.ends_with("FOR UPDATE"));
    for statement in &statements[2..4] {
        assert!(statement.sql.starts_with("DELETE FROM \"segment_"));
        assert!(statement.sql.contains("WHERE"));
        assert_eq!(statement.values.as_ref().unwrap().0, vec![1.into()]);
    }
    let summary = &statements[4];
    assert!(summary.sql.starts_with("INSERT INTO \"segment_summaries\""));
    assert_eq!(&summary.values.as_ref().unwrap().0[..2], &[1.into(), 0.into()]);
    assert_eq!(statements.last().unwrap().sql, "COMMIT");
    assert!(!statements
        .iter()
        .any(|s| s.sql.starts_with("UPDATE \"segment_efforts\"")));
    assert!(!statements
        .iter()
        .any(|s| s.sql.starts_with("INSERT INTO \"segment_user_summaries\"")));
}
