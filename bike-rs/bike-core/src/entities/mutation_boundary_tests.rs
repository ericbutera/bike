use super::*;
use crate::background_jobs::entities::{pipeline_outputs, work_units};
use crate::test_support;
use sea_orm::{ConnectionTrait, DatabaseConnection, EntityTrait, Schema};
use serde_json::json;
async fn database() -> DatabaseConnection {
    let db = super::model_boundary_tests::segment_database().await;
    let schema = Schema::new(db.get_database_backend());
    for statement in [
        schema.create_table_from_entity(user_preferences::Entity),
        schema.create_table_from_entity(activity_import_attempts::Entity),
        schema.create_table_from_entity(analytics_user_states::Entity),
        schema.create_table_from_entity(activity_training_analyses::Entity),
        schema.create_table_from_entity(strava_connections::Entity),
        schema.create_table_from_entity(pipeline_outputs::Entity),
        schema.create_table_from_entity(work_units::Entity),
        schema.create_table_from_entity(crate::background_jobs::entities::task_attempts::Entity),
    ] {
        db.execute(&statement).await.unwrap();
    }
    db
}

async fn segment(db: &DatabaseConnection) -> segments::Model {
    let active: segments::ActiveModel = test_support::segment().into();
    test_support::insert(db, active).await
}

#[tokio::test]
async fn segment_patch_is_atomic_and_unchanged_patch_does_not_write() {
    use segments::SegmentMode;
    let db = database().await;
    let original = segment(&db).await;
    let invalid = original
        .clone()
        .update_details(&db, Some(" "), Some(SegmentMode::Dh), Some(true))
        .await;
    assert_eq!(
        invalid.unwrap_err().status,
        axum::http::StatusCode::BAD_REQUEST
    );
    let unchanged = original
        .clone()
        .update_details(&db, None, None, None)
        .await
        .unwrap();
    assert_eq!(unchanged, original);
    let changed = original
        .update_details(&db, Some("  New name  "), Some(SegmentMode::Dh), Some(true))
        .await
        .unwrap();
    assert_eq!(
        (&*changed.title, &*changed.mode, changed.starred),
        ("New name", "dh", true)
    );
    assert_eq!(
        segments::Entity::find_by_id(changed.id)
            .one(&db)
            .await
            .unwrap(),
        Some(changed)
    );
}

#[tokio::test]
async fn segment_rename_rolls_back_if_analytics_write_fails() {
    let db = database().await;
    let original = segment(&db).await;
    let active: segment_efforts::ActiveModel = test_support::segment_effort().into();
    test_support::insert(&db, active).await;
    db.execute_unprepared("DROP TABLE activity_analytics")
        .await
        .unwrap();
    assert!(original
        .clone()
        .update_details(&db, Some("Renamed"), None, Some(true))
        .await
        .is_err());
    assert_eq!(
        segments::Entity::find_by_id(original.id)
            .one(&db)
            .await
            .unwrap(),
        Some(original)
    );
}

fn parsed_activity() -> crate::activity_parser::ParsedActivityData {
    crate::strava::provider_payload::parse_strava_provider_payload(include_bytes!(
        "../../testdata/strava-real-ride.json"
    ))
    .unwrap()
}

async fn builder_source(db: &DatabaseConnection, title: &str) -> segments::ActivitySegmentSource {
    let parsed = parsed_activity();
    let active: activities::ActiveModel = activities::Model {
        derived_data_json: Some(crate::activity_data::serialize_derived_activity_data(
            &parsed.derived_data,
        )),
        format: Some("json".into()),
        ..test_support::activity()
    }
    .into();
    test_support::insert(db, active).await;
    segments::ActivitySegmentSource::load(db, 7, 42, title, 0, 1)
        .await
        .unwrap()
}

