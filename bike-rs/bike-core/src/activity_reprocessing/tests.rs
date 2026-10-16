use super::*;
use crate::object_store::ObjectOwner;
use crate::test_support as fixtures;
use ::object_store::{path::Path, ObjectStoreExt};
mod fixture;

#[tokio::test]
async fn command_reads_owned_metadata_without_source_bytes() {
    let db = fixture::metadata(fixture::mock()).into_connection();
    let command = command(&db, 42, "fresh-request".into(), Priority::NewActivity)
        .await
        .unwrap();
    assert_eq!(command.activity_id, 42);
    assert_eq!(command.source.activity_import_id, 18);
    assert_eq!(command.source.command_id, "fresh-request");
    assert_eq!(
        command.source.object.sha256,
        fixtures::reprocess_artifact().checksum_sha256
    );
    assert_eq!(db.into_transaction_log().len(), 3);
}

#[tokio::test]
async fn provider_owned_imports_require_strava_sync_or_backfill() {
    let db = fixture::mock()
        .append_query_results([vec![fixtures::reprocess_activity()]])
        .append_query_results([vec![activity_imports::Model {
            source: "strava_sync".into(),
            ..fixtures::reprocess_import()
        }]])
        .into_connection();
    let error = command(&db, 42, "fresh-request".into(), Priority::NewActivity)
        .await
        .unwrap_err();
    assert_eq!(error.status.as_u16(), 400);
    assert!(error.message.contains("Strava sync or backfill"));
    assert_eq!(db.into_transaction_log().len(), 2);
}

#[tokio::test]
async fn retained_ride_reprocessing_preserves_identity_and_hands_off_removed_segments() {
    let (objects, _) = fixture::retain_source(fixtures::REPROCESS_SOURCE).await;
    let db = fixture::mutation(fixture::source_reads(fixture::mock()), Ok(1)).into_connection();
    let execution = fixtures::reprocess_execution();
    let result = reprocess(&db, &objects, execution.clone()).await.unwrap();
    execution.command.validate_result(&result).unwrap();
    assert_eq!(result.activity_id, fixtures::activity().id);
    assert_eq!(result.activity_import_id, fixtures::activity_import().id);
    assert_eq!(result, fixture::scope().result(&execution.command).unwrap());
    let log = db.into_transaction_log();
    let transaction = format!("{:?}", log.last().unwrap());
    assert!(transaction.contains("activity_event_history"));
    assert!(transaction.contains("refresh_scope"));
    assert!(transaction.contains("COMMIT"));
}

#[tokio::test]
async fn committed_retry_uses_original_scope_without_reading_source_or_current_target() {
    let db = fixture::mock()
        .append_query_results([vec![fixture::history()]])
        .into_connection();
    let (objects, _) = fixtures::object_storage(ObjectOwner::ActivityImports);
    let result = reprocess(&db, &objects, fixtures::reprocess_execution())
        .await
        .unwrap();
    assert_eq!(
        result.segments[0].segment_id,
        fixtures::segment_effort().segment_id
    );
    assert_eq!(db.into_transaction_log().len(), 1);
}

#[tokio::test]
async fn concurrent_committed_retry_rechecks_audit_before_rejecting_newer_version() {
    let db = fixture::mock()
        .append_query_results([vec![fixture::saved_activity()]])
        .append_query_results([vec![fixture::history()]])
        .into_connection();
    assert_eq!(fixture::apply(&db).await.unwrap(), fixture::scope());
}

#[tokio::test]
async fn command_key_cannot_identify_different_source_work() {
    let db = fixture::mock()
        .append_query_results([vec![fixture::history()]])
        .into_connection();
    let mut execution = fixtures::reprocess_execution();
    execution.command.source.object.sha256 = "b".repeat(64);
    let (objects, _) = fixtures::object_storage(ObjectOwner::ActivityImports);
    let error = reprocess(&db, &objects, execution).await.unwrap_err();
    assert_eq!(error.status.as_u16(), 409);
}

