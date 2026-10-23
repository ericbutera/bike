use super::*;

#[test]
fn unit_happy_highlights_preserve_personal_best_in_storage() {
    let highlight = ActivityAchievementHighlight {
        segment_id: 7,
        segment_title: "Riverfront climb".into(),
        effort_index: 1,
        overall_rank: Some(3),
        personal_rank: Some(1),
        personal_best_duration_seconds: Some(355),
    };
    let stored = StoredActivityAchievementHighlights::from_items(vec![highlight.clone()]);
    let value = serde_json::to_value(&stored).unwrap();

    assert_eq!(value["v"], 1);
    assert_eq!(value["items"][0]["personal_best_duration_seconds"], 355);
    let restored: StoredActivityAchievementHighlights = serde_json::from_value(value).unwrap();
    assert_eq!(restored.items, vec![highlight]);
}

#[test]
fn unit_happy_unversioned_highlights_load_with_current_format() {
    let stored: StoredActivityAchievementHighlights = serde_json::from_value(serde_json::json!({"items": []})).unwrap();
    assert_eq!(stored, StoredActivityAchievementHighlights::default());
}

use crate::entities::{activity_analytics, activity_award_outputs, segment_efforts};
use crate::test_support as fixtures;

fn live_results() -> LiveResults {
    LiveResults {
        titles: std::collections::HashMap::from([(1, "Riverfront climb".into())]),
        overall: std::collections::HashMap::from([(1, 3)]),
        personal: std::collections::HashMap::from([(1, 1), (2, 2)]),
        personal_bests: std::collections::HashMap::from([(1, 60)]),
    }
}

#[test]
fn awards_use_live_saved_positions_instead_of_mutable_effort_ranks() {
    let effort = segment_efforts::Model {
        overall_rank: Some(1),
        user_rank: Some(3),
        ..fixtures::segment_effort()
    };
    let highlight = live_results().highlight(&effort).unwrap();
    assert_eq!(highlight.overall_rank, Some(3));
    assert_eq!(highlight.personal_rank, Some(1));
    assert_eq!(highlight.personal_best_duration_seconds, Some(60));
    assert_eq!(highlight.segment_title, "Riverfront climb");
}

#[test]
fn an_effort_absent_from_live_results_earns_no_award_even_with_old_ranks() {
    let effort = segment_efforts::Model {
        id: 99,
        overall_rank: Some(1),
        user_rank: Some(1),
        ..fixtures::segment_effort()
    };
    assert_eq!(live_results().highlight(&effort), None);
}

#[test]
fn repeated_attempts_keep_their_own_saved_positions_and_recording_order() {
    let first = fixtures::segment_effort();
    let second = segment_efforts::Model {
        id: 2,
        effort_index: 2,
        start_route_point_index: 20,
        ..first.clone()
    };
    let highlights = live_results().highlights(&[first, second]);
    assert_eq!(highlights.len(), 2);
    assert_eq!(highlights[0].effort_index, 1);
    assert_eq!(highlights[0].overall_rank, Some(3));
    assert_eq!(highlights[1].effort_index, 2);
    assert_eq!(highlights[1].overall_rank, None);
    assert_eq!(highlights[1].personal_rank, Some(2));
}

#[tokio::test]
async fn tied_later_attempt_saves_its_second_place_without_taking_the_original_kom_or_pr() {
    use crate::entities::segment_personal_achievements;
    let later = segment_efforts::Model {
        id: 2,
        ..fixtures::segment_effort()
    };
    let original_overall = fixtures::overall_achievement();
    let original_personal = fixtures::personal_achievement();
    let later_personal = segment_personal_achievements::Model {
        position: 2,
        effort_id: later.id,
        ..original_personal.clone()
    };
    assert_eq!(later.duration_seconds, original_overall.duration_seconds);
    assert_eq!(later.duration_seconds, original_personal.duration_seconds);
    let db = fixtures::postgres_mock()
        .append_query_results([vec![fixtures::activity_id_row()]])
        .append_query_results([Vec::<activity_award_outputs::Model>::new()])
        .append_query_results([Vec::<activity_analytics::Model>::new()])
        .append_query_results([vec![later]])
        .append_query_results([vec![fixtures::segment()]])
        .append_query_results([vec![original_overall]])
        .append_query_results([vec![original_personal, later_personal]])
        .append_exec_results([fixtures::written(), fixtures::written()])
        .into_connection();
    record(&db, &fixtures::activity()).await.unwrap();
    let log = db.into_transaction_log();
    let award = log[0]
        .statements()
        .iter()
        .find(|statement| statement.sql.starts_with("INSERT INTO \"activity_earned_awards\""))
        .unwrap();
    assert_eq!(
        award.values.as_ref().unwrap().0,
        vec![
            42.into(),
            1.into(),
            1.into(),
            1.into(),
            fixtures::segment().title.into(),
            1.into(),
            Option::<i32>::None.into(),
            2.into(),
            60.into(),
        ]
    );
}

