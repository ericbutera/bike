use super::*;
use crate::{object_store::ObjectOwner, test_support as fixtures};
use ::object_store::{path::Path, ObjectStoreExt};
mod fixture;

#[tokio::test]
async fn command_reads_owned_metadata_without_source_bytes() {
    let db = fixture::metadata(fixture::mock()).into_connection();
    let input = command(&db, 42, "fresh-request".into(), Priority::NewActivity)
        .await
        .unwrap();
    assert_eq!(input.activity_id, 42);
    assert_eq!(input.source.activity_import_id, 18);
    assert_eq!(input.source.command_id, "fresh-request");
    assert_eq!(
        input.source.object.sha256,
        fixtures::reprocess_artifact().checksum_sha256
    );
    assert_eq!(db.into_transaction_log().len(), 3);
}

#[tokio::test]
async fn provider_owned_imports_require_native_strava_work() {
    let db = fixture::mock()
        .append_query_results([vec![fixtures::reprocess_activity()]])
        .append_query_results([vec![activity_imports::Model {
            source: "strava_sync".into(),
            ..fixtures::reprocess_import()
        }]])
        .into_connection();
    let error = command(&db, 42, "request".into(), Priority::NewActivity)
        .await
        .unwrap_err();
    assert_eq!(error.status.as_u16(), 400);
}

#[tokio::test]
async fn repeated_reprocessing_reads_the_file_and_preserves_activity_and_import_identity() {
    let (objects, _) = fixture::retain_source(fixtures::REPROCESS_SOURCE).await;
    for _ in 0..2 {
        let db = fixture::mutation(fixture::source_reads(fixture::mock()), Ok(1)).into_connection();
        let result = reprocess(&db, &objects, fixtures::reprocess_execution()).await.unwrap();
        assert_eq!(result.activity_id, 42);
        assert_eq!(result.activity_import_id, 18);
        assert_eq!(result.user_id, 7);
        let log = format!("{:?}", db.into_transaction_log());
        assert!(log.contains("activity_event_history"));
        assert!(log.contains("COMMIT"));
        assert!(!log.contains("refresh_scope"));
        assert!(!log.contains("processing_pipeline"));
        assert!(!log.contains("segment_efforts"));
    }
}

#[tokio::test]
async fn retries_read_source_bytes_instead_of_returning_a_saved_execution_receipt() {
    let (objects, backend) = fixture::retain_source(fixtures::REPROCESS_SOURCE).await;
    let db = fixture::mutation(fixture::source_reads(fixture::mock()), Ok(1)).into_connection();
    reprocess(&db, &objects, fixtures::reprocess_execution()).await.unwrap();
    backend
        .put(
            &Path::parse(fixtures::reprocess_artifact().storage_path).unwrap(),
            b"corrupt".as_slice().into(),
        )
        .await
        .unwrap();
    let db = fixture::metadata(fixture::mock()).into_connection();
    assert_eq!(
        reprocess(&db, &objects, fixtures::reprocess_execution())
            .await
            .unwrap_err()
            .status
            .as_u16(),
        422
    );
}

#[tokio::test]
async fn missing_or_reassigned_activity_is_rejected_before_saving() {
    for current in [
        Vec::new(),
        vec![activities::Model {
            activity_import_id: Some(99),
            ..fixtures::reprocess_activity()
        }],
    ] {
        let missing = current.is_empty();
        let db = fixture::mock()
            .append_query_results([vec![fixtures::reprocess_import()]])
            .append_query_results([current])
            .into_connection();
        assert_eq!(
            fixture::apply(&db).await.unwrap_err().status.as_u16(),
            if missing { 404 } else { 409 }
        );
        let log = format!("{:?}", db.into_transaction_log());
        assert!(!log.contains("UPDATE "));
        assert!(log.contains("ROLLBACK"));
    }
}