#[tokio::test]
async fn stale_or_deleted_target_is_rejected_before_mutation() {
    for current in [vec![fixture::saved_activity()], Vec::new()] {
        let missing = current.is_empty();
        let db = fixture::mock()
            .append_query_results([current])
            .append_query_results([Vec::<activity_event_history::Model>::new()])
            .into_connection();
        let error = fixture::apply(&db).await.unwrap_err();
        assert_eq!(error.status.as_u16(), if missing { 404 } else { 409 });
        let log = format!("{:?}", db.into_transaction_log());
        assert!(!log.contains("UPDATE \\\""));
        assert!(!log.contains("INSERT"));
        assert!(!log.contains("COMMIT"));
    }
}

#[tokio::test]
async fn audit_failure_rolls_back_the_source_transaction() {
    let db = fixture::mutation(
        fixture::mock(),
        Err(sea_orm::DbErr::Custom("fixture audit unavailable".into())),
    )
    .into_connection();
    assert_eq!(fixture::apply(&db).await.unwrap_err().status.as_u16(), 500);
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("activity_event_history"));
    assert!(log.contains("ROLLBACK"));
    assert!(!log.contains("COMMIT"));
}

#[tokio::test]
async fn concurrent_command_key_collision_rolls_back_the_losing_mutation() {
    let db = fixture::mutation(fixture::mock(), Ok(0)).into_connection();
    assert_eq!(fixture::apply(&db).await.unwrap_err().status.as_u16(), 409);
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("activity_event_history"));
    assert!(log.contains("ROLLBACK"));
    assert!(!log.contains("COMMIT"));
}

#[tokio::test]
async fn retained_bytes_must_match_submitted_identity() {
    let (objects, _) = fixture::retain_source(b"corrupt").await;
    let db = fixture::source_reads(fixture::mock()).into_connection();
    assert_eq!(
        reprocess(&db, &objects, fixtures::reprocess_execution())
            .await
            .unwrap_err()
            .status
            .as_u16(),
        500
    );
    assert_eq!(db.into_transaction_log().len(), 4); // rejected before profile or mutation
}

#[tokio::test]
async fn missing_retained_source_is_rejected_before_mutation() {
    let (objects, _) = fixtures::object_storage(ObjectOwner::ActivityImports);
    let db = fixture::source_reads(fixture::mock()).into_connection();
    let error = reprocess(&db, &objects, fixtures::reprocess_execution())
        .await
        .unwrap_err();
    assert_eq!(error.status.as_u16(), 404);
    assert_eq!(db.into_transaction_log().len(), 4);
}

#[tokio::test]
async fn supplemental_recording_uses_shared_storage_and_preserves_primary_geometry() {
    let (objects, backend) = fixture::retain_source(fixtures::REPROCESS_SOURCE).await;
    let artifact = fixtures::reprocess_recording_artifact();
    backend
        .put(
            &Path::parse(&artifact.storage_path).unwrap(),
            fixtures::REPROCESS_RECORDING_SOURCE.into(),
        )
        .await
        .unwrap();
    let db = fixture::mock()
        .append_query_results([vec![fixtures::reprocess_artifact(), artifact]])
        .into_connection();
    let mut parsed = fixtures::parsed_reprocess_activity();
    let route = parsed.derived_data.route_points.clone();
    crate::activity_import_pipeline::enrich_recording(
        &db,
        &objects,
        &fixtures::reprocess_import(),
        &fixture::artifact(),
        None,
        &mut parsed,
    )
    .await
    .unwrap();
    assert_eq!(
        parsed.derived_data.recording.environment,
        crate::activity_recording::RecordingEnvironment::Virtual
    );
    assert_eq!(parsed.derived_data.route_points, route);
    assert_eq!(db.into_transaction_log().len(), 1);
}

#[tokio::test]
async fn corrupt_supplemental_recording_is_rejected_before_mutation() {
    let (objects, backend) = fixture::retain_source(fixtures::REPROCESS_SOURCE).await;
    let artifact = fixtures::reprocess_recording_artifact();
    backend
        .put(
            &Path::parse(&artifact.storage_path).unwrap(),
            b"corrupt".as_slice().into(),
        )
        .await
        .unwrap();
    let db = fixture::metadata(
        fixture::mock().append_query_results([Vec::<activity_event_history::Model>::new()]),
    )
    .append_query_results([vec![fixtures::reprocess_artifact(), artifact]])
    .into_connection();
    let error = reprocess(&db, &objects, fixtures::reprocess_execution())
        .await
        .unwrap_err();
    assert_eq!(error.status.as_u16(), 500);
    assert_eq!(db.into_transaction_log().len(), 5);
}
