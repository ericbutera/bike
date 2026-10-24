use super::*;
use crate::test_support as fixtures;

#[tokio::test]
async fn reprocessing_an_unchanged_activity_retains_saved_efforts_without_writes() {
    let route = fixtures::parsed_reprocess_activity().derived_data.route_points;
    let matches = match_segment_efforts(&route, &route);
    assert_eq!(matches.len(), 1);
    let matched = &matches[0];
    let existing = segment_efforts::Model {
        id: 77,
        start_route_point_index: matched.start_route_point_index,
        end_route_point_index: matched.end_route_point_index,
        start_elapsed_seconds: matched.start_elapsed_seconds,
        end_elapsed_seconds: matched.end_elapsed_seconds,
        duration_seconds: matched.duration_seconds,
        distance_meters: matched.distance_meters,
        overall_rank: Some(1),
        user_rank: Some(1),
        ..fixtures::segment_effort()
    };
    let segment = segments::Model {
        route_data_json: Some(serialize_segment_route_points(&route)),
        ..fixtures::segment()
    };
    let db = fixtures::postgres_mock()
        .append_query_results([vec![segment], Vec::new()])
        .append_query_results([vec![existing.clone()]])
        .into_connection();
    replace_segment_efforts_for_activity(&db, existing.user_id, existing.activity_id, &route)
        .await
        .unwrap();
    let log = db.into_transaction_log();
    let statements: Vec<_> = log.iter().flat_map(|transaction| transaction.statements()).collect();
    assert!(statements.iter().all(|statement| {
        let is_read = statement.sql.starts_with("SELECT");
        let is_boundary = statement.sql == "BEGIN" || statement.sql == "COMMIT";
        is_read || is_boundary
    }));
    let efforts_read = statements
        .iter()
        .find(|statement| statement.sql.contains("FROM \"segment_efforts\""))
        .unwrap();
    assert!(efforts_read.sql.contains("\"user_id\" ="));
    assert!(efforts_read.sql.contains("\"activity_id\" IN"));
}

#[tokio::test]
async fn losing_all_matches_removes_only_saved_ids_in_the_activity_transaction() {
    let effort = fixtures::segment_effort();
    let db = fixtures::postgres_mock()
        .append_query_results([vec![effort.clone()]])
        .append_exec_results([fixtures::written()])
        .into_connection();
    replace_segment_efforts_for_activity(&db, effort.user_id, effort.activity_id, &[])
        .await
        .unwrap();
    let log = db.into_transaction_log();
    let statements = log[0].statements();
    assert_eq!(statements.first().unwrap().sql, "BEGIN");
    let deletion = statements
        .iter()
        .find(|statement| statement.sql.starts_with("DELETE"))
        .unwrap();
    assert_eq!(
        deletion.values.as_ref().unwrap().0,
        vec![effort.user_id.into(), effort.activity_id.into(), effort.id.into()]
    );
    assert_eq!(statements.last().unwrap().sql, "COMMIT");
}

#[tokio::test]
async fn failed_reconciliation_rolls_back_without_publishing_partial_efforts() {
    let route = fixtures::parsed_reprocess_activity().derived_data.route_points;
    let segment = segments::Model {
        route_data_json: Some(serialize_segment_route_points(&route)),
        ..fixtures::segment()
    };
    let db = fixtures::postgres_mock()
        .append_query_results([vec![segment], Vec::new()])
        .append_query_results([Vec::<segment_efforts::Model>::new()])
        .append_exec_errors([sea_orm::DbErr::Custom("fixture effort write failed".into())])
        .into_connection();
    assert!(replace_segment_efforts_for_activity(&db, 7, 42, &route).await.is_err());
    let log = db.into_transaction_log();
    assert_eq!(log[0].statements().last().unwrap().sql, "ROLLBACK");
}

#[tokio::test]
async fn mixed_profile_attempts_keep_their_recording_order_and_saved_ids_on_retry() {
    let laps = [
        (0.0, 600),
        (60.0, 540),
        (110.0, 480),
        (110.0, 420),
        (60.0, 360),
        (0.0, 300),
    ];
    let recording = super::traversals::lap_recording(&laps);
    let segment = segments::Model {
        route_data_json: Some(serialize_segment_route_points(&recording[..41])),
        ..fixtures::segment()
    };
    let starts = [0, 1200, 2340, 3420, 4440, 5400];
    let saved: Vec<_> = laps
        .iter()
        .zip(starts)
        .enumerate()
        .map(|(index, ((_, duration), start))| {
            let attempt = index as i32 + 1;
            segment_efforts::Model {
                id: 70 + attempt,
                effort_index: attempt,
                start_route_point_index: index as i32 * 41,
                end_route_point_index: index as i32 * 41 + 40,
                start_elapsed_seconds: start,
                end_elapsed_seconds: start + duration,
                duration_seconds: *duration,
                distance_meters: Some(2000.0),
                overall_rank: Some(7 - attempt),
                user_rank: Some(7 - attempt),
                ..fixtures::segment_effort()
            }
        })
        .collect();
    let db = fixtures::postgres_mock()
        .append_query_results([vec![segment], Vec::new()])
        .append_query_results([saved])
        .into_connection();

    replace_segment_efforts_for_activity(&db, 7, 42, &recording)
        .await
        .unwrap();

    let log = db.into_transaction_log();
    let statements = log[0].statements();
    assert_eq!(statements.first().unwrap().sql, "BEGIN");
    assert_eq!(statements.last().unwrap().sql, "COMMIT");
    assert!(statements[1..statements.len() - 1]
        .iter()
        .all(|statement| statement.sql.starts_with("SELECT")));
}