#[tokio::test]
async fn audit_failure_rolls_back_source_save() {
    let db = fixture::mutation(
        fixture::mock(),
        Err(sea_orm::DbErr::Custom("fixture audit unavailable".into())),
    )
    .into_connection();
    assert_eq!(fixture::apply(&db).await.unwrap_err().status.as_u16(), 500);
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("ROLLBACK"));
    assert!(!log.contains("COMMIT"));
}

#[tokio::test]
async fn a_new_import_reuses_its_existing_activity_after_a_lost_completion() {
    let import = fixtures::reprocess_import();
    let activity = fixtures::reprocess_activity();
    let db = fixture::mock()
        .append_query_results([vec![import]])
        .append_query_results([vec![activity]])
        .append_query_results([vec![fixture::saved_activity()]])
        .append_exec_results([fixtures::written()])
        .append_query_results([Vec::<crate::entities::activity_recordings::Model>::new()])
        .append_exec_results([
            fixtures::written(),
            fixtures::written(),
            fixtures::written(),
            fixtures::written(),
        ])
        .append_exec_results((0..3).map(|_| fixtures::written()))
        .into_connection();
    let mut execution = fixtures::reprocess_execution();
    execution.command.target = ImportTarget::NewActivity;
    let saved = activities::Model::apply_import(
        &db,
        &fixture::objects(),
        &execution,
        &fixture::artifact(),
        &fixtures::parsed_reprocess_activity(),
        &crate::training_profile::TrainingProfile::default(),
    )
    .await
    .unwrap();
    assert_eq!(saved.id, 42);
    let log = format!("{:?}", db.into_transaction_log());
    assert!(!log.contains("INSERT INTO \\\"activities\\\""));
}

#[tokio::test]
async fn a_new_import_saves_the_activity_and_links_its_import() {
    let db = fixture::mock()
        .append_query_results([vec![fixtures::reprocess_import()]])
        .append_query_results([Vec::<activities::Model>::new()])
        .append_query_results([Vec::<activities::Model>::new()])
        .append_query_results([vec![fixture::saved_activity()]])
        .append_exec_results([fixtures::written()])
        .append_query_results([Vec::<crate::entities::activity_recordings::Model>::new()])
        .append_exec_results([
            fixtures::written(),
            fixtures::written(),
            fixtures::written(),
            fixtures::written(),
        ])
        .append_exec_results((0..3).map(|_| fixtures::written()))
        .into_connection();
    let mut execution = fixtures::reprocess_execution();
    execution.command.target = ImportTarget::NewActivity;
    let saved = activities::Model::apply_import(
        &db,
        &fixture::objects(),
        &execution,
        &fixture::artifact(),
        &fixtures::parsed_reprocess_activity(),
        &crate::training_profile::TrainingProfile::default(),
    )
    .await
    .unwrap();
    assert_eq!(saved.id, 42);
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("INSERT INTO"));
    assert!(log.contains("activity_id"));
    assert!(log.contains("COMMIT"));
}

#[tokio::test]
async fn missing_retained_source_is_terminal_before_mutation() {
    let (objects, _) = fixtures::object_storage(ObjectOwner::ActivityImports);
    let db = fixture::metadata(fixture::mock()).into_connection();
    assert_eq!(
        reprocess(&db, &objects, fixtures::reprocess_execution())
            .await
            .unwrap_err()
            .status
            .as_u16(),
        404
    );
    assert_eq!(db.into_transaction_log().len(), 3);
}

#[tokio::test]
async fn malformed_source_with_valid_integrity_is_terminal_before_mutation() {
    let bytes = b"malformed tcx";
    let (objects, _) = fixture::retain_source(bytes).await;
    let artifact = crate::entities::activity_import_artifacts::Model {
        size_bytes: bytes.len() as i64,
        checksum_sha256: crate::object_store::checksum(bytes),
        ..fixtures::reprocess_artifact()
    };
    let mut execution = fixtures::reprocess_execution();
    execution.command.source.object.bytes = bytes.len();
    execution
        .command
        .source
        .object
        .sha256
        .clone_from(&artifact.checksum_sha256);
    let db = fixture::source_metadata(fixture::mock(), artifact).into_connection();
    assert!(reprocess(&db, &objects, execution)
        .await
        .unwrap_err()
        .status
        .is_client_error());
    assert_eq!(db.into_transaction_log().len(), 3);
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
}

