use super::*;
use crate::test_support as fixtures;

#[test]
fn repeated_attempts_occupy_one_position_per_athlete() {
    let rows = overall_top_ten(&fixtures::shared_segment_efforts());
    assert_eq!(rows.iter().map(|row| row.user_id).collect::<Vec<_>>(), vec![7, 8]);
}

#[test]
fn an_athlete_is_represented_by_their_fastest_attempt() {
    let mut efforts = fixtures::shared_segment_efforts();
    efforts.swap(0, 1);
    let rows = overall_top_ten(&efforts);
    assert_eq!(rows[0].effort_id, 1);
}

#[test]
fn standings_are_ordered_by_duration() {
    let mut efforts = fixtures::shared_segment_efforts();
    efforts.reverse();
    let rows = overall_top_ten(&efforts);
    assert_eq!(
        rows.iter().map(|row| row.duration_seconds).collect::<Vec<_>>(),
        vec![100, 300]
    );
}

#[test]
fn standings_are_limited_to_ten_athletes() {
    assert_eq!(overall_top_ten(&fixtures::distinct_athlete_efforts(11)).len(), 10);
}

#[test]
fn equal_times_choose_the_smaller_effort_identity_first() {
    let efforts: Vec<_> = fixtures::shared_segment_efforts()
        .into_iter()
        .rev()
        .map(|effort| segment_efforts::Model {
            duration_seconds: 100,
            ..effort
        })
        .collect();
    let rows = overall_top_ten(&efforts);
    assert_eq!(rows.iter().map(|row| row.effort_id).collect::<Vec<_>>(), vec![1, 3]);
}

#[test]
fn later_equal_time_attempt_keeps_the_original_effort_on_the_leaderboard() {
    let original = fixtures::segment_effort();
    let before = overall_top_ten(std::slice::from_ref(&original));
    let later = segment_efforts::Model {
        id: 2,
        activity_id: 43,
        ..original.clone()
    };
    // Present the later attempt first so input order cannot decide the tie.
    let after = overall_top_ten(&[later, original]);
    assert_eq!(after, before);
    assert_eq!(after[0].effort_id, 1);
}

#[test]
fn tied_athletes_receive_consecutive_positions() {
    let efforts: Vec<_> = fixtures::shared_segment_efforts()
        .into_iter()
        .map(|effort| segment_efforts::Model {
            duration_seconds: 100,
            ..effort
        })
        .collect();
    let rows = overall_top_ten(&efforts);
    assert_eq!(rows.iter().map(|row| row.position).collect::<Vec<_>>(), vec![1, 2]);
}

#[test]
fn removing_the_fastest_attempt_promotes_that_athletes_next_attempt() {
    let efforts: Vec<_> = fixtures::shared_segment_efforts()
        .into_iter()
        .filter(|effort| effort.id != 1)
        .collect();
    assert_eq!(overall_top_ten(&efforts)[0].effort_id, 2);
}

#[test]
fn removing_an_athletes_last_attempt_promotes_the_next_qualifying_athlete() {
    let efforts: Vec<_> = fixtures::distinct_athlete_efforts(11)
        .into_iter()
        .filter(|effort| effort.user_id != 1)
        .collect();
    assert_eq!(overall_top_ten(&efforts)[9].user_id, 11);
}

#[tokio::test]
async fn rebuild_locks_the_segment_before_reading_efforts() {
    let db = fixtures::segment_rebuild_mock(fixtures::shared_segment_efforts())
        .append_exec_results((0..2).map(|_| fixtures::written()))
        .into_connection();
    rebuild(&db, 1).await.unwrap();
    let log = db.into_transaction_log();
    let statements = log[0].statements();
    assert!(statements[1].sql.ends_with("FOR UPDATE"));
    assert!(statements[2].sql.contains("FROM \"segment_efforts\""));
}

#[tokio::test]
async fn rebuild_replaces_saved_standings_atomically() {
    let db = fixtures::segment_rebuild_mock(fixtures::shared_segment_efforts())
        .append_exec_results((0..2).map(|_| fixtures::written()))
        .into_connection();
    rebuild(&db, 1).await.unwrap();
    let log = db.into_transaction_log();
    assert_eq!(log.len(), 1);
    let statements = log[0].statements();
    assert_eq!(statements[0].sql, "BEGIN");
    assert!(statements[3]
        .sql
        .starts_with("DELETE FROM \"segment_overall_achievements\""));
    assert!(statements[4]
        .sql
        .starts_with("INSERT INTO \"segment_overall_achievements\""));
    assert_eq!(statements.last().unwrap().sql, "COMMIT");
}

