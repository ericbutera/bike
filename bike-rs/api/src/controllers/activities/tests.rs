use super::*;
use bike_core::test_support as fixtures;
use chrono::Duration;

fn ranked_effort() -> segment_efforts::Model {
    segment_efforts::Model {
        overall_rank: Some(12),
        user_rank: Some(12),
        ..fixtures::segment_effort()
    }
}

fn input(efforts: Vec<segment_efforts::Model>) -> ActivitySegmentEffortInput {
    ActivitySegmentEffortInput {
        efforts,
        segments_by_id: HashMap::from([(1, fixtures::segment())]),
        user_summary_by_segment: HashMap::from([(1, fixtures::segment_user_summary())]),
    }
}

#[tokio::test]
async fn missing_activity_ranks_do_not_read_segment_history() {
    let db = fixtures::postgres_mock()
        .append_query_results([vec![fixtures::segment_effort()]])
        .append_query_results([vec![fixtures::segment_user_summary()]])
        .append_query_results([vec![fixtures::segment()]])
        .into_connection();
    let result = load_activity_segment_efforts(&db, 7, 42).await.unwrap();
    assert_eq!(result.state, ActivitySegmentRanksState::Missing);
    assert_eq!(result.efforts.len(), 1);
    assert_eq!(result.efforts[0].duration_seconds, 60);
    assert_eq!(result.efforts[0].overall_rank, None);
    assert_eq!(result.efforts[0].personal_rank, None);
    assert_eq!(db.into_transaction_log().len(), 3);
}

#[tokio::test]
async fn ready_and_stale_reads_keep_saved_ranks_and_only_query_this_activity() {
    for stale in [false, true] {
        let segment = segments::Model {
            last_activity_change_at: fixtures::timestamp() + Duration::seconds(i64::from(stale)),
            ..fixtures::segment()
        };
        let db = fixtures::postgres_mock()
            .append_query_results([vec![ranked_effort()]])
            .append_query_results([vec![fixtures::segment_user_summary()]])
            .append_query_results([vec![segment]])
            .into_connection();
        let result = load_activity_segment_efforts(&db, 7, 42).await.unwrap();
        let expected = if stale {
            ActivitySegmentRanksState::Updating
        } else {
            ActivitySegmentRanksState::Ready
        };
        assert_eq!(result.state, expected);
        assert_eq!(result.efforts[0].overall_rank, Some(12));
        assert_eq!(result.efforts[0].personal_rank, Some(12));
        assert_eq!(result.efforts[0].personal_best_duration_seconds, Some(60));

        let log = db.into_transaction_log();
        assert_eq!(log.len(), 3);
        let sql = &log[0].statements()[0].sql;
        assert!(sql.contains("\"segment_efforts\".\"user_id\" ="));
        assert!(sql.contains("\"segment_efforts\".\"activity_id\" IN"));
        assert_eq!(
            log[0].statements()[0].values.as_ref().unwrap().0,
            vec![7.into(), 42.into()]
        );
    }
}

#[test]
fn repeated_attempts_preserve_ranks_times_and_route_boundaries() {
    let first = ranked_effort();
    let second = segment_efforts::Model {
        id: 2,
        effort_index: 2,
        duration_seconds: 90,
        start_route_point_index: 10,
        end_route_point_index: 20,
        overall_rank: Some(20),
        user_rank: Some(20),
        ..first.clone()
    };
    let input = input(vec![first, second]);
    assert_eq!(activity_segment_ranks_state(&input), ActivitySegmentRanksState::Ready);
    let grouped = group_activity_segment_efforts(input);
    let attempts = &grouped[&42];
    assert_eq!(attempts.len(), 2);
    assert_eq!((attempts[0].effort_index, attempts[0].overall_rank), (1, Some(12)));
    assert_eq!((attempts[1].effort_index, attempts[1].overall_rank), (2, Some(20)));
    assert_eq!(attempts[1].duration_seconds, 90);
    assert_eq!(
        (attempts[1].start_route_point_index, attempts[1].end_route_point_index),
        (10, 20)
    );
}

#[test]
fn missing_context_is_explicit_without_discarding_saved_rank_values() {
    let mut input = input(vec![ranked_effort()]);
    input.user_summary_by_segment.clear();
    assert_eq!(activity_segment_ranks_state(&input), ActivitySegmentRanksState::Missing);
    let grouped = group_activity_segment_efforts(input);
    assert_eq!(grouped[&42][0].overall_rank, Some(12));
    assert_eq!(grouped[&42][0].personal_best_duration_seconds, None);
}

#[test]
fn missing_ranks_take_precedence_over_stale_values() {
    let missing = segment_efforts::Model {
        id: 2,
        ..fixtures::segment_effort()
    };
    let mut input = input(vec![ranked_effort(), missing]);
    input.segments_by_id.get_mut(&1).unwrap().last_activity_change_at += Duration::seconds(1);
    assert_eq!(activity_segment_ranks_state(&input), ActivitySegmentRanksState::Missing);
}

#[test]
fn stale_personal_best_marks_the_section_updating() {
    let mut input = input(vec![ranked_effort()]);
    input.user_summary_by_segment.get_mut(&1).unwrap().updated_at -= Duration::seconds(1);
    assert_eq!(
        activity_segment_ranks_state(&input),
        ActivitySegmentRanksState::Updating
    );
    input
        .user_summary_by_segment
        .get_mut(&1)
        .unwrap()
        .personal_best_duration_seconds = None;
    assert_eq!(activity_segment_ranks_state(&input), ActivitySegmentRanksState::Missing);
}

#[tokio::test]
async fn zero_efforts_distinguish_completed_empty_missing_and_outdated_results() {
    for (count, expected) in [
        (None, ActivitySegmentRanksState::Missing),
        (Some(0), ActivitySegmentRanksState::Ready),
        (Some(2), ActivitySegmentRanksState::Updating),
    ] {
        let rows: Vec<_> = count
            .into_iter()
            .map(|count| activity_analytics::Model {
                segment_effort_count: count,
                ..fixtures::activity_analytics()
            })
            .collect();
        let db = fixtures::postgres_mock()
            .append_query_results([Vec::<segment_efforts::Model>::new()])
            .append_query_results([rows])
            .into_connection();
        let result = load_activity_segment_efforts(&db, 7, 42).await.unwrap();
        assert_eq!(result.state, expected);
        assert!(result.efforts.is_empty());
        assert_eq!(db.into_transaction_log().len(), 2);
    }
}

#[test]
fn detail_state_serializes_and_summary_omits_it() {
    let model = fixtures::activity();
    let summary = ActivityResponse::from_summary_with_achievement_highlights(model.clone(), Vec::new());
    assert!(serde_json::to_value(summary)
        .unwrap()
        .get("segment_ranks_state")
        .is_none());
    let mut detail = ActivityResponse::from_detail(model, Vec::new());
    for state in [
        ActivitySegmentRanksState::Ready,
        ActivitySegmentRanksState::Missing,
        ActivitySegmentRanksState::Updating,
    ] {
        detail.segment_ranks_state = Some(state);
        let json = serde_json::to_value(&detail).unwrap();
        assert_eq!(json["segment_ranks_state"], serde_json::to_value(state).unwrap());
        assert_eq!(json["title"], fixtures::activity().title);
    }
}