#[tokio::test]
async fn segment_builder_creation_adoption_and_replacement_preserve_flags() {
    let db = database().await;
    let source = builder_source(&db, "Builder name").await;
    let created = segments::Model::create_from_activity(&db, &source)
        .await
        .unwrap();
    assert_eq!(created.source_activity_id, Some(42));
    assert_eq!(created.source_end_route_point_index, Some(1));
    assert_eq!(created.distance_meters, source.distance_meters());
    assert_eq!((created.mode.as_str(), created.starred), ("xc", false));
    let changed = created
        .update_details(&db, None, Some(segments::SegmentMode::Dh), Some(true))
        .await
        .unwrap();
    let renamed = segments::ActivitySegmentSource::load(&db, 7, 42, "Renamed", 0, 1)
        .await
        .unwrap();
    let adopted = changed.adopt_builder_source(&db, &renamed).await.unwrap();
    assert_eq!(
        (&*adopted.title, &*adopted.mode, adopted.starred),
        ("Renamed", "dh", true)
    );
    let replaced = adopted.replace_from_activity(&db, &renamed).await.unwrap();
    assert_eq!((&*replaced.mode, replaced.starred), ("dh", true));
    assert!(
        segments::ActivitySegmentSource::load(&db, 8, 42, "Foreign", 0, 1)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn segment_replacement_rejects_another_segments_route_without_writing() {
    let db = database().await;
    let source = builder_source(&db, "Builder name").await;
    let created = segments::Model::create_from_activity(&db, &source)
        .await
        .unwrap();
    let active: segments::ActiveModel = segments::Model {
        id: created.id + 1,
        ..test_support::segment()
    }
    .into();
    let other = test_support::insert(&db, active).await;
    assert!(other
        .clone()
        .replace_from_activity(&db, &source)
        .await
        .is_err());
    assert_eq!(
        segments::Entity::find_by_id(other.id)
            .one(&db)
            .await
            .unwrap(),
        Some(other)
    );
}

fn preferences(goal: bool) -> user_preferences::PreferencesUpdate {
    let mut value = json!({"unit_system":" mixed ","estimated_ftp_watts":260,
        "heart_rate_zone_bounds_bpm":[110,130,150,170]});
    if goal {
        value["xc_goal_start_date"] = json!("2026-09-01");
        value["xc_goal_target_date"] = json!("2026-10-31");
        value["xc_goal_target_distance_meters"] = json!(100000);
        value["xc_goal_target_elevation_gain_meters"] = json!(1500);
    }
    serde_json::from_value(value).unwrap()
}

#[tokio::test]
async fn preferences_insert_update_and_clear_keep_backfill_decisions() {
    use user_preferences::{GoalBackfillChange, Model};
    let db = database().await;
    let saved = Model::save_preferences(&db, 7, preferences(true))
        .await
        .unwrap();
    assert_eq!(saved.goal_backfill, GoalBackfillChange::Queue);
    assert_eq!(saved.model.unit_system, "mixed");
    saved
        .model
        .set_goal_backfill(&db, Some("completed"), Some(test_support::timestamp()))
        .await
        .unwrap();
    let saved = Model::save_preferences(&db, 7, preferences(true))
        .await
        .unwrap();
    assert_eq!(saved.goal_backfill, GoalBackfillChange::Unchanged);
    assert_eq!(
        saved.model.xc_goal_backfill_status.as_deref(),
        Some("completed")
    );
    let saved = Model::save_preferences(&db, 7, preferences(false))
        .await
        .unwrap();
    assert_eq!(saved.goal_backfill, GoalBackfillChange::Clear);
    assert_eq!(saved.model.estimated_ftp_watts, Some(260));
    assert!(saved.model.xc_goal_start_date.is_none());
    assert!(saved.model.xc_goal_backfill_status.is_none());
    assert!(saved.model.xc_goal_backfill_completed_at.is_none());
    assert!(Model::for_user(&db, 8).await.unwrap().is_none());
}

#[tokio::test]
async fn invalid_preferences_preserve_the_entire_previous_record() {
    let db = database().await;
    let saved = user_preferences::Model::save_preferences(&db, 7, preferences(true))
        .await
        .unwrap()
        .model;
    let mut update = preferences(false);
    update.unit_system = "metric".into();
    update.estimated_ftp_watts = Some(-1);
    assert!(user_preferences::Model::save_preferences(&db, 7, update)
        .await
        .is_err());
    assert_eq!(
        user_preferences::Model::for_user(&db, 7).await.unwrap(),
        Some(saved)
    );
}

#[tokio::test]
async fn import_state_transitions_keep_linkage_attempts_and_completion_times() {
    let db = database().await;
    let import = test_support::insert(&db, test_support::new_activity_import()).await;
    let processing = import
        .mark_processing(&db, "activity_saved", Some(42), test_support::timestamp())
        .await
        .unwrap();
    assert_eq!(processing.activity_id, Some(42));
    assert_eq!(processing.status, "processing");
    assert_eq!(
        processing.last_processing_event_at,
        Some(test_support::timestamp())
    );
    let failed = processing
        .mark_failed(&db, "segments_built", "fixture failure")
        .await
        .unwrap();
    assert_eq!(failed.processing_attempts, 1);
    assert_eq!(failed.processing_error.as_deref(), Some("fixture failure"));
    let retry = failed
        .mark_processing(&db, "activity_saved", None, test_support::timestamp())
        .await
        .unwrap();
    assert_eq!(retry.activity_id, Some(42));
    assert_eq!(retry.processing_attempts, 1);
    assert!(retry.processing_error.is_none());
    let processed = retry.mark_processed(&db).await.unwrap();
    assert_eq!(processed.processing_stage, "complete");
    assert!(processed.processing_error.is_none());
    assert!(processed.processed_at.is_some());
    let duplicate = import.mark_duplicate(&db, 99).await.unwrap();
    assert_eq!(
        (duplicate.status.as_str(), duplicate.activity_id),
        ("duplicate", Some(99))
    );
    let native = duplicate.mark_native_processed(&db).await.unwrap();
    assert_eq!(native.status, "processed");
    assert_eq!(native.activity_id, Some(99));
}

#[tokio::test]
async fn ingestion_attempt_records_and_finishes_reused_stage_state() {
    use crate::activity_import_execution::{self as execution, StageRecord};
    let db = database().await;
    let import = test_support::insert(&db, test_support::new_activity_import()).await;
    let attempt = execution::create_attempt(
        &db,
        &import,
        json!({"source":"fixture"}),
        "raw_stored",
        "raw_stored",
        None,
    )
    .await
    .unwrap();
    let record = StageRecord {
        stage: "activity_parsed".into(),
        status: "running".into(),
        started_at: Some(test_support::timestamp()),
        completed_at: None,
        summary: vec!["Parsed fixture".into()],
        error: None,
        reused_attempt_id: None,
    };
    let attempt = execution::record_stage(&db, attempt, record, None, None)
        .await
        .unwrap();
    assert_eq!(attempt.current_stage, "activity_parsed");
    execution::finish_active(&db, &import, "failed", Some("Parse failed"))
        .await
        .unwrap();
    let finished = activity_import_attempts::Entity::find_for_execution(&db, attempt.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(finished.status, "failed");
    assert!(finished.finished_at.is_some());
    assert_eq!(
        execution::stages(&finished)
            .unwrap()
            .into_iter()
            .find(|s| s.stage == "activity_parsed")
            .unwrap()
            .error
            .as_deref(),
        Some("Parse failed")
    );
    let replay = execution::create_attempt(
        &db,
        &import,
        json!({}),
        "activity_saved",
        "activity_saved",
        Some(&finished),
    )
    .await
    .unwrap();
    assert_eq!(replay.reused_attempt_id, Some(finished.id));
    assert_eq!(execution::stages(&replay).unwrap()[0].status, "reused");
    execution::finish_active(&db, &import, "duplicate", None)
        .await
        .unwrap();
    let finished = activity_import_attempts::Entity::find_for_execution(&db, replay.id)
        .await
        .unwrap()
        .unwrap();
    assert!(execution::stages(&finished)
        .unwrap()
        .iter()
        .any(|s| s.status == "skipped"));
}

#[tokio::test]
async fn copied_artifacts_keep_source_metadata_and_receive_new_ids() {
    let db = database().await;
    let active: activity_import_artifacts::ActiveModel = test_support::import_artifact().into();
    let original = test_support::insert(&db, active).await;
    let resized = original.clone().copy_with_size(&db, 999).await.unwrap();
    assert_ne!(resized.id, original.id);
    assert_eq!(resized.size_bytes, 999);
    let copied = original.clone().copy_to_import(&db, 77).await.unwrap();
    assert_eq!(copied.activity_import_id, 77);
    assert_eq!(copied.storage_path, original.storage_path);
    assert_eq!(copied.checksum_sha256, original.checksum_sha256);
    let active: activities::ActiveModel = test_support::activity().into();
    let activity = test_support::insert(&db, active).await;
    let adopted = activity.adopt_source_metadata(&db, &copied).await.unwrap();
    assert_eq!(adopted.original_filename, Some(copied.original_filename));
    assert_eq!(adopted.format, Some(copied.format));
}

fn connection_token() -> crate::strava::client::StravaAuthorizationTokenResponse {
    serde_json::from_value(json!({
        "access_token":"fixture-access","refresh_token":"fixture-refresh",
        "expires_at":2000000000,"scope":"activity:read_all",
        "athlete":{"id":123,"username":"fixture","firstname":"Original",
            "lastname":"Rider","profile_medium":"https://example.test/original.jpg"}
    }))
    .unwrap()
}

#[tokio::test]
async fn credential_mutations_encrypt_reconnections_and_keep_refresh_scope_fallback() {
    use crate::strava::{client::StravaRefreshTokenResponse, credentials};
    let db = database().await;

    let created = strava_connections::Model::connect_from_token(&db, 7, connection_token())
        .await
        .unwrap();
    assert_ne!(created.access_token, "fixture-access");
    assert_eq!(created.last_sync_status, "never");
    assert_eq!(
        (
            created.last_sync_imported_count,
            created.last_sync_duplicate_count,
            created.last_sync_failed_count
        ),
        (0, 0, 0)
    );
    assert_eq!(
        credentials::open(&created.access_token).unwrap(),
        "fixture-access"
    );
    let connected = strava_connections::Model::connect_from_token(&db, 7, connection_token())
        .await
        .unwrap();
    assert_eq!(created.id, connected.id);
    assert!(
        strava_connections::Model::connect_from_token(&db, 8, connection_token())
            .await
            .is_err()
    );
    let refresh: StravaRefreshTokenResponse = serde_json::from_value(json!({
        "access_token":"new-access","refresh_token":"new-refresh","expires_at":2100000000
    }))
    .unwrap();
    let updated = connected.refresh_credentials(&db, refresh).await.unwrap();
    assert_eq!(updated.scopes, "activity:read_all");
    assert_eq!(
        credentials::open(&updated.access_token).unwrap(),
        "new-access"
    );
    assert_eq!(updated.athlete_username.as_deref(), Some("fixture"));
}

#[tokio::test]
async fn work_evidence_records_counts_and_failures_without_changing_outcomes() {
    use crate::background_jobs::execution::{measured, ExecutionContext, WorkCounts, WorkScope};
    let db = database().await;
    let context = ExecutionContext {
        task_id: 9,
        attempt: 2,
        task_type: "process_activity_import".into(),
        metrics: None,
    };
    let scope = WorkScope {
        kind: "import",
        work_key: "import:18".into(),
        revision: "2".into(),
        mode: "full",
        reason: "test",
    };
    let result: Result<i32, &str> = context
        .clone()
        .scope(measured(&db, scope.clone(), async {
            Ok((
                42,
                WorkCounts {
                    rows_written: 3,
                    outcome: "completed",
                    ..Default::default()
                },
            ))
        }))
        .await;
    assert_eq!(result, Ok(42));
    let result: Result<i32, &str> = context
        .scope(measured(&db, scope, async { Err("fixture failure") }))
        .await;
    assert_eq!(result, Err("fixture failure"));
    let evidence = work_units::Model::page_for_task(&db, 9, 0).await.unwrap();
    assert_eq!(evidence.len(), 2);
    assert_eq!(evidence[0].counts.as_ref().unwrap()["rows_written"], 3);
    assert_eq!(evidence[1].outcome, "failed");
    assert_eq!(evidence[1].error.as_deref(), Some("fixture failure"));
}

fn archive_summary() -> crate::archive_import::ActivityArchiveImportResponse {
    crate::archive_import::ActivityArchiveImportResponse {
        source: "https://example.test/resolved.zip".into(),
        total_entries: 3,
        supported_entry_count: 2,
        imported_count: 1,
        duplicate_count: 1,
        skipped_unsupported_count: 1,
        failed_count: 0,
        error_samples: Vec::new(),
    }
}

#[tokio::test]
async fn archive_job_transitions_preserve_progress_and_failure_details() {
    let db = database().await;
    let queued = activity_archive_import_jobs::Model::queue(
        &db,
        7,
        "rider",
        "https://example.test/archive.zip".into(),
    )
    .await
    .unwrap();
    assert_eq!(queued.status, "queued");
    let running = activity_archive_import_jobs::Model::mark_running(&db, &queued)
        .await
        .unwrap();
    assert!(running.started_at.is_some());
    let summary = archive_summary();
    let finished = activity_archive_import_jobs::Model::mark_finished(&db, &running, &summary)
        .await
        .unwrap();
    assert_eq!(finished.imported_count, 1);
    assert_eq!(finished.duplicate_count, 1);
    assert!(finished.finished_at.is_some());
    let failed = activity_archive_import_jobs::Model::mark_failed(
        &db,
        &finished,
        None,
        "Download failed".into(),
    )
    .await
    .unwrap();
    assert_eq!(failed.status, "failed");
    assert_eq!(failed.failure_message.as_deref(), Some("Download failed"));
    assert_eq!(failed.imported_count, 1);
}

#[tokio::test]
async fn preference_service_returns_backfill_state_and_does_not_queue_unchanged_goal() {
    use crate::background_jobs::entities::background_tasks;
    let db = database().await;
    let schema = Schema::new(db.get_database_backend());
    for statement in [
        schema.create_table_from_entity(background_tasks::Entity),
        schema.create_table_from_entity(activity_import_locks::Entity),
    ] {
        db.execute(&statement).await.unwrap();
    }
    let queue = crate::jobs::JobQueue::new(db.clone());
    let saved = crate::services::preferences::update_preferences(&db, &queue, 7, preferences(true))
        .await
        .unwrap();
    assert_eq!(saved.xc_goal_backfill_status.as_deref(), Some("queued"));
    crate::services::preferences::update_preferences(&db, &queue, 7, preferences(true))
        .await
        .unwrap();
    let jobs = background_tasks::Entity::find().all(&db).await.unwrap();
    assert_eq!(jobs.len(), 1);
    let saved =
        crate::services::preferences::update_preferences(&db, &queue, 7, preferences(false))
            .await
            .unwrap();
    assert!(saved.xc_goal_backfill_status.is_none());
}

#[tokio::test]
async fn preference_service_keeps_saved_preferences_when_backfill_enqueue_fails() {
    let db = database().await;
    db.execute(
        &Schema::new(db.get_database_backend())
            .create_table_from_entity(activity_import_locks::Entity),
    )
    .await
    .unwrap();
    let queue = crate::jobs::JobQueue::new(db.clone());
    let error = crate::services::preferences::update_preferences(&db, &queue, 7, preferences(true))
        .await
        .unwrap_err();
    assert_eq!(error.status, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    assert!(error
        .message
        .starts_with("Failed to queue XC training backfill: "));
    let saved = user_preferences::Model::for_user(&db, 7)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.estimated_ftp_watts, Some(260));
    assert!(saved.xc_goal_start_date.is_some());
    assert_eq!(saved.xc_goal_backfill_status.as_deref(), Some("failed"));
    assert!(activity_import_locks::Model::for_user(&db, 7)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn adopting_builder_links_preserves_the_stored_route_and_import_metadata() {
    let db = database().await;
    let source = builder_source(&db, "Adopted name").await;
    let mut previous_route = source.route_points().to_vec();
    for point in &mut previous_route {
        point.longitude += 0.000001;
    }
    let active: segments::ActiveModel = segments::Model {
        source: "manual_segment_import".into(),
        mode: "dh".into(),
        starred: true,
        original_filename: Some("saved.gpx".into()),
        format: Some("gpx".into()),
        distance_meters: Some(999.0),
        route_data_json: Some(crate::segment_support::serialize_segment_route_points(
            &previous_route,
        )),
        ..test_support::segment()
    }
    .into();
    let original = test_support::insert(&db, active).await;
    let adopted = original
        .clone()
        .adopt_builder_source(&db, &source)
        .await
        .unwrap();
    assert_eq!(
        adopted,
        segments::Model {
            title: "Adopted name".into(),
            source_activity_id: Some(42),
            source_start_route_point_index: Some(0),
            source_end_route_point_index: Some(1),
            updated_at: adopted.updated_at,
            ..original
        }
    );
}

#[tokio::test]
async fn native_completion_keeps_prior_processing_events_and_import_identity() {
    let db = database().await;
    let active: activity_imports::ActiveModel = activity_imports::Model {
        source: "strava_sync".into(),
        activity_id: Some(42),
        archive_job_id: Some(55),
        processing_attempts: 3,
        processing_error: Some("Prior failure".into()),
        ..test_support::activity_import()
    }
    .into();
    let original = test_support::insert(&db, active).await;
    let completed = original.clone().mark_native_processed(&db).await.unwrap();
    assert!(completed.processed_at.is_some());
    assert_eq!(
        completed,
        activity_imports::Model {
            status: "processed".into(),
            processing_stage: "complete".into(),
            processing_error: None,
            processed_at: completed.processed_at,
            updated_at: completed.updated_at,
            ..original
        }
    );
}

#[tokio::test]
async fn credential_refresh_applies_supplied_profile_and_scope_without_resetting_sync_state() {
    use crate::strava::{client::StravaRefreshTokenResponse, credentials};
    let db = database().await;
    let active: strava_connections::ActiveModel = strava_connections::Model {
        last_sync_status: "completed".into(),
        last_sync_imported_count: 17,
        ..test_support::connection()
    }
    .into();
    let original = test_support::insert(&db, active).await;
    let refresh: StravaRefreshTokenResponse = serde_json::from_value(json!({
        "access_token":"new-access","refresh_token":"new-refresh","expires_at":2100000000,
        "scope":"read","athlete":{"id":124,"firstname":"Updated"}
    }))
    .unwrap();
    let updated = original
        .clone()
        .refresh_credentials(&db, refresh)
        .await
        .unwrap();
    assert_eq!(updated.scopes, "read");
    assert_eq!(updated.expires_at.timestamp(), 2100000000);
    assert_eq!(
        credentials::open(&updated.access_token).unwrap(),
        "new-access"
    );
    assert_eq!(
        credentials::open(&updated.refresh_token).unwrap(),
        "new-refresh"
    );
    assert_eq!(
        (
            updated.athlete_id,
            updated.athlete_username.as_deref(),
            updated.athlete_first_name.as_deref(),
            updated.athlete_last_name.as_deref(),
            updated.athlete_profile_medium_url.as_deref()
        ),
        (124, None, Some("Updated"), None, None)
    );
    assert_eq!(
        (updated.id, updated.user_id, updated.created_at),
        (original.id, original.user_id, original.created_at)
    );
    assert_eq!(
        (
            updated.last_sync_status.as_str(),
            updated.last_sync_imported_count
        ),
        ("completed", 17)
    );
}

#[tokio::test]
async fn archive_progress_updates_counts_without_finishing_and_restart_resets_them() {
    use crate::archive_import::ActivityArchiveImportResponse;
    let db = database().await;
    let active: activity_archive_import_jobs::ActiveModel =
        test_support::archive_import_job().into();
    let job = test_support::insert(&db, active).await;
    let running = activity_archive_import_jobs::Model::mark_running(&db, &job)
        .await
        .unwrap();
    let summary = ActivityArchiveImportResponse {
        total_entries: 9,
        supported_entry_count: 6,
        imported_count: 2,
        duplicate_count: 1,
        skipped_unsupported_count: 3,
        failed_count: 3,
        error_samples: vec!["Fixture problem".into()],
        ..archive_summary()
    };
    activity_archive_import_jobs::Entity::store_progress(
        &db,
        running.user_id,
        running.id,
        &summary,
    )
    .await
    .unwrap();
    let progressed =
        activity_archive_import_jobs::Entity::find_owned(&db, running.user_id, running.id)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(
        progressed,
        activity_archive_import_jobs::Model {
            total_entries: 9,
            supported_entry_count: 6,
            imported_count: 2,
            duplicate_count: 1,
            skipped_unsupported_count: 3,
            failed_count: 3,
            error_samples_json: Some(serde_json::to_string(&summary.error_samples).unwrap()),
            updated_at: progressed.updated_at,
            ..running
        }
    );
    let reset = activity_archive_import_jobs::Model::mark_running(&db, &progressed)
        .await
        .unwrap();
    assert!([
        reset.total_entries,
        reset.supported_entry_count,
        reset.imported_count,
        reset.duplicate_count,
        reset.skipped_unsupported_count,
        reset.failed_count
    ]
    .into_iter()
    .all(|count| count == 0));
    assert!(reset.error_samples_json.is_none());
    assert!(reset.finished_at.is_none());
}

#[tokio::test]
async fn task_insert_defaults_keep_the_payload_schedule_and_retry_policy() {
    use crate::background_jobs::entities::background_tasks;
    let db = database().await;
    db.execute(
        &Schema::new(db.get_database_backend()).create_table_from_entity(background_tasks::Entity),
    )
    .await
    .unwrap();
    let scheduled_for = test_support::timestamp();
    let payload = json!({"data":{"user_id":7}});
    let task = background_tasks::Model::enqueue(
        &db,
        "rebuild_fitness_freshness".into(),
        payload.clone(),
        Some(scheduled_for),
        9,
    )
    .await
    .unwrap();
    assert!(task.id > 0);
    assert_eq!(task.payload, payload);
    assert_eq!(task.scheduled_for, Some(scheduled_for));
    assert_eq!(task.max_attempts, 9);
    assert_eq!((task.status.as_str(), task.attempts), ("pending", 0));
    assert_eq!(
        (task.error, task.result, task.started_at, task.completed_at),
        (None, None, None, None)
    );
    assert_eq!(task.created_at, task.updated_at);
}

#[tokio::test]
async fn attempt_insert_defaults_keep_the_requested_stages_and_import_link() {
    let db = database().await;
    let import = test_support::insert(&db, test_support::new_activity_import()).await;
    let source = json!({"source":"fixture"});
    let attempt = activity_import_attempts::Model::create_attempt(
        &db,
        &import,
        source.clone(),
        "activity_saved",
        "activity_saved",
        None,
    )
    .await
    .unwrap();
    assert!(attempt.id > 0);
    assert_eq!(
        (
            attempt.user_id,
            attempt.activity_import_id,
            attempt.activity_id
        ),
        (import.user_id, import.id, import.activity_id)
    );
    assert_eq!(attempt.source_json, source);
    assert_eq!(
        (
            attempt.status.as_str(),
            attempt.requested_stage.as_str(),
            attempt.start_stage.as_str(),
            attempt.current_stage.as_str()
        ),
        (
            "queued",
            "activity_saved",
            "activity_saved",
            "activity_saved"
        )
    );
    assert_eq!(
        (attempt.error, attempt.started_at, attempt.finished_at),
        (None, None, None)
    );
    assert_eq!(attempt.created_at, attempt.updated_at);
}
