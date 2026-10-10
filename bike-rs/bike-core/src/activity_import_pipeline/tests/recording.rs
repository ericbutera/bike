use super::*;
use crate::activity_data::{deserialize_derived_activity_data, serialize_derived_activity_data};
use crate::activity_recording::{RecordingContext, RecordingEnvironment};
use crate::activity_source_recovery::{
    apply_recovery, plan_recovery, retire_generated_artifact, RecoveryOutcome,
};
use sea_orm::IntoActiveModel;

fn recording_upload(virtual_ride: bool) -> ActivityUploadPayload {
    ActivityUploadPayload {
        original_filename: "synthetic.fit".into(),
        format: "fit".into(),
        mime_type: None,
        source_correlation_id: None,
        bytes: if virtual_ride {
            include_bytes!("../../../tests/fixtures/recording/virtual-generic.fit").to_vec()
        } else {
            include_bytes!("../../../tests/fixtures/recording/outdoor.fit").to_vec()
        },
    }
}

async fn import_recording(
    db: &DatabaseConnection,
    dir: &str,
    virtual_ride: bool,
) -> PersistedActivityImport {
    let imported = match persist_test_activity_upload(
        db,
        dir,
        recording_upload(virtual_ride),
        "archive_url_import",
        &TrainingProfile::default(),
    )
    .await
    .unwrap()
    {
        PersistActivityUploadOutcome::Imported(imported) => imported,
        PersistActivityUploadOutcome::Duplicate(_) => panic!("expected import"),
    };
    mark_activity_imports_processed(db, &[imported.import.id])
        .await
        .unwrap();
    imported
}

