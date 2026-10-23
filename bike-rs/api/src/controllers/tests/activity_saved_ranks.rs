use super::*;
use bike_core::test_support as fixtures;

#[tokio::test]
async fn activity_efforts_return_saved_ranks_and_personal_best_without_recalculation() {
    let effort = segment_efforts::Model {
        overall_rank: Some(12),
        user_rank: Some(4),
        ..fixtures::segment_effort()
    };
    let summary = segment_user_summaries::Model {
        personal_best_duration_seconds: Some(37),
        ..fixtures::segment_user_summary()
    };
    let db = fixtures::postgres_mock()
        .append_query_results([vec![effort]])
        .append_query_results([vec![summary]])
        .append_query_results([vec![fixtures::segment()]])
        .into_connection();

    let rows = load_activity_segment_efforts(&db, 7, 42).await.unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].overall_rank, Some(12));
    assert_eq!(rows[0].personal_rank, Some(4));
    assert_eq!(rows[0].personal_best_duration_seconds, Some(37));
    assert_eq!(db.into_transaction_log().len(), 3);
}

#[tokio::test]
async fn missing_saved_rank_keeps_other_saved_results_without_loading_history() {
    for ranks in [(None, Some(4)), (Some(12), None), (None, None)] {
        let effort = segment_efforts::Model {
            overall_rank: ranks.0,
            user_rank: ranks.1,
            ..fixtures::segment_effort()
        };
        let db = fixtures::postgres_mock()
            .append_query_results([vec![effort]])
            .append_query_results([vec![fixtures::segment_user_summary()]])
            .append_query_results([vec![fixtures::segment()]])
            .into_connection();

        let rows = load_activity_segment_efforts(&db, 7, 42).await.unwrap();

        assert_eq!(rows[0].overall_rank, ranks.0);
        assert_eq!(rows[0].personal_rank, ranks.1);
        assert_eq!(rows[0].personal_best_duration_seconds, Some(60));
        assert_eq!(db.into_transaction_log().len(), 3);
    }
}

#[tokio::test]
async fn missing_saved_personal_best_stays_null_without_replacing_saved_ranks() {
    for summaries in [
        vec![],
        vec![segment_user_summaries::Model {
            personal_best_duration_seconds: None,
            ..fixtures::segment_user_summary()
        }],
    ] {
        let effort = segment_efforts::Model {
            overall_rank: Some(12),
            user_rank: Some(4),
            ..fixtures::segment_effort()
        };
        let db = fixtures::postgres_mock()
            .append_query_results([vec![effort]])
            .append_query_results([summaries])
            .append_query_results([vec![fixtures::segment()]])
            .into_connection();

        let rows = load_activity_segment_efforts(&db, 7, 42).await.unwrap();

        assert_eq!(rows[0].overall_rank, Some(12));
        assert_eq!(rows[0].personal_rank, Some(4));
        assert_eq!(rows[0].personal_best_duration_seconds, None);
        assert_eq!(db.into_transaction_log().len(), 3);
    }
}

#[tokio::test]
async fn stale_saved_ranks_and_summary_remain_available_without_history_or_repair() {
    let effort = segment_efforts::Model {
        overall_rank: Some(12),
        user_rank: Some(4),
        ..fixtures::segment_effort()
    };
    let segment = segments::Model {
        last_activity_change_at: fixtures::timestamp() + chrono::Duration::days(1),
        ..fixtures::segment()
    };
    let summary = segment_user_summaries::Model {
        personal_best_duration_seconds: Some(37),
        ..fixtures::segment_user_summary()
    };
    let db = fixtures::postgres_mock()
        .append_query_results([vec![effort]])
        .append_query_results([vec![summary]])
        .append_query_results([vec![segment]])
        .into_connection();

    let rows = load_activity_segment_efforts(&db, 7, 42).await.unwrap();

    assert_eq!(rows[0].overall_rank, Some(12));
    assert_eq!(rows[0].personal_rank, Some(4));
    assert_eq!(rows[0].personal_best_duration_seconds, Some(37));
    let log = db.into_transaction_log();
    assert_eq!(log.len(), 3);
    assert!(log
        .iter()
        .flat_map(|tx| tx.statements())
        .all(|s| s.sql.starts_with("SELECT")));
}