#[tokio::test]
async fn corrupt_supplemental_recording_is_rejected_before_saving() {
    let (objects, backend) = fixture::retain_source(fixtures::REPROCESS_SOURCE).await;
    let artifact = fixtures::reprocess_recording_artifact();
    backend
        .put(
            &Path::parse(&artifact.storage_path).unwrap(),
            b"corrupt".as_slice().into(),
        )
        .await
        .unwrap();
    let db = fixture::metadata(fixture::mock())
        .append_query_results([vec![fixtures::reprocess_artifact(), artifact]])
        .into_connection();
    assert_eq!(
        reprocess(&db, &objects, fixtures::reprocess_execution())
            .await
            .unwrap_err()
            .status
            .as_u16(),
        422
    );
    assert_eq!(db.into_transaction_log().len(), 4);
}

#[tokio::test]
async fn checksumless_legacy_files_remain_readable_without_replacing_the_source() {
    let (objects, _) = fixture::retain_source(fixtures::REPROCESS_SOURCE).await;
    let artifact = crate::activity_import_pipeline::ActivityProcessingArtifact {
        checksum_sha256: None,
        ..fixture::artifact()
    };
    assert_eq!(artifact.read(&objects).await.unwrap(), fixtures::REPROCESS_SOURCE);
    let changed = crate::activity_import_pipeline::ActivityProcessingArtifact {
        size_bytes: 1,
        ..artifact
    };
    assert_eq!(changed.read(&objects).await.unwrap_err().status, 422);
}

#[tokio::test]
async fn another_recording_of_the_same_ride_reuses_the_activity() {
    let parsed = fixtures::parsed_reprocess_activity();
    let duplicate = activities::Model {
        started_at: parsed.draft.started_at,
        sport: parsed.draft.sport.clone(),
        total_time_seconds: parsed.draft.total_time_seconds,
        moving_time_seconds: parsed.draft.moving_time_seconds,
        distance_meters: parsed.draft.distance_meters,
        derived_data_json: Some(crate::activity_data::serialize_derived_activity_data(
            &parsed.derived_data,
        )),
        ..fixtures::reprocess_activity()
    };
    let db = fixture::mock()
        .append_query_results([vec![fixtures::reprocess_import()]])
        .append_query_results([Vec::<activities::Model>::new()])
        .append_query_results([vec![duplicate.clone()], vec![duplicate.clone()]])
        .append_query_results([vec![fixtures::reprocess_import()]])
        .append_exec_results([fixtures::written()])
        .append_query_results([Vec::<crate::entities::activity_recordings::Model>::new()])
        .append_exec_results([fixtures::written()])
        .append_exec_results((0..3).map(|_| fixtures::written()))
        .into_connection();
    let mut execution = fixtures::reprocess_execution();
    execution.command.target = ImportTarget::NewActivity;
    let saved = activities::Model::apply_import(
        &db,
        &fixture::objects(),
        &execution,
        &fixture::artifact(),
        &parsed,
        &crate::training_profile::TrainingProfile::default(),
    )
    .await
    .unwrap();
    assert_eq!(saved.id, duplicate.id);
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("activity_recordings"));
    assert!(log.contains("duplicate"));
    assert!(log.contains("COMMIT"));
}

#[tokio::test]
async fn recording_publication_failure_rolls_back_before_import_completion_and_audit() {
    let db = fixture::mock()
        .append_query_results([vec![fixtures::reprocess_import()]])
        .append_query_results([vec![fixtures::reprocess_activity()]])
        .append_query_results([vec![fixture::saved_activity()]])
        .append_query_results([Vec::<crate::entities::activity_recordings::Model>::new()])
        .append_exec_results((0..3).map(|_| fixtures::written()))
        .append_exec_errors([sea_orm::DbErr::Custom("recording reference unavailable".into())])
        .into_connection();
    assert_eq!(fixture::apply(&db).await.unwrap_err().status.as_u16(), 500);
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("ROLLBACK"));
    assert!(!log.contains("activity_event_history"));
    assert!(!log.contains("analytics_user_states"));
}