#[tokio::test]
async fn rebuild_persists_the_calculated_positions() {
    let db = fixtures::segment_rebuild_mock(fixtures::shared_segment_efforts())
        .append_exec_results((0..2).map(|_| fixtures::written()))
        .into_connection();
    rebuild(&db, 1).await.unwrap();
    let log = db.into_transaction_log();
    assert_eq!(
        log[0].statements()[4].values.as_ref().unwrap().0,
        vec![
            1.into(),
            1.into(),
            7.into(),
            1.into(),
            100.into(),
            1.into(),
            2.into(),
            8.into(),
            3.into(),
            300.into()
        ]
    );
}

#[tokio::test]
async fn empty_segments_clear_old_standings_without_inserting_replacements() {
    let db = fixtures::segment_rebuild_mock(Vec::new())
        .append_exec_results([fixtures::written()])
        .into_connection();
    rebuild(&db, 1).await.unwrap();
    let log = db.into_transaction_log();
    let statements = log[0].statements();
    assert!(statements[3].sql.starts_with("DELETE"));
    assert_eq!(statements.len(), 5);
}

#[tokio::test]
async fn retries_replace_the_same_rows() {
    let db = fixtures::segment_rebuild_mock(fixtures::shared_segment_efforts())
        .append_query_results([vec![fixtures::segment()]])
        .append_query_results([fixtures::shared_segment_efforts()])
        .append_exec_results((0..4).map(|_| fixtures::written()))
        .into_connection();
    rebuild(&db, 1).await.unwrap();
    rebuild(&db, 1).await.unwrap();
    let log = db.into_transaction_log();
    assert_eq!(log[0].statements(), log[1].statements());
}

#[tokio::test]
async fn failed_publication_rolls_back() {
    let db = fixtures::segment_rebuild_mock(fixtures::shared_segment_efforts())
        .append_exec_results([fixtures::written()])
        .append_exec_errors([DbErr::Custom("fixture insert failure".into())])
        .into_connection();
    assert!(rebuild(&db, 1).await.is_err());
    let log = db.into_transaction_log();
    assert_eq!(log[0].statements().last().unwrap().sql, "ROLLBACK");
}

#[tokio::test]
async fn reads_return_the_saved_positions() {
    let scenario = fixtures::shared_segments();
    let rows = achievements::Model::for_segment(&fixtures::saved_overall_achievements_db(), scenario.segment.id)
        .await
        .unwrap();
    assert_eq!(rows, scenario.standings_for(scenario.segment.id));
}

#[tokio::test]
async fn reads_return_empty_when_no_positions_are_saved() {
    let db = fixtures::postgres_mock()
        .append_query_results([Vec::<achievements::Model>::new()])
        .into_connection();
    assert!(achievements::Model::for_segment(&db, 1).await.unwrap().is_empty());
}

#[tokio::test]
async fn reads_scope_saved_positions_to_the_requested_segment() {
    let scenario = fixtures::shared_segments();
    let db = fixtures::postgres_mock()
        .append_query_results([scenario.standings_for(scenario.other_segment.id)])
        .into_connection();
    achievements::Model::for_segment(&db, scenario.other_segment.id)
        .await
        .unwrap();
    let log = db.into_transaction_log();
    assert_eq!(
        log[0].statements()[0].values.as_ref().unwrap().0,
        vec![scenario.other_segment.id.into()]
    );
}

#[tokio::test]
async fn reads_request_saved_position_order() {
    let db = fixtures::saved_overall_achievements_db();
    achievements::Model::for_segment(&db, 1).await.unwrap();
    let log = db.into_transaction_log();
    assert!(log[0].statements()[0]
        .sql
        .contains("ORDER BY \"segment_overall_achievements\".\"position\" ASC"));
}

#[tokio::test]
async fn reads_only_query_saved_standings() {
    let db = fixtures::saved_overall_achievements_db();
    achievements::Model::for_segment(&db, 1).await.unwrap();
    let log = db.into_transaction_log();
    assert_eq!(log.len(), 1);
    assert!(!log[0].statements()[0].sql.contains("segment_efforts"));
}

#[tokio::test]
async fn a_deleted_segment_needs_no_output() {
    let db = fixtures::postgres_mock()
        .append_query_results([Vec::<segments::Model>::new()])
        .into_connection();
    rebuild(&db, 1).await.unwrap();
    let log = db.into_transaction_log();
    assert_eq!(log[0].statements().len(), 3);
}
