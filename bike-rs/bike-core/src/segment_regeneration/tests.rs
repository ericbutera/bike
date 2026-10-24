use super::*;
use crate::test_support as fixtures;

#[tokio::test]
async fn retry_rebuilds_a_removed_segment_from_existing_user_summaries() {
    let activity = fixtures::activity();
    let db = fixtures::postgres_mock()
        .append_query_results([vec![activity.clone()]])
        .append_query_results([fixtures::segment_id_rows(&[18])])
        .append_query_results([fixtures::segment_id_rows(&[])])
        .append_query_results([Vec::<crate::entities::segment_efforts::Model>::new()])
        .append_query_results([fixtures::segment_id_rows(&[])])
        .append_exec_results([fixtures::written()])
        .into_connection();
    assert_eq!(match_activity(&db, &activity).await.unwrap(), vec![18]);
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("COMMIT"));
    assert!(!log.contains("activity_event_history"));
    assert!(!log.contains("processing_pipeline"));
}

#[tokio::test]
async fn a_deleted_activity_stops_matching_before_replacing_efforts() {
    let db = fixtures::postgres_mock()
        .append_query_results([Vec::<activities::Model>::new()])
        .into_connection();
    assert_eq!(
        match_activity(&db, &fixtures::activity())
            .await
            .unwrap_err()
            .status
            .as_u16(),
        404
    );
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("ROLLBACK"));
    assert!(!log.contains("DELETE"));
}