#[tokio::test]
async fn retired_copies_cannot_enter_manual_or_archive_activity_processing() {
    for source in ["manual_upload", "archive_url_import"] {
        let db = test_db().await;
        let dir = test_uploads_dir();
        let error = persist_test_activity_upload(
            &db,
            &dir,
            tcx_upload(true),
            source,
            &TrainingProfile::default(),
        )
        .await
        .err()
        .expect("retired generated input must fail");
        assert!(error.to_string().contains("retired"));
        assert_eq!(activities::Entity::find().count(&db).await.unwrap(), 0);
        assert_eq!(
            background_tasks::Entity::find().count(&db).await.unwrap(),
            0
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[tokio::test]
async fn authentic_tcx_from_manual_and_archive_inputs_keeps_real_heatmap_geometry() {
    for source in ["manual_upload", "archive_url_import"] {
        let db = test_db().await;
        let dir = test_uploads_dir();
        let result = persist_test_activity_upload(
            &db,
            &dir,
            tcx_upload(false),
            source,
            &TrainingProfile::default(),
        )
        .await
        .unwrap();
        let PersistActivityUploadOutcome::Imported(imported) = result else {
            panic!("expected an authentic activity import")
        };
        let derived =
            deserialize_derived_activity_data(imported.activity.derived_data_json.as_ref());
        assert_eq!(derived.route_points.len(), 3);
        assert!(!crate::heatmaps::preparation::prepare_route(&derived).is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

fn tcx_upload(retired: bool) -> ActivityUploadPayload {
    ActivityUploadPayload {
        original_filename: "synthetic.tcx".into(),
        format: "tcx".into(),
        mime_type: None,
        source_correlation_id: None,
        bytes: if retired {
            include_bytes!("../../../tests/fixtures/recording/retired-copy.tcx").to_vec()
        } else {
            include_bytes!("../../../tests/fixtures/recording/outdoor.tcx").to_vec()
        },
    }
}

async fn legacy_generated_copy(
    db: &DatabaseConnection,
    dir: &str,
    original: &activities::Model,
) -> activity_imports::Model {
    let import = store_activity_upload_import_with_artifacts(
        db,
        StoreActivityUploadImportRequest {
            uploads_dir: dir,
            user_storage_key: "test-user",
            user_id: original.user_id,
            upload: ActivityUploadPayload {
                original_filename: "generated.tcx".into(),
                format: "tcx".into(),
                mime_type: None,
                source_correlation_id: Some("1111".into()),
                bytes: b"obsolete generated content".to_vec(),
            },
            primary_artifact_kind: ACTIVITY_IMPORT_ARTIFACT_KIND_GENERATED_EXPORT,
            primary_source_quality: ACTIVITY_IMPORT_SOURCE_QUALITY_GENERATED_TCX,
            additional_artifacts: Vec::new(),
            source: "strava_sync",
        },
    )
    .await
    .unwrap();
    let mut derived = deserialize_derived_activity_data(original.derived_data_json.as_ref());
    derived.recording = RecordingContext::default();
    for point in &mut derived.route_points {
        point.elapsed_seconds -= 2;
    }
    let mut copy = original.clone().into_active_model();
    copy.id = sea_orm::ActiveValue::NotSet;
    copy.source = Set("strava_sync".into());
    copy.source_correlation_id = Set(Some("1111".into()));
    copy.title = Set("Rider title".into());
    copy.activity_type = Set("race".into());
    copy.started_at = Set(original.started_at + ChronoDuration::seconds(2));
    copy.activity_import_id = Set(Some(import.id));
    copy.derived_data_json = Set(Some(serialize_derived_activity_data(&derived)));
    let activity = copy.insert(db).await.unwrap();
    let mut import = import.into_active_model();
    import.activity_id = Set(Some(activity.id));
    import.update(db).await.unwrap()
}

#[tokio::test]
async fn archive_backfill_replaces_generated_source_for_virtual_and_real_rides() {
    for virtual_ride in [true, false] {
        let db = test_db().await;
        let dir = test_uploads_dir();
        assert_archive_recovery(&db, &dir, virtual_ride).await;
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[tokio::test]
#[ignore = "requires BIKE_TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn archive_recovery_on_postgres_retains_sources_and_queues_replay_atomically() {
    for virtual_ride in [true, false] {
        let url = std::env::var("BIKE_TEST_DATABASE_URL").expect("disposable PostgreSQL URL");
        let db = Database::connect(
            sea_orm::ConnectOptions::new(url)
                .max_connections(1)
                .to_owned(),
        )
        .await
        .unwrap();
        let schema = format!("source_recovery_{}", Uuid::new_v4().simple());
        db.execute_unprepared(&format!(
            "CREATE SCHEMA {schema}; SET search_path TO {schema}"
        ))
        .await
        .unwrap();
        let db = create_test_tables(db).await;
        let dir = test_uploads_dir();
        assert_archive_recovery(&db, &dir, virtual_ride).await;
        std::fs::remove_dir_all(dir).unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
            .await
            .unwrap();
    }
}

async fn assert_archive_recovery(db: &DatabaseConnection, dir: &str, virtual_ride: bool) {
    let original = import_recording(db, dir, virtual_ride).await;
    let import = legacy_generated_copy(db, dir, &original.activity).await;
    let generated = generated_source(db, &import).await;
    assert!(!retire_generated_artifact(db, dir, generated.clone(), true)
        .await
        .unwrap());
    assert_legacy_source_rejected(db, &import).await;
    let plan = plan_recovery(db, dir, import.clone(), Utc::now())
        .await
        .unwrap();
    assert_eq!(plan.outcome, RecoveryOutcome::ArchiveOriginal);
    assert!(apply_recovery(db, dir, plan).await.unwrap());
    assert!(!retire_generated_artifact(db, dir, generated.clone(), true)
        .await
        .unwrap());
    assert_replay_job(db, import.activity_id.unwrap()).await;
    let promoted =
        assert_retained_original(db, dir, import.id, &original.import, virtual_ride).await;
    assert_recovered_activity(db, dir, &import, promoted, virtual_ride).await;
    assert_generated_cleanup(db, dir, generated).await;
    assert!(Path::new(dir).join(original.import.storage_path).exists());
}

async fn assert_retained_original(
    db: &DatabaseConnection,
    dir: &str,
    import_id: i32,
    original: &activity_imports::Model,
    virtual_ride: bool,
) -> activity_imports::Model {
    let promoted = activity_imports::Entity::find_by_id(import_id)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(promoted.format, "fit");
    assert_ne!(promoted.storage_path, original.storage_path);
    let bytes = tokio::fs::read(Path::new(dir).join(&promoted.storage_path))
        .await
        .unwrap();
    assert_eq!(bytes, recording_upload(virtual_ride).bytes);
    promoted
}

async fn assert_legacy_source_rejected(db: &DatabaseConnection, import: &activity_imports::Model) {
    assert!(load_best_activity_parsing_artifact(db, import)
        .await
        .is_err());
    let stored = activity_imports::Entity::find_owned(db, import.user_id, import.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.format, "tcx");
}

async fn generated_source(
    db: &DatabaseConnection,
    import: &activity_imports::Model,
) -> activity_import_artifacts::Model {
    activity_import_artifacts::Entity::for_import(db, import.user_id, import.id)
        .await
        .unwrap()
        .into_iter()
        .find(|a| a.artifact_kind == "generated_export")
        .unwrap()
}

async fn assert_generated_cleanup(
    db: &DatabaseConnection,
    dir: &str,
    generated: activity_import_artifacts::Model,
) {
    assert!(retire_generated_artifact(db, dir, generated.clone(), false)
        .await
        .unwrap());
    assert!(Path::new(dir).join(&generated.storage_path).exists());
    assert!(retire_generated_artifact(db, dir, generated.clone(), true)
        .await
        .unwrap());
    assert!(!Path::new(dir).join(&generated.storage_path).exists());
    assert!(activity_import_artifacts::Entity::find_by_id(generated.id)
        .one(db)
        .await
        .unwrap()
        .is_none());
}

async fn assert_replay_job(db: &DatabaseConnection, activity_id: i32) {
    let jobs = background_tasks::Entity::find()
        .filter(background_tasks::Column::TaskType.eq("reprocess_activity_import"))
        .all(db)
        .await
        .unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].payload["data"]["activity_id"], activity_id);
    assert_eq!(jobs[0].status, "pending");
}

async fn assert_recovered_activity(
    db: &DatabaseConnection,
    dir: &str,
    import: &activity_imports::Model,
    promoted: activity_imports::Model,
    virtual_ride: bool,
) {
    let activity = activities::Model::find_owned(db, import.activity_id.unwrap(), 1)
        .await
        .unwrap()
        .unwrap();
    let replayed = reprocess_activity_from_import(
        db,
        dir,
        1,
        activity,
        promoted.clone(),
        Some(&TrainingProfile::default()),
    )
    .await
    .unwrap();
    assert_eq!(replayed.activity.title, "Rider title");
    mark_activity_imports_processed(db, &[import.id])
        .await
        .unwrap();
    assert_eq!(replayed.activity.activity_type, "race");
    assert_eq!(
        replayed.activity.source_correlation_id.as_deref(),
        Some("1111")
    );
    assert_eq!(
        replayed.activity.recording_context().excluded(),
        virtual_ride
    );
    let derived = deserialize_derived_activity_data(replayed.activity.derived_data_json.as_ref());
    assert_eq!(
        crate::heatmaps::preparation::prepare_route(&derived).is_empty(),
        virtual_ride
    );
    let rerun = plan_recovery(db, dir, promoted, Utc::now()).await.unwrap();
    assert_eq!(rerun.outcome, RecoveryOutcome::NativeSource);
    assert!(!apply_recovery(db, dir, rerun).await.unwrap());
}

#[tokio::test]
async fn archive_recovery_rejects_other_owners_corrupt_sources_and_stale_plans() {
    let db = test_db().await;
    let dir = test_uploads_dir();
    let original = import_recording(&db, &dir, true).await;
    let import = legacy_generated_copy(&db, &dir, &original.activity).await;
    let plan = plan_recovery(&db, &dir, import.clone(), Utc::now())
        .await
        .unwrap();
    let mut changed = import.clone().into_active_model();
    changed.original_filename = Set("changed.tcx".into());
    let changed = changed.update(&db).await.unwrap();
    assert!(apply_recovery(&db, &dir, plan).await.is_err());
    assert_eq!(
        background_tasks::Entity::find()
            .filter(background_tasks::Column::TaskType.eq("reprocess_activity_import"))
            .count(&db)
            .await
            .unwrap(),
        0
    );
    let mut other_owner = original.activity.clone().into_active_model();
    other_owner.user_id = Set(2);
    other_owner.update(&db).await.unwrap();
    let gap = plan_recovery(&db, &dir, changed.clone(), Utc::now())
        .await
        .unwrap();
    assert_eq!(gap.outcome, RecoveryOutcome::ArchiveRequired);
    let mut owned = original.activity.clone().into_active_model();
    owned.user_id = Set(1);
    owned.update(&db).await.unwrap();
    tokio::fs::write(
        Path::new(&dir).join(&original.import.storage_path),
        b"corrupt FIT",
    )
    .await
    .unwrap();
    assert!(plan_recovery(&db, &dir, changed, Utc::now()).await.is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn archive_recovery_uses_retained_duplicate_import_without_second_activity() {
    let db = test_db().await;
    let dir = test_uploads_dir();
    let original = import_recording(&db, &dir, true).await;
    let import = legacy_generated_copy(&db, &dir, &original.activity).await;
    let mut archive = original.import.into_active_model();
    archive.activity_id = Set(import.activity_id);
    archive.update(&db).await.unwrap();
    activities::Entity::delete_by_id(original.activity.id)
        .exec(&db)
        .await
        .unwrap();
    let plan = plan_recovery(&db, &dir, import, Utc::now()).await.unwrap();
    assert_eq!(plan.outcome, RecoveryOutcome::ArchiveOriginal);
    assert!(apply_recovery(&db, &dir, plan).await.unwrap());
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn recovery_promotes_retained_json_without_fetching_and_checks_provider_identity() {
    let db = test_db().await;
    let dir = test_uploads_dir();
    let original = import_recording(&db, &dir, false).await;
    let import = legacy_generated_copy(&db, &dir, &original.activity).await;
    activities::Entity::delete_by_id(original.activity.id)
        .exec(&db)
        .await
        .unwrap();
    let payload: crate::strava_provider_payload::StoredStravaProviderPayload =
        serde_json::from_slice(include_bytes!("../../../testdata/strava-real-ride.json")).unwrap();
    let upload = crate::strava::build_activity_upload(&payload.activity, &payload.streams).unwrap();
    let path = format!("retained_{}.json", Uuid::new_v4());
    tokio::fs::write(Path::new(&dir).join(&path), &upload.bytes)
        .await
        .unwrap();
    activity_import_artifacts::Entity::store_gateway_payload(&db, &import, path, &upload)
        .await
        .unwrap();
    assert!(plan_recovery(&db, &dir, import.clone(), Utc::now())
        .await
        .is_err());
    let activity = activities::Model::find_owned(&db, import.activity_id.unwrap(), 1)
        .await
        .unwrap()
        .unwrap();
    let mut activity = activity.into_active_model();
    activity.source_correlation_id = Set(Some(format!("strava:{}", payload.activity.id)));
    activity.update(&db).await.unwrap();
    let plan = plan_recovery(&db, &dir, import.clone(), Utc::now())
        .await
        .unwrap();
    assert_eq!(plan.outcome, RecoveryOutcome::RetainedProvider);
    assert!(apply_recovery(&db, &dir, plan).await.unwrap());
    let promoted = activity_imports::Entity::find_by_id(import.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(promoted.format, "json");
    let plan = plan_recovery(&db, &dir, promoted, Utc::now())
        .await
        .unwrap();
    assert_eq!(plan.outcome, RecoveryOutcome::NativeSource);
    assert!(!apply_recovery(&db, &dir, plan).await.unwrap());
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn recovery_refuses_ambiguous_archive_originals() {
    let db = test_db().await;
    let dir = test_uploads_dir();
    let original = import_recording(&db, &dir, true).await;
    let import = legacy_generated_copy(&db, &dir, &original.activity).await;
    let other = store_activity_upload_import_with_artifacts(
        &db,
        StoreActivityUploadImportRequest {
            uploads_dir: &dir,
            user_storage_key: "test-user",
            user_id: 1,
            upload: recording_upload(false),
            primary_artifact_kind: ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL,
            primary_source_quality: "fit_original",
            additional_artifacts: Vec::new(),
            source: "archive_url_import",
        },
    )
    .await
    .unwrap();
    let mut activity = original.activity.into_active_model();
    activity.id = sea_orm::ActiveValue::NotSet;
    activity.activity_import_id = Set(Some(other.id));
    activity.insert(&db).await.unwrap();
    let plan = plan_recovery(&db, &dir, import, Utc::now()).await.unwrap();
    assert_eq!(plan.outcome, RecoveryOutcome::AmbiguousArchive);
    assert!(!apply_recovery(&db, &dir, plan).await.unwrap());
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn recovery_retains_noncycling_provider_input_without_ride_jobs_or_data_loss() {
    let db = test_db().await;
    let dir = test_uploads_dir();
    let original = import_recording(&db, &dir, false).await;
    let import = legacy_generated_copy(&db, &dir, &original.activity).await;
    activities::Entity::delete_by_id(original.activity.id)
        .exec(&db)
        .await
        .unwrap();
    let mut payload: crate::strava_provider_payload::StoredStravaProviderPayload =
        serde_json::from_slice(include_bytes!("../../../testdata/strava-real-ride.json")).unwrap();
    payload.activity.sport_type = Some("Run".into());
    let upload = crate::strava::build_activity_upload(&payload.activity, &payload.streams).unwrap();
    let path = format!("retained_{}.json", Uuid::new_v4());
    tokio::fs::write(Path::new(&dir).join(&path), &upload.bytes)
        .await
        .unwrap();
    activity_import_artifacts::Entity::store_gateway_payload(&db, &import, path, &upload)
        .await
        .unwrap();
    let original = activities::Model::find_owned(&db, import.activity_id.unwrap(), 1)
        .await
        .unwrap()
        .unwrap();
    let mut activity = original.clone().into_active_model();
    activity.sport = Set("run".into());
    activity.source_correlation_id = Set(Some(payload.activity.id.to_string()));
    activity.update(&db).await.unwrap();
    let plan = plan_recovery(&db, &dir, import.clone(), Utc::now())
        .await
        .unwrap();
    assert!(!plan.reprocessing_required);
    assert!(apply_recovery(&db, &dir, plan).await.unwrap());
    assert_eq!(
        background_tasks::Entity::find()
            .filter(background_tasks::Column::TaskType.eq("reprocess_activity_import"))
            .count(&db)
            .await
            .unwrap(),
        0
    );
    let retained = activities::Model::find_owned(&db, import.activity_id.unwrap(), 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.derived_data_json, original.derived_data_json);
    assert_eq!(retained.title, original.title);
    assert_eq!(retained.sport, "run");
    assert_eq!(retained.format.as_deref(), Some("json"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn archived_source_registration_checks_identity_bytes_and_idempotency() {
    use crate::activity_source_recovery::{register_archive_source, ArchivedActivitySource};
    let db = test_db().await;
    let dir = test_uploads_dir();
    let original = import_recording(&db, &dir, false).await;
    let import = legacy_generated_copy(&db, &dir, &original.activity).await;
    let checksum = hex::encode(Sha256::digest(&recording_upload(false).bytes));
    let source = || ArchivedActivitySource {
        recording_label: None,
        import_id: import.id,
        provider_activity_id: 1111,
        original_filename: "original.fit".into(),
        format: "fit".into(),
        storage_path: original.import.storage_path.clone(),
        checksum_sha256: checksum.clone(),
    };
    let mut wrong = source();
    wrong.provider_activity_id = 9999;
    assert!(register_archive_source(&db, &dir, 1, wrong, true)
        .await
        .is_err());
    let mut corrupt = source();
    corrupt.checksum_sha256 = "0".repeat(64);
    assert!(register_archive_source(&db, &dir, 1, corrupt, true)
        .await
        .is_err());
    assert!(register_archive_source(&db, &dir, 2, source(), true)
        .await
        .is_err());
    register_archive_source(&db, &dir, 1, source(), false)
        .await
        .unwrap();
    assert_eq!(
        activity_import_artifacts::Entity::for_import(&db, 1, import.id)
            .await
            .unwrap()
            .len(),
        1
    );
    for _ in 0..2 {
        register_archive_source(&db, &dir, 1, source(), true)
            .await
            .unwrap();
    }
    assert_eq!(
        activity_import_artifacts::Entity::for_import(&db, 1, import.id)
            .await
            .unwrap()
            .len(),
        2
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn unavailable_sources_keep_activity_data_and_require_owner_scoped_current_plans() {
    use crate::activity_source_recovery::withhold_unrecovered_source;
    let db = test_db().await;
    let dir = test_uploads_dir();
    let original = import_recording(&db, &dir, false).await;
    let import = legacy_generated_copy(&db, &dir, &original.activity).await;
    activities::Entity::delete_by_id(original.activity.id)
        .exec(&db)
        .await
        .unwrap();
    let before = activities::Model::find_owned(&db, import.activity_id.unwrap(), 1)
        .await
        .unwrap()
        .unwrap();
    seed_ready_heatmap(&db, before.id).await;
    let plan = plan_recovery(&db, &dir, import.clone(), Utc::now())
        .await
        .unwrap();
    assert_eq!(plan.outcome, RecoveryOutcome::ArchiveRequired);
    assert!(withhold_unrecovered_source(&db, &plan).await.unwrap());
    assert!(withhold_unrecovered_source(&db, &plan).await.is_err());
    let after = activities::Model::find_owned(&db, before.id, 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(before.derived_data_json, after.derived_data_json);
    assert_eq!(before.title, after.title);
    assert_eq!(after.format.as_deref(), Some("unavailable"));
    use crate::entities::{heatmap_chunks, heatmap_projections, heatmap_user_states};
    let projection = heatmap_projections::Entity::find_by_id(before.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(projection.status, "pending");
    assert_eq!(projection.generation, 2);
    assert_eq!(heatmap_chunks::Entity::find().count(&db).await.unwrap(), 0);
    assert_eq!(
        heatmap_user_states::Entity::find_by_id(1)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .revision,
        2
    );
    assert!(Path::new(&dir).join(import.storage_path).exists());
    std::fs::remove_dir_all(dir).unwrap();
}

async fn seed_ready_heatmap(db: &DatabaseConnection, activity_id: i32) {
    use crate::entities::{heatmap_chunks, heatmap_projections, heatmap_user_states};
    let schema = Schema::new(db.get_database_backend());
    db.execute(&schema.create_table_from_entity(heatmap_projections::Entity))
        .await
        .unwrap();
    db.execute(&schema.create_table_from_entity(heatmap_chunks::Entity))
        .await
        .unwrap();
    db.execute(&schema.create_table_from_entity(heatmap_user_states::Entity))
        .await
        .unwrap();
    heatmap_projections::ActiveModel {
        published_at: Set(None),
        activity_id: Set(activity_id),
        user_id: Set(1),
        generation: Set(1),
        status: Set("ready".into()),
        projection_version: Set(crate::heatmaps::PROJECTION_VERSION),
        queued_at: Set(None),
        error: Set(None),
        min_x: Set(Some(0.1)),
        min_y: Set(Some(0.1)),
        max_x: Set(Some(0.2)),
        max_y: Set(Some(0.2)),
    }
    .insert(db)
    .await
    .unwrap();
    heatmap_chunks::ActiveModel {
        activity_id: Set(activity_id),
        band: Set(0),
        chunk_index: Set(0),
        min_x: Set(0.1),
        min_y: Set(0.1),
        max_x: Set(0.2),
        max_y: Set(0.2),
        points: Set(vec![0; 16]),
    }
    .insert(db)
    .await
    .unwrap();
    heatmap_user_states::ActiveModel {
        user_id: Set(1),
        revision: Set(1),
    }
    .insert(db)
    .await
    .unwrap();
}

#[tokio::test]
async fn retained_source_refresh_recovers_recording_evidence_without_changing_gps() {
    use crate::activity_source_recovery::refresh_retained_recording;
    for virtual_ride in [false, true] {
        let db = test_db().await;
        let dir = test_uploads_dir();
        let original = import_recording(&db, &dir, virtual_ride).await;
        let mut derived =
            deserialize_derived_activity_data(original.activity.derived_data_json.as_ref());
        derived.recording = Default::default();
        activities::Model::store_recording(
            &db,
            original.activity.id,
            1,
            original.activity.updated_at,
            &derived,
        )
        .await
        .unwrap();
        let activity = activities::Model::find_owned(&db, original.activity.id, 1)
            .await
            .unwrap()
            .unwrap();
        refresh_retained_recording(&db, &dir, activity, true)
            .await
            .unwrap();
        let activity = activities::Model::find_owned(&db, original.activity.id, 1)
            .await
            .unwrap()
            .unwrap();
        let after = deserialize_derived_activity_data(activity.derived_data_json.as_ref());
        assert_eq!(derived.route_points, after.route_points);
        assert_eq!(after.recording.excluded(), virtual_ride);
        assert_eq!(
            crate::heatmaps::preparation::prepare_route(&after).is_empty(),
            virtual_ride
        );
        assert!(!refresh_retained_recording(&db, &dir, activity, true)
            .await
            .unwrap());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[tokio::test]
async fn authentic_phone_archive_replays_original_json_and_keeps_virtual_archive_evidence() {
    use crate::activity_source_recovery::{register_archive_source, ArchivedActivitySource};
    for virtual_ride in [false, true] {
        let db = test_db().await;
        let dir = test_uploads_dir();
        let original = import_recording(&db, &dir, false).await;
        let import = legacy_generated_copy(&db, &dir, &original.activity).await;
        let bytes = synthetic_phone_bytes(&original.activity);
        activities::Entity::delete_by_id(original.activity.id)
            .exec(&db)
            .await
            .unwrap();
        let path = format!("phone_{}.json", Uuid::new_v4());
        tokio::fs::write(Path::new(&dir).join(&path), &bytes)
            .await
            .unwrap();
        register_archive_source(
            &db,
            &dir,
            1,
            ArchivedActivitySource {
                import_id: import.id,
                provider_activity_id: 1111,
                format: "json".into(),
                original_filename: "phone.json".into(),
                storage_path: path.clone(),
                checksum_sha256: hex::encode(Sha256::digest(&bytes)),
                recording_label: Some(if virtual_ride { "Virtual Ride" } else { "Ride" }.into()),
            },
            true,
        )
        .await
        .unwrap();
        let plan = plan_recovery(&db, &dir, import.clone(), Utc::now())
            .await
            .unwrap();
        assert!(apply_recovery(&db, &dir, plan).await.unwrap());
        let promoted = activity_imports::Entity::find_owned(&db, 1, import.id)
            .await
            .unwrap()
            .unwrap();
        assert_recovered_activity(&db, &dir, &import, promoted, virtual_ride).await;
        assert_eq!(
            tokio::fs::read(Path::new(&dir).join(path)).await.unwrap(),
            bytes
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}

fn synthetic_phone_bytes(activity: &activities::Model) -> Vec<u8> {
    let derived = deserialize_derived_activity_data(activity.derived_data_json.as_ref());
    serde_json::to_vec(&serde_json::json!({
        "metadata":{"activity_name":"Synthetic phone ride", "activity_type":"Ride", "start_date":activity.started_at.to_rfc3339(), "elapsed_time":activity.total_time_seconds, "timer_time":activity.moving_time_seconds},
        "data":[{"fields":["time","latlng"], "values":derived.route_points.iter().map(|point| serde_json::json!([activity.started_at.timestamp()+i64::from(point.elapsed_seconds),[point.latitude,point.longitude]])).collect::<Vec<_>>() }]
    })).unwrap()
}

#[tokio::test]
async fn virtual_and_real_imports_preserve_context_through_all_stages_and_reprocessing() {
    for virtual_ride in [true, false] {
        let db = test_db().await;
        let dir = test_uploads_dir();
        let imported = import_recording(&db, &dir, virtual_ride).await;
        assert_eq!(
            imported.import.processing_stage,
            ACTIVITY_IMPORT_STAGE_TRAINING_ANALYSIS_BUILT
        );
        let original =
            deserialize_derived_activity_data(imported.activity.derived_data_json.as_ref());
        assert_eq!(original.recording.excluded(), virtual_ride);
        assert_eq!(original.route_points.len(), 16);
        reprocess_activity_from_import(
            &db,
            &dir,
            1,
            imported.activity.clone(),
            imported.import.clone(),
            Some(&TrainingProfile::default()),
        )
        .await
        .unwrap();
        let replayed = activities::Entity::find_by_id(imported.activity.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            deserialize_derived_activity_data(replayed.derived_data_json.as_ref()).recording,
            original.recording
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[tokio::test]
async fn a_generic_replay_cannot_erase_persisted_virtual_evidence() {
    let db = test_db().await;
    let dir = test_uploads_dir();
    let imported = import_recording(&db, &dir, false).await;
    let mut derived =
        deserialize_derived_activity_data(imported.activity.derived_data_json.as_ref());
    derived.recording.observe("verified_recorder", "Zwift");
    activities::Model::store_recording(
        &db,
        imported.activity.id,
        1,
        imported.activity.updated_at,
        &derived,
    )
    .await
    .unwrap();
    let activity = activities::Entity::find_by_id(imported.activity.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    reprocess_activity_from_import(
        &db,
        &dir,
        1,
        activity,
        imported.import,
        Some(&TrainingProfile::default()),
    )
    .await
    .unwrap();
    let replayed = activities::Entity::find_by_id(imported.activity.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        replayed.recording_context().environment,
        RecordingEnvironment::Virtual
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn matching_copies_inherit_virtual_evidence_in_either_import_order() {
    for original_first in [true, false] {
        let db = test_db().await;
        let dir = test_uploads_dir();
        let first = import_recording(&db, &dir, original_first).await;
        // Distinct provider identity/start time, identical absolute GPS samples.
        let mut changed = first.activity.clone().into_active_model();
        changed.started_at = Set(first.activity.started_at - ChronoDuration::seconds(2));
        let mut derived =
            deserialize_derived_activity_data(first.activity.derived_data_json.as_ref());
        for point in &mut derived.route_points {
            point.elapsed_seconds += 2;
        }
        changed.derived_data_json = Set(Some(serialize_derived_activity_data(&derived)));
        changed.update(&db).await.unwrap();
        let second = import_recording(&db, &dir, !original_first).await;
        for id in [first.activity.id, second.activity.id] {
            let activity = activities::Entity::find_by_id(id)
                .one(&db)
                .await
                .unwrap()
                .unwrap();
            assert!(activity.recording_context().excluded());
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[tokio::test]
async fn another_users_matching_virtual_route_cannot_hide_a_real_activity() {
    let db = test_db().await;
    let dir = test_uploads_dir();
    let virtual_activity = import_recording(&db, &dir, true).await;
    let mut other = virtual_activity.activity.into_active_model();
    other.user_id = Set(2);
    other.update(&db).await.unwrap();
    let real = import_recording(&db, &dir, false).await;
    assert!(!real.activity.recording_context().excluded());
    assert!(!crate::heatmaps::geometry::prepare(
        &deserialize_derived_activity_data(real.activity.derived_data_json.as_ref()).route_points
    )
    .is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn duplicate_evidence_promotes_existing_ride_without_replacing_its_route() {
    let db = test_db().await;
    let dir = test_uploads_dir();
    let imported = import_recording(&db, &dir, false).await;
    let before = deserialize_derived_activity_data(imported.activity.derived_data_json.as_ref());
    let mut context = RecordingContext::default();
    context.observe("strava.type", "VirtualRide");
    let merged = merge_duplicate_recording(&db, imported.activity, &context)
        .await
        .unwrap();
    let after = deserialize_derived_activity_data(merged.derived_data_json.as_ref());
    assert!(after.recording.excluded());
    assert_eq!(before.route_points, after.route_points);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn every_ingestion_node_keeps_virtual_and_real_heatmap_behavior() {
    for virtual_ride in [true, false] {
        let db = test_db().await;
        let dir = test_uploads_dir();
        let import = store_activity_upload_import_with_artifacts(
            &db,
            StoreActivityUploadImportRequest {
                uploads_dir: &dir,
                user_storage_key: "test-user",
                user_id: 1,
                upload: recording_upload(virtual_ride),
                source: "manual_upload",
                primary_artifact_kind: ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL,
                primary_source_quality: ACTIVITY_IMPORT_SOURCE_QUALITY_FIT_ORIGINAL,
                additional_artifacts: Vec::new(),
            },
        )
        .await
        .unwrap();
        let profile = TrainingProfile::default();
        let run = ActivityProcessingRun {
            db: &db,
            uploads_dir: &dir,
            user_id: 1,
            import,
            existing_activity: None,
            source_correlation_id: None,
            deduplication: ActivityUploadDeduplication::Disabled,
            training_profile: Some(&profile),
            cache_refresh: ReprocessCacheRefresh::Immediate,
        };
        let attempt = begin_processing_attempt(&run).await.unwrap();
        let mut state = load_activity_processing_state(&run, attempt).await.unwrap();
        for node in activity_processing_topological_order().unwrap() {
            let metadata = *activity_processing_graph_nodes()
                .iter()
                .find(|entry| entry.node == node)
                .unwrap();
            assert!(run_activity_processing_node(&run, &mut state, metadata)
                .await
                .unwrap()
                .is_none());
            if node == ActivityProcessingNode::RawStored {
                let parsed =
                    crate::activity_parser::parse_activity_data("raw.fit", "fit", &state.bytes)
                        .unwrap();
                assert_heatmap_recording(&parsed.derived_data, virtual_ride, node);
            }
            if let Some(parsed) = &state.parsed_activity {
                assert_heatmap_recording(&parsed.derived_data, virtual_ride, node);
            }
            if let Some(activity) = &state.activity_model {
                let stored = activities::Model::find_owned(&db, activity.id, 1)
                    .await
                    .unwrap()
                    .unwrap();
                let data = deserialize_derived_activity_data(stored.derived_data_json.as_ref());
                assert_heatmap_recording(&data, virtual_ride, node);
            }
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}

fn provider_artifact(trainer: bool) -> ActivityImportArtifactPayload {
    ActivityImportArtifactPayload {
        artifact_kind: ACTIVITY_IMPORT_ARTIFACT_KIND_PROVIDER_PAYLOAD.into(), format: "json".into(),
        source_quality: ACTIVITY_IMPORT_SOURCE_QUALITY_STRAVA_STREAMS.into(), original_filename: "provider.json".into(),
        mime_type: Some("application/json".into()),
        bytes: serde_json::to_vec(&serde_json::json!({
            "v":1,"provider":"strava","provider_activity_id":99,
            "activity":{"id":99,"name":"Synthetic ride","sport_type":"Ride","type":"Ride","trainer":trainer,"start_date":"2020-01-01T00:00:00Z"},
            "streams":{"time":{"data":[0,5]},"latlng":{"data":[[45,-120],[45,-119.9998]]}}
        })).unwrap(),
    }
}

#[tokio::test]
async fn authoritative_fit_geometry_does_not_discard_indoor_flags_from_provider_artifact() {
    for (virtual_recorder, trainer, excluded) in [
        (false, true, true),
        (true, false, true),
        (false, false, false),
    ] {
        let db = test_db().await;
        let dir = test_uploads_dir();
        let outcome = persist_activity_upload_with_artifacts(
            &db,
            PersistActivityUploadWithArtifactsRequest {
                uploads_dir: &dir,
                user_storage_key: "test-user",
                user_id: 1,
                upload: recording_upload(virtual_recorder),
                primary_artifact_kind: ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL,
                primary_source_quality: ACTIVITY_IMPORT_SOURCE_QUALITY_FIT_ORIGINAL,
                additional_artifacts: vec![provider_artifact(trainer)],
                source: "strava_sync",
                deduplication: ActivityUploadDeduplication::Enabled,
                training_profile: Some(&TrainingProfile::default()),
            },
        )
        .await
        .unwrap();
        let PersistActivityUploadOutcome::Imported(imported) = outcome else {
            panic!("expected import")
        };
        let derived =
            deserialize_derived_activity_data(imported.activity.derived_data_json.as_ref());
        assert_eq!(derived.route_points.len(), 16); // FIT wins geometry priority.
        assert_eq!(derived.recording.excluded(), excluded);
        assert_eq!(
            crate::heatmaps::preparation::prepare_route(&derived).is_empty(),
            excluded
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[tokio::test]
async fn repeated_provider_identity_preserves_new_trainer_evidence() {
    let db = test_db().await;
    let dir = test_uploads_dir();
    for trainer in [false, true] {
        let mut upload = recording_upload(false);
        upload.source_correlation_id = Some("99".into());
        let outcome = persist_activity_upload_with_artifacts(
            &db,
            PersistActivityUploadWithArtifactsRequest {
                uploads_dir: &dir,
                user_storage_key: "test-user",
                user_id: 1,
                upload,
                primary_artifact_kind: ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL,
                primary_source_quality: ACTIVITY_IMPORT_SOURCE_QUALITY_FIT_ORIGINAL,
                additional_artifacts: vec![provider_artifact(trainer)],
                source: "strava_sync",
                deduplication: ActivityUploadDeduplication::Enabled,
                training_profile: Some(&TrainingProfile::default()),
            },
        )
        .await
        .unwrap();
        let activity = match outcome {
            PersistActivityUploadOutcome::Imported(imported) => {
                assert!(!trainer);
                imported.activity
            }
            PersistActivityUploadOutcome::Duplicate(duplicate) => {
                assert!(trainer);
                duplicate.activity
            }
        };
        assert_eq!(activity.recording_context().excluded(), trainer);
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn evidence_writes_require_owner_and_current_source_version() {
    let db = test_db().await;
    let dir = test_uploads_dir();
    let imported = import_recording(&db, &dir, false).await;
    let mut data = deserialize_derived_activity_data(imported.activity.derived_data_json.as_ref());
    data.recording.observe("verified_creator", "zwift");
    assert!(!activities::Model::store_recording(
        &db,
        imported.activity.id,
        2,
        imported.activity.updated_at,
        &data
    )
    .await
    .unwrap());
    assert!(!activities::Model::store_recording(
        &db,
        imported.activity.id,
        1,
        imported.activity.updated_at - ChronoDuration::seconds(1),
        &data
    )
    .await
    .unwrap());
    let preserved = activities::Model::find_owned(&db, imported.activity.id, 1)
        .await
        .unwrap()
        .unwrap();
    assert!(!preserved.recording_context().excluded());
    assert!(activities::Model::store_recording(
        &db,
        imported.activity.id,
        1,
        imported.activity.updated_at,
        &data
    )
    .await
    .unwrap());
    assert!(activities::Model::find_owned(&db, imported.activity.id, 1)
        .await
        .unwrap()
        .unwrap()
        .recording_context()
        .excluded());
    std::fs::remove_dir_all(dir).unwrap();
}

fn assert_heatmap_recording(
    data: &crate::activity_data::ActivityDerivedData,
    excluded: bool,
    node: ActivityProcessingNode,
) {
    assert_eq!(data.recording.excluded(), excluded, "{node:?}");
    assert_eq!(data.route_points.len(), 16, "{node:?}");
    assert_eq!(
        crate::heatmaps::preparation::prepare_route(data).is_empty(),
        excluded,
        "{node:?}"
    );
}