#[tokio::test]
async fn retry_keeps_the_existing_decision_without_reading_live_results() {
    let db = fixtures::postgres_mock()
        .append_query_results([vec![fixtures::activity_id_row()]])
        .append_query_results([vec![fixtures::activity_award_output()]])
        .into_connection();
    record(&db, &fixtures::activity()).await.unwrap();
    let log = db.into_transaction_log();
    let statements = log[0].statements();
    assert_eq!(statements.len(), 4);
    assert_eq!(statements.last().unwrap().sql, "COMMIT");
}

#[tokio::test]
async fn an_activity_without_efforts_records_an_explicit_empty_result() {
    let db = fixtures::postgres_mock()
        .append_query_results([vec![fixtures::activity_id_row()]])
        .append_query_results([Vec::<activity_award_outputs::Model>::new()])
        .append_query_results([Vec::<activity_analytics::Model>::new()])
        .append_query_results([Vec::<segment_efforts::Model>::new()])
        .append_exec_results([fixtures::written()])
        .into_connection();
    record(&db, &fixtures::activity()).await.unwrap();
    let log = db.into_transaction_log();
    let inserts: Vec<_> = log[0]
        .statements()
        .iter()
        .filter(|statement| statement.sql.starts_with("INSERT"))
        .collect();
    assert_eq!(inserts.len(), 1);
    assert!(inserts[0].sql.starts_with("INSERT INTO \"activity_award_outputs\""));
    assert_eq!(inserts[0].values.as_ref().unwrap().0, vec![42.into(), 7.into()]);
}

#[tokio::test]
async fn first_decision_filters_unplaced_attempts_and_saves_exact_award_order() {
    use crate::entities::{segment_overall_achievements, segment_personal_achievements, segments};
    let efforts: Vec<_> = [(1, 11, 1), (2, 12, 1), (3, 13, 1), (4, 11, 2), (5, 14, 1)]
        .into_iter()
        .map(|(id, segment_id, effort_index)| segment_efforts::Model {
            id,
            segment_id,
            effort_index,
            start_route_point_index: id * 10,
            end_route_point_index: id * 10 + 1,
            ..fixtures::segment_effort()
        })
        .collect();
    let titles = [11, 12, 13, 14].map(|id| segments::Model {
        id,
        title: format!("Segment {id}"),
        ..fixtures::segment()
    });
    let overall: Vec<_> = [(13, 3, 1, 40), (14, 5, 10, 50)]
        .into_iter()
        .map(
            |(segment_id, effort_id, position, duration_seconds)| segment_overall_achievements::Model {
                segment_id,
                effort_id,
                position,
                duration_seconds,
                ..fixtures::overall_achievement()
            },
        )
        .collect();
    let personal: Vec<_> = [(11, 1, 1, 60), (11, 4, 2, 70), (13, 3, 1, 40), (14, 5, 1, 50)]
        .into_iter()
        .map(
            |(segment_id, effort_id, position, duration_seconds)| segment_personal_achievements::Model {
                segment_id,
                effort_id,
                position,
                duration_seconds,
                ..fixtures::personal_achievement()
            },
        )
        .collect();
    let db = fixtures::postgres_mock()
        .append_query_results([vec![fixtures::activity_id_row()]])
        .append_query_results([Vec::<activity_award_outputs::Model>::new()])
        .append_query_results([Vec::<activity_analytics::Model>::new()])
        .append_query_results([efforts])
        .append_query_results([titles])
        .append_query_results([overall])
        .append_query_results([personal])
        .append_exec_results([fixtures::written(), fixtures::written()])
        .into_connection();
    record(&db, &fixtures::activity()).await.unwrap();
    let log = db.into_transaction_log();
    let insert = log[0]
        .statements()
        .iter()
        .find(|statement| statement.sql.starts_with("INSERT INTO \"activity_earned_awards\""))
        .unwrap();
    // recording position, display position, segment, attempt, overall, personal, best.
    let expected: Vec<sea_orm::Value> = [
        (2, 1, 13, 1, Some(1), 1, 40),
        (4, 2, 14, 1, Some(10), 1, 50),
        (1, 3, 11, 1, None, 1, 60),
        (3, 4, 11, 2, None, 2, 60),
    ]
    .into_iter()
    .flat_map(|(position, display, segment, attempt, overall, personal, best)| {
        [
            42.into(),
            position.into(),
            display.into(),
            segment.into(),
            format!("Segment {segment}").into(),
            attempt.into(),
            overall.into(),
            personal.into(),
            best.into(),
        ]
    })
    .collect();
    assert_eq!(insert.values.as_ref().unwrap().0, expected);
}