#[tokio::test]
async fn duplicate_recordings_publish_merged_evidence_without_replacing_the_original_samples() {
    let parsed = fixtures::parsed_reprocess_activity();
    let existing = activities::Model {
        started_at: parsed.draft.started_at,
        sport: parsed.draft.sport.clone(),
        total_time_seconds: parsed.draft.total_time_seconds,
        moving_time_seconds: parsed.draft.moving_time_seconds,
        distance_meters: parsed.draft.distance_meters,
        derived_data_json: Some((&parsed.derived_data).into()),
        ..fixtures::reprocess_activity()
    };
    let mut enriched = parsed.clone();
    enriched.derived_data.recording.trainer("trainer", true);
    let db = fixture::mock()
        .append_query_results([vec![fixtures::reprocess_import()]])
        .append_query_results([Vec::<activities::Model>::new()])
        .append_query_results([vec![existing.clone()], vec![existing.clone()]])
        .append_query_results([vec![fixtures::reprocess_import()]])
        .append_exec_results([fixtures::written()])
        .append_query_results([Vec::<crate::entities::activity_recordings::Model>::new()])
        .append_exec_results([fixtures::written(), fixtures::written()])
        .append_exec_results((0..3).map(|_| fixtures::written()))
        .into_connection();
    let objects = fixture::objects();
    let mut execution = fixtures::reprocess_execution();
    execution.command.target = ImportTarget::NewActivity;
    let result = activities::Model::apply_import(
        &db,
        &objects,
        &execution,
        &fixture::artifact(),
        &enriched,
        &crate::training_profile::TrainingProfile::default(),
    )
    .await
    .unwrap();
    let data = crate::activity_data::ActivityDerivedData::from(result.derived_data_json.unwrap());
    assert_eq!(data.route_points, parsed.derived_data.route_points);
    assert!(data.recording.excluded());
    let expected = objects
        .transform(
            "bike/tasks/recordings/7",
            &crate::entities::activity_recordings::Recording::from(data),
        )
        .await
        .unwrap();
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains(&expected.sha256));
    assert!(log.contains("COMMIT"));
}

#[tokio::test]
async fn changed_duplicate_candidate_retries_before_recording_publication() {
    let parsed = fixtures::parsed_reprocess_activity();
    let candidate = activities::Model {
        started_at: parsed.draft.started_at,
        sport: parsed.draft.sport.clone(),
        total_time_seconds: parsed.draft.total_time_seconds,
        moving_time_seconds: parsed.draft.moving_time_seconds,
        distance_meters: parsed.draft.distance_meters,
        derived_data_json: Some((&parsed.derived_data).into()),
        ..fixtures::reprocess_activity()
    };
    let changed = activities::Model {
        updated_at: candidate.updated_at + chrono::Duration::seconds(1),
        ..candidate.clone()
    };
    let db = fixture::mock()
        .append_query_results([vec![fixtures::reprocess_import()]])
        .append_query_results([Vec::<activities::Model>::new()])
        .append_query_results([vec![candidate], vec![changed]])
        .into_connection();
    let mut execution = fixtures::reprocess_execution();
    execution.command.target = ImportTarget::NewActivity;
    let error = activities::Model::apply_import(
        &db,
        &fixture::objects(),
        &execution,
        &fixture::artifact(),
        &parsed,
        &crate::training_profile::TrainingProfile::default(),
    )
    .await
    .unwrap_err();
    assert_eq!(error.status.as_u16(), 500);
    let log = format!("{:?}", db.into_transaction_log());
    assert!(!log.contains("activity_recordings"));
    assert!(!log.contains("SET "));
    assert!(log.contains("ROLLBACK"));
}
