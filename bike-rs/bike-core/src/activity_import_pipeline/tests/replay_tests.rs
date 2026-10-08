use super::*;

#[tokio::test]
async fn replay_queue_failure_rolls_back_import_and_attempt_state() {
    let db = test_db().await;
    let dir = test_uploads_dir();
    let imported = completed_import(&db, &dir).await;
    let metadata = activity_import_attempts::Entity::recent_metadata(&db, 1, imported.import.id)
        .await
        .unwrap();
    assert!(metadata[0].checkpoint_json.is_none());
    let before = activity_imports::Entity::find_by_id(imported.import.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    db.execute_unprepared("DROP TABLE background_tasks")
        .await
        .unwrap();
    assert!(
        queue_activity_import_replay(&db, &dir, &before, "activity_saved")
            .await
            .is_err()
    );
    assert_eq!(
        activity_imports::Entity::find_by_id(before.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap(),
        before
    );
    assert_eq!(
        activity_import_attempts::Entity::recent(&db, 1, before.id)
            .await
            .unwrap()
            .len(),
        1
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn finalization_queue_failure_closes_attempt_without_rewriting_completed_stages() {
    let db = test_db().await;
    let dir = test_uploads_dir();
    let outcome = persist_test_activity_upload(
        &db,
        &dir,
        cycling_gpx_upload(),
        "manual_upload",
        &TrainingProfile::default(),
    )
    .await
    .unwrap();
    let PersistActivityUploadOutcome::Imported(imported) = outcome else {
        panic!("expected import");
    };
    let schema = Schema::new(db.get_database_backend());
    db.execute(&schema.create_table_from_entity(crate::entities::analytics_user_states::Entity))
        .await
        .unwrap();
    db.execute_unprepared("DROP TABLE background_tasks")
        .await
        .unwrap();
    let error = crate::activity_import_lifecycle::complete_activity_imports(
        &db,
        &crate::jobs::JobQueue::new(db.clone()),
        1,
        &[imported.import.id],
        imported.affected_segment_ids,
        Some(imported.fitness_dirty_from_day),
    )
    .await
    .expect_err("finalization rejects a failed queue operation");
    assert!(error.message.contains("Could not queue fitness rebuild"));
    let attempt = activity_import_attempts::Entity::recent(&db, 1, imported.import.id)
        .await
        .unwrap()
        .remove(0);
    assert_eq!(attempt.status, "failed");
    assert!(attempt.finished_at.is_some());
    assert!(execution::stages(&attempt)
        .unwrap()
        .iter()
        .all(|stage| stage.status == "completed"));
    let import = activity_imports::Entity::find_by_id(imported.import.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(import.status, "failed");
    assert_eq!(import.processing_stage, "finalizing");
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn worker_restart_preserves_interrupted_archive_attempt_and_queues_new_execution() {
    let db = test_db().await;
    let dir = test_uploads_dir();
    let imported = completed_import(&db, &dir).await;
    let mut active: activity_imports::ActiveModel = imported.import.clone().into();
    active.source = Set("archive_url_import".into());
    let import = active.update(&db).await.unwrap();
    let (_, id) = queue_activity_import_replay(&db, &dir, &import, "activity_parsed")
        .await
        .unwrap();
    assert!(activity_import_attempts::Entity::claim(&db, id)
        .await
        .unwrap());
    let queued = activity_import_attempts::Entity::find_by_id(id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    execution::record_stage(
        &db,
        queued,
        execution::StageRecord {
            stage: "activity_parsed".into(),
            status: "running".into(),
            started_at: Some(Utc::now()),
            completed_at: None,
            summary: vec![],
            error: None,
            reused_attempt_id: None,
        },
        None,
        import.activity_id,
    )
    .await
    .unwrap();
    assert_eq!(
        crate::activity_import_recovery::recover_abandoned_activity_imports_after_worker_start(
            &db,
            &crate::jobs::JobQueue::new(db.clone()),
            Utc::now() + ChronoDuration::seconds(ACTIVITY_IMPORT_STALE_PROCESSING_SECONDS + 30)
        )
        .await
        .unwrap(),
        1
    );
    let interrupted = activity_import_attempts::Entity::find_by_id(id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(interrupted.status, "failed");
    assert_eq!(execution::stages(&interrupted).unwrap()[1].status, "failed");
    let task = background_tasks::Entity::find()
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert!(task.payload["data"].get("attempt_id").is_none());
    reprocess_activity_from_import(
        &db,
        &dir,
        1,
        imported.activity,
        import.clone(),
        Some(&TrainingProfile::default()),
    )
    .await
    .unwrap();
    mark_activity_imports_processed(&db, &[import.id])
        .await
        .unwrap();
    let attempts = activity_import_attempts::Entity::recent(&db, 1, import.id)
        .await
        .unwrap();
    assert_eq!(attempts[0].status, "completed");
    assert_eq!(attempts[1], interrupted);
    std::fs::remove_dir_all(dir).unwrap();
}

async fn completed_import(db: &DatabaseConnection, dir: &str) -> PersistedActivityImport {
    let outcome = persist_test_activity_upload(
        db,
        dir,
        cycling_gpx_upload(),
        "manual_upload",
        &TrainingProfile::default(),
    )
    .await
    .unwrap();
    let PersistActivityUploadOutcome::Imported(imported) = outcome else {
        panic!("expected import");
    };
    mark_activity_imports_processed(db, &[imported.import.id])
        .await
        .unwrap();
    imported
}

#[tokio::test]
async fn recovery_restores_a_missing_task_without_replacing_a_queued_checkpoint_replay() {
    let db = test_db().await;
    let dir = test_uploads_dir();
    let imported = completed_import(&db, &dir).await;
    let (_, id) = queue_activity_import_replay(&db, &dir, &imported.import, "activity_saved")
        .await
        .unwrap();
    background_tasks::Entity::delete_many()
        .exec(&db)
        .await
        .unwrap();
    crate::activity_import_recovery::recover_abandoned_activity_imports_after_worker_start(
        &db,
        &crate::jobs::JobQueue::new(db.clone()),
        Utc::now() + ChronoDuration::seconds(ACTIVITY_IMPORT_STALE_PROCESSING_SECONDS + 30),
    )
    .await
    .unwrap();
    let restored = background_tasks::Entity::find()
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(restored.payload["data"]["attempt_id"], id);
    let import = activity_imports::Entity::find_by_id(imported.import.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(import.processing_stage, "activity_saved");
    assert_eq!(
        activity_import_attempts::Entity::find_by_id(id)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .status,
        "queued"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn recovery_preserves_a_live_archive_parent_even_when_a_completed_entry_is_old() {
    let db = test_db().await;
    let dir = test_uploads_dir();
    let imported = completed_import(&db, &dir).await;
    let mut active: activity_imports::ActiveModel = imported.import.clone().into();
    active.source = Set("archive_url_import".into());
    active.archive_job_id = Set(Some(9));
    let import = active.update(&db).await.unwrap();
    let (_, id) = queue_activity_import_replay(&db, &dir, &import, "activity_saved")
        .await
        .unwrap();
    assert!(activity_import_attempts::Entity::claim(&db, id)
        .await
        .unwrap());
    let now = Utc::now();
    let stale = now - ChronoDuration::seconds(ACTIVITY_IMPORT_STALE_PROCESSING_SECONDS + 30);
    let mut active: activity_imports::ActiveModel = import.clone().into();
    active.last_processing_event_at = Set(Some(stale));
    active.update(&db).await.unwrap();
    let task = insert_process_activity_import_task(&db, import.id, "processing", now).await;
    let mut parent: background_tasks::ActiveModel = task.into();
    parent.task_type = Set("activity_archive_import".into());
    parent.payload = Set(serde_json::json!({"data":{"job_id":9}}));
    parent.update(&db).await.unwrap();
    assert_eq!(
        crate::activity_import_recovery::recover_abandoned_activity_imports_after_worker_start(
            &db,
            &crate::jobs::JobQueue::new(db.clone()),
            now
        )
        .await
        .unwrap(),
        0
    );
    let attempt = activity_import_attempts::Entity::find_by_id(id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(attempt.status, "running");
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn queued_replay_rejects_a_checkpoint_changed_before_worker_execution() {
    let db = test_db().await;
    let dir = test_uploads_dir();
    let imported = completed_import(&db, &dir).await;
    let (_, id) = queue_activity_import_replay(&db, &dir, &imported.import, "segments_built")
        .await
        .unwrap();
    let mut active: activities::ActiveModel = imported.activity.into();
    active.updated_at = Set(Utc::now());
    let activity = active.update(&db).await.unwrap();
    let error = reprocess_activity_from_import(
        &db,
        &dir,
        1,
        activity,
        imported.import.clone(),
        Some(&TrainingProfile::default()),
    )
    .await
    .err()
    .expect("queued replay rejects a changed checkpoint");
    assert_eq!(error.status, 409);
    let attempt = activity_import_attempts::Entity::find_by_id(id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(attempt.status, "failed");
    assert!(attempt
        .error
        .unwrap()
        .contains("changed after replay was queued"));
    let plan = plan_activity_import_replay(&db, &dir, &imported.import, "segments_built")
        .await
        .unwrap();
    assert_eq!(plan.start_stage, "raw_stored");
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn replay_from_every_stage_reuses_prerequisites_and_finishes_descendants() {
    let db = test_db().await;
    let dir = test_uploads_dir();
    let imported = completed_import(&db, &dir).await;
    let order = activity_processing_topological_order().unwrap();
    for (index, stage) in order.iter().enumerate() {
        let import = activity_imports::Entity::find_owned(&db, 1, imported.import.id)
            .await
            .unwrap()
            .unwrap();
        let (plan, attempt_id) = queue_activity_import_replay(&db, &dir, &import, stage.id())
            .await
            .unwrap();
        assert_eq!(plan.start_stage, stage.id());
        let activity = activities::Model::find_owned(&db, imported.activity.id, 1)
            .await
            .unwrap()
            .unwrap();
        let replayed = reprocess_activity_from_import(
            &db,
            &dir,
            1,
            activity,
            import,
            Some(&TrainingProfile::default()),
        )
        .await
        .unwrap();
        assert_eq!(replayed.activity.id, imported.activity.id);
        mark_activity_imports_processed(&db, &[imported.import.id])
            .await
            .unwrap();
        let attempt = activity_import_attempts::Entity::find_by_id(attempt_id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(attempt.status, "completed");
        for (position, record) in execution::stages(&attempt).unwrap().iter().enumerate() {
            assert_eq!(
                record.status,
                if position < index {
                    "reused"
                } else {
                    "completed"
                }
            );
            assert!(!record.summary.is_empty());
            if position < index {
                assert_eq!(record.reused_attempt_id, plan.reused_attempt_id);
            }
        }
    }
    assert_eq!(activities::Entity::find().count(&db).await.unwrap(), 1);
    assert_eq!(
        activity_import_attempts::Entity::recent(&db, 1, imported.import.id)
            .await
            .unwrap()
            .len(),
        8
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn failed_parse_without_an_activity_preserves_failure_and_can_be_replayed() {
    let db = test_db().await;
    let dir = test_uploads_dir();
    let mut upload = cycling_gpx_upload();
    upload.bytes = b"not a GPX document".to_vec();
    assert!(persist_test_activity_upload(
        &db,
        &dir,
        upload,
        "manual_upload",
        &TrainingProfile::default()
    )
    .await
    .is_err());
    let import = activity_imports::Entity::find()
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(import.status, "failed");
    assert_eq!(import.processing_stage, "activity_parsed");
    assert_eq!(import.activity_id, None);
    let first = activity_import_attempts::Entity::recent(&db, 1, import.id)
        .await
        .unwrap()
        .remove(0);
    let records = execution::stages(&first).unwrap();
    assert_eq!(records[0].status, "completed");
    assert_eq!(records[1].status, "failed");
    assert_eq!(records[2].status, "pending");
    let (plan, second_id) = queue_activity_import_replay(&db, &dir, &import, "activity_parsed")
        .await
        .unwrap();
    assert_eq!(plan.start_stage, "activity_parsed");
    assert!(process_stored_activity_import(
        &db,
        &dir,
        1,
        import.clone(),
        ActivityUploadDeduplication::Enabled,
        None
    )
    .await
    .is_err());
    let attempts = activity_import_attempts::Entity::recent(&db, 1, import.id)
        .await
        .unwrap();
    assert_eq!(attempts[0].id, second_id);
    assert_eq!(attempts[0].status, "failed");
    assert_eq!(attempts[1], first);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn stale_checkpoints_fall_back_and_corrupt_sources_block_replay() {
    let db = test_db().await;
    let dir = test_uploads_dir();
    let imported = completed_import(&db, &dir).await;
    let mut active: activities::ActiveModel = imported.activity.clone().into();
    active.title = Set("Changed after ingest".into());
    active.updated_at = Set(Utc::now());
    active.update(&db).await.unwrap();
    let plan = plan_activity_import_replay(&db, &dir, &imported.import, "segments_built")
        .await
        .unwrap();
    assert_eq!(plan.start_stage, "raw_stored");
    assert!(plan.reason.is_some());
    std::fs::write(
        Path::new(&dir).join(&imported.import.storage_path),
        b"corrupt",
    )
    .unwrap();
    assert!(
        queue_activity_import_replay(&db, &dir, &imported.import, "raw_stored")
            .await
            .is_err()
    );
    assert_eq!(
        activity_import_attempts::Entity::recent(&db, 1, imported.import.id)
            .await
            .unwrap()
            .len(),
        1
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn a_second_replay_cannot_queue_while_the_first_is_active() {
    let db = test_db().await;
    let dir = test_uploads_dir();
    let imported = completed_import(&db, &dir).await;
    queue_activity_import_replay(&db, &dir, &imported.import, "raw_stored")
        .await
        .unwrap();
    let error = queue_activity_import_replay(&db, &dir, &imported.import, "raw_stored")
        .await
        .unwrap_err();
    assert_eq!(error.status, 409);
    std::fs::remove_dir_all(dir).unwrap();
}
