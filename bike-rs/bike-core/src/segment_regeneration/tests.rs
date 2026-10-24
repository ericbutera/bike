use super::*;
use crate::test_support as fixtures;
use crate::{entities::activity_recordings, object_store::ObjectOwner};

#[tokio::test]
async fn retry_rebuilds_a_removed_segment_from_pending_user_summaries() {
    let activity = fixtures::activity();
    let (objects, _) = fixtures::object_storage(ObjectOwner::ActivityImports);
    let recording = fixtures::matching_recording(&objects, &activity, Vec::new()).await;
    let db = fixtures::postgres_mock()
        .append_query_results([vec![activity.clone()]])
        .append_query_results([vec![recording]])
        .append_query_results([fixtures::segment_id_rows(&[18])])
        .append_query_results([fixtures::segment_id_rows(&[])])
        .append_query_results([Vec::<crate::entities::segment_efforts::Model>::new()])
        .append_query_results([fixtures::segment_id_rows(&[])])
        .into_connection();
    assert_eq!(match_activity(&db, &objects, &activity).await.unwrap(), vec![18]);
    let transactions = db.into_transaction_log();
    let log = format!("{transactions:?}");
    assert!(log.contains("COMMIT"));
    assert!(!log.contains("activity_event_history"));
    assert!(!log.contains("processing_pipeline"));
    assert!(!transactions
        .iter()
        .flat_map(|txn| txn.statements())
        .any(|stmt| stmt.sql.starts_with("UPDATE")));
}

#[tokio::test]
async fn a_deleted_activity_stops_matching_before_replacing_efforts() {
    let (objects, _) = fixtures::object_storage(ObjectOwner::ActivityImports);
    let db = fixtures::postgres_mock()
        .append_query_results([Vec::<activities::Model>::new()])
        .into_connection();
    assert_eq!(
        match_activity(&db, &objects, &fixtures::activity())
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

#[tokio::test]
async fn retry_scope_uses_pending_results_instead_of_every_rider_summary() {
    let activity = fixtures::activity();
    let (objects, _) = fixtures::object_storage(ObjectOwner::ActivityImports);
    let recording = fixtures::matching_recording(&objects, &activity, Vec::new()).await;
    let db = fixtures::postgres_mock()
        .append_query_results([vec![activity.clone()]])
        .append_query_results([vec![recording]])
        .append_query_results([fixtures::segment_id_rows(&[])])
        .append_query_results([fixtures::segment_id_rows(&[])])
        .append_query_results([Vec::<crate::entities::segment_efforts::Model>::new()])
        .append_query_results([fixtures::segment_id_rows(&[])])
        .into_connection();
    assert!(match_activity(&db, &objects, &activity).await.unwrap().is_empty());
    let log = db.into_transaction_log();
    let reads = log[0].statements();
    let pending = reads
        .iter()
        .find(|stmt| stmt.sql.contains("FROM \"segment_user_summaries\""))
        .unwrap();
    assert!(pending.sql.contains("\"user_id\" ="));
    assert!(pending
        .sql
        .contains("\"segments\".\"last_activity_change_at\" > \"segment_user_summaries\".\"updated_at\""));
    assert!(!reads.iter().any(|stmt| stmt.sql.starts_with("UPDATE")));
}

#[tokio::test]
async fn missing_normalized_recording_fails_matching_without_a_legacy_fallback() {
    let activity = activities::Model {
        derived_data_json: Some((&fixtures::parsed_reprocess_activity().derived_data).into()),
        ..fixtures::activity()
    };
    let (objects, _) = fixtures::object_storage(ObjectOwner::ActivityImports);
    let db = fixtures::postgres_mock()
        .append_query_results([vec![activity.clone()]])
        .append_query_results([Vec::<activity_recordings::Model>::new()])
        .into_connection();
    let error = match_activity(&db, &objects, &activity).await.unwrap_err();
    assert!(error.message.contains("no normalized recording"));
    let log = db.into_transaction_log();
    assert_eq!(log[0].statements().last().unwrap().sql, "ROLLBACK");
    assert_eq!(log[0].statements().len(), 4);
}

#[tokio::test]
async fn non_cycling_matching_does_not_require_or_read_a_recording() {
    let (objects, _) = fixtures::object_storage(ObjectOwner::ActivityImports);
    let db = fixtures::postgres_mock().into_connection();
    let activity = activities::Model {
        sport: "walk".into(),
        ..fixtures::activity()
    };
    assert!(activity_recordings::Model::matching_route(&db, &objects, &activity)
        .await
        .unwrap()
        .is_empty());
    assert!(db.into_transaction_log().is_empty());
}