fn legacy_decision(items: Vec<ActivityAchievementHighlight>) -> sea_orm::MockDatabase {
    fixtures::postgres_mock()
        .append_query_results([vec![fixtures::activity_id_row()]])
        .append_query_results([Vec::<activity_award_outputs::Model>::new()])
        .append_query_results([vec![activity_analytics::Model {
            achievement_highlights_json: Some(StoredActivityAchievementHighlights::from_items(items)),
            ..fixtures::activity_analytics()
        }]])
}

#[tokio::test]
async fn legacy_awards_convert_without_reading_live_results_and_clear_only_after_saving() {
    let first = ActivityAchievementHighlight {
        overall_rank: None,
        personal_rank: Some(2),
        ..fixtures::activity_earned_awards().into_highlight()
    };
    let second = ActivityAchievementHighlight {
        effort_index: 2,
        overall_rank: Some(1),
        personal_rank: Some(1),
        ..first.clone()
    };
    let db = legacy_decision(vec![first.clone(), second])
        .append_exec_results((0..3).map(|_| fixtures::written()))
        .into_connection();
    record(&db, &fixtures::activity()).await.unwrap();
    let log = db.into_transaction_log();
    let statements = log[0].statements();
    let insert = statements
        .iter()
        .position(|s| s.sql.starts_with("INSERT INTO \"activity_earned_awards\""))
        .unwrap();
    let values = &statements[insert].values.as_ref().unwrap().0;
    assert_eq!(values.len(), 18);
    assert_eq!(values[4], first.segment_title.into());
    assert_eq!(
        [values[1].clone(), values[2].clone(), values[5].clone()],
        [2.into(), 1.into(), 2.into()]
    );
    assert_eq!(
        [values[10].clone(), values[11].clone(), values[14].clone()],
        [1.into(), 2.into(), 1.into()]
    );
    assert_eq!(values[6], 1.into());
    assert_eq!(values[16], 2.into());
    assert!(statements[insert + 1].sql.starts_with("UPDATE \"activity_analytics\""));
    assert_eq!(statements.last().unwrap().sql, "COMMIT");
}

#[tokio::test]
async fn legacy_empty_outcome_is_saved_without_reading_efforts() {
    let db = legacy_decision(Vec::new())
        .append_exec_results((0..2).map(|_| fixtures::written()))
        .into_connection();
    record(&db, &fixtures::activity()).await.unwrap();
    let log = db.into_transaction_log();
    let statements = log[0].statements();
    assert_eq!(statements.iter().filter(|s| s.sql.starts_with("INSERT")).count(), 1);
    assert!(statements
        .iter()
        .any(|s| s.sql.starts_with("UPDATE \"activity_analytics\"")));
    assert_eq!(statements.last().unwrap().sql, "COMMIT");
}