#[tokio::test]
async fn batched_activity_efforts_preserve_traversal_order_and_scope_saved_reads_to_each_rider() {
    let scenario = fixtures::shared_segments();
    for rider in [&scenario.creator, &scenario.contributor] {
        let mut efforts = scenario
            .efforts
            .iter()
            .filter(|effort| effort.user_id == rider.id)
            .cloned()
            .collect::<Vec<_>>();
        let activity_ids = [efforts[0].activity_id, 44];
        efforts.push(segment_efforts::Model {
            id: 99,
            activity_id: 44,
            ..efforts[0].clone()
        });
        let db = fixtures::postgres_mock()
            .append_query_results([efforts.clone()])
            .append_query_results([vec![
                segment_user_summaries::Model {
                    user_id: rider.id,
                    personal_best_duration_seconds: Some(37),
                    ..fixtures::segment_user_summary()
                },
                segment_user_summaries::Model {
                    segment_id: scenario.other_segment.id,
                    user_id: rider.id,
                    personal_best_duration_seconds: Some(45),
                    ..fixtures::segment_user_summary()
                },
            ]])
            .append_query_results([vec![scenario.segment.clone(), scenario.other_segment.clone()]])
            .into_connection();

        let rows = load_activity_segment_efforts_by_activity_ids(&db, rider.id, &activity_ids)
            .await
            .unwrap();

        assert_eq!(rows.len(), 2);
        for activity_id in activity_ids {
            let expected = efforts.iter().filter(|effort| effort.activity_id == activity_id);
            let activity_rows = &rows[&activity_id];
            assert_eq!(activity_rows.len(), expected.clone().count());
            for (row, effort) in activity_rows.iter().zip(expected) {
                assert_eq!(row.segment_id, effort.segment_id);
                assert_eq!(row.effort_index, effort.effort_index);
                assert_eq!(row.start_route_point_index, effort.start_route_point_index);
                assert_eq!(row.overall_rank, effort.overall_rank);
                assert_eq!(row.personal_rank, effort.user_rank);
            }
        }
        assert!(rows.keys().all(|id| activity_ids.contains(id)));
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 3);
        let effort_sql = &log[0].statements()[0].sql;
        assert!(effort_sql.contains("\"segment_efforts\".\"user_id\" = $1"));
        assert_eq!(
            log[0].statements()[0].values.as_ref().unwrap().0,
            vec![rider.id.into(), activity_ids[0].into(), 44.into()]
        );
        assert!(effort_sql.contains("\"segment_efforts\".\"activity_id\" IN"));
        assert!(effort_sql.contains(
            "ORDER BY \"segment_efforts\".\"activity_id\" ASC, \"segment_efforts\".\"start_route_point_index\" ASC"
        ));
        let summary_sql = &log[1].statements()[0].sql;
        assert!(summary_sql.contains("\"segment_user_summaries\".\"user_id\" = $1"));
        assert!(summary_sql.contains("\"segment_user_summaries\".\"segment_id\" IN"));
        assert_eq!(
            log[1].statements()[0].values.as_ref().unwrap().0,
            vec![rider.id.into(), 1.into(), 2.into()]
        );
    }
}

#[tokio::test]
async fn empty_activity_selection_and_unmatched_activity_do_not_load_saved_standings() {
    let db = fixtures::postgres_mock().into_connection();
    assert!(load_activity_segment_efforts_by_activity_ids(&db, 7, &[])
        .await
        .unwrap()
        .is_empty());
    assert!(db.into_transaction_log().is_empty());

    let db = fixtures::postgres_mock()
        .append_query_results([Vec::<segment_efforts::Model>::new()])
        .into_connection();
    assert!(load_activity_segment_efforts(&db, 7, 42).await.unwrap().is_empty());
    assert_eq!(db.into_transaction_log().len(), 1);
}
