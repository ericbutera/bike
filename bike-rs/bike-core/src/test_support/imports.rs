use crate::entities::{activity_archive_import_jobs, activity_import_artifacts};

pub fn activity_import_attempt() -> crate::entities::activity_import_attempts::Model {
    let execution = super::import_execution();
    let at = super::timestamp();
    crate::entities::activity_import_attempts::Model {
        id: 19,
        user_id: execution.command.source.user_id,
        activity_import_id: execution.command.source.activity_import_id,
        activity_id: None,
        worker_task_id: None,
        command_id: Some(execution.command.source.command_id.clone()),
        temporal_context: Some(serde_json::to_value(&execution).expect("valid fixture execution")),
        status: "started".into(),
        requested_stage: "raw_stored".into(),
        start_stage: "raw_stored".into(),
        current_stage: "raw_stored".into(),
        reused_attempt_id: None,
        source_json: serde_json::json!({
            "filename": super::activity_import().original_filename,
            "format": super::activity_import().format,
        }),
        stages_json: serde_json::to_value(
            crate::activity_import_execution::initial_stages("raw_stored", None).expect("valid fixture stages"),
        )
        .expect("valid fixture stage records"),
        checkpoint_json: None,
        error: None,
        created_at: at,
        started_at: Some(at),
        finished_at: None,
        updated_at: at,
    }
}

pub fn import_artifact() -> activity_import_artifacts::Model {
    let import = super::activity_import();
    activity_import_artifacts::Model {
        id: 19,
        activity_import_id: import.id,
        user_id: import.user_id,
        artifact_kind: "original".into(),
        format: import.format,
        source_quality: "gpx_original".into(),
        original_filename: import.original_filename,
        storage_path: import.storage_path,
        size_bytes: import.size_bytes,
        mime_type: import.mime_type,
        checksum_sha256: "fixture-checksum".into(),
        created_at: import.created_at,
        updated_at: import.updated_at,
    }
}

pub fn archive_import_job() -> activity_archive_import_jobs::Model {
    let at = super::timestamp();
    activity_archive_import_jobs::Model {
        id: 20,
        user_id: super::customer().id,
        user_storage_key: "fixture-user".into(),
        archive_url: "https://example.com/export.zip".into(),
        resolved_url: None,
        status: "queued".into(),
        failure_message: None,
        error_samples_json: None,
        total_entries: 0,
        supported_entry_count: 0,
        imported_count: 0,
        duplicate_count: 0,
        skipped_unsupported_count: 0,
        failed_count: 0,
        created_at: at,
        started_at: None,
        finished_at: None,
        updated_at: at,
    }
}
