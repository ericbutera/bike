use crate::entities::{activity_archive_import_jobs, activity_import_artifacts};

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