#[tokio::test]
async fn failed_conversion_rolls_back_and_keeps_legacy_input_for_retry() {
    let db = legacy_decision(vec![fixtures::activity_earned_awards().into_highlight()])
        .append_exec_results([fixtures::written()])
        .append_exec_errors([sea_orm::DbErr::Custom("award write failed".into())])
        .into_connection();
    assert!(record(&db, &fixtures::activity()).await.is_err());
    let log = db.into_transaction_log();
    let statements = log[0].statements();
    assert_eq!(statements.last().unwrap().sql, "ROLLBACK");
    assert!(!statements.iter().any(|s| s.sql.starts_with("UPDATE")));
}

#[tokio::test]
async fn live_award_inputs_use_two_rider_scoped_queries_across_segments_and_attempts() {
    use crate::entities::{segment_overall_achievements, segment_personal_achievements, segments};
    for user_id in [7, 8] {
        let efforts = vec![
            segment_efforts::Model {
                user_id,
                ..fixtures::segment_effort()
            },
            segment_efforts::Model {
                id: 2,
                segment_id: 2,
                user_id,
                ..fixtures::segment_effort()
            },
            segment_efforts::Model {
                id: 3,
                effort_index: 2,
                user_id,
                ..fixtures::segment_effort()
            },
        ];
        let db = fixtures::postgres_mock()
            .append_query_results([vec![
                fixtures::segment(),
                segments::Model {
                    id: 2,
                    ..fixtures::segment()
                },
            ]])
            .append_query_results([vec![
                segment_overall_achievements::Model {
                    user_id,
                    position: 3,
                    ..fixtures::overall_achievement()
                },
                segment_overall_achievements::Model {
                    user_id,
                    effort_id: 2,
                    segment_id: 2,
                    ..fixtures::overall_achievement()
                },
            ]])
            .append_query_results([vec![
                segment_personal_achievements::Model {
                    user_id,
                    position: 2,
                    ..fixtures::personal_achievement()
                },
                segment_personal_achievements::Model {
                    user_id,
                    effort_id: 3,
                    duration_seconds: 55,
                    ..fixtures::personal_achievement()
                },
                segment_personal_achievements::Model {
                    user_id,
                    effort_id: 2,
                    segment_id: 2,
                    ..fixtures::personal_achievement()
                },
            ]])
            .into_connection();
        let results = LiveResults::load(&db, user_id, &efforts).await.unwrap();
        let highlights = results.highlights(&efforts);
        assert_eq!(
            highlights
                .iter()
                .map(|h| (h.segment_id, h.effort_index, h.overall_rank, h.personal_rank))
                .collect::<Vec<_>>(),
            vec![
                (1, 1, Some(3), Some(2)),
                (2, 1, Some(1), Some(1)),
                (1, 2, None, Some(1))
            ]
        );
        assert_eq!(results.personal_bests[&1], 55);
        assert_eq!(results.personal_bests[&2], 60);
        assert_bulk_standings_queries(db, user_id);
    }
}

#[tokio::test]
async fn bulk_standings_skip_the_database_when_no_segments_are_requested() {
    use crate::entities::{segment_overall_achievements, segment_personal_achievements};
    let db = fixtures::postgres_mock().into_connection();
    assert!(segment_overall_achievements::Model::for_user_segments(&db, 7, &[])
        .await
        .unwrap()
        .is_empty());
    assert!(segment_personal_achievements::Model::for_user_segments(&db, 7, &[])
        .await
        .unwrap()
        .is_empty());
    assert!(db.into_transaction_log().is_empty());
}

fn assert_bulk_standings_queries(db: sea_orm::DatabaseConnection, user_id: i32) {
    let log = db.into_transaction_log();
    let queries: Vec<_> = log.iter().flat_map(|txn| txn.statements()).collect();
    assert_eq!(queries.len(), 3); // Titles, overall positions, personal positions.
    for (query, table) in queries[1..]
        .iter()
        .zip(["segment_overall_achievements", "segment_personal_achievements"])
    {
        assert!(query.sql.contains(&format!("FROM \"{table}\"")));
        assert!(query.sql.contains(&format!("\"{table}\".\"user_id\" = $1")));
        assert!(query.sql.contains(&format!("\"{table}\".\"segment_id\" IN ($2, $3)")));
        assert_eq!(
            query.values.as_ref().unwrap().0,
            vec![user_id.into(), 1.into(), 2.into()]
        );
    }
}
