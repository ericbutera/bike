use super::*;
use crate::background_jobs::{batches, entities::worker_batches};
use crate::jobs::{Job, ProcessActivityImportTask};
use sea_orm::{ConnectionTrait, TransactionTrait};
/// Each producer invocation stores at most sixteen raw sources and commits the
/// children and cursor together. The retained archive belongs to the batch.
pub async fn queue_archive_page(
    db: &DatabaseConnection,
    uploads_dir: &str,
    task_id: i32,
    job_id: i32,
    batch_id: Option<i32>,
) -> Result<(), AppError> {
    let batch = load_or_start(db, uploads_dir, batch_id.unwrap_or(task_id), job_id).await?;
    if batch.sealed || batch.status != "running" {
        return Ok(());
    }
    let archive = PathBuf::from(
        batch.source["archive_path"]
            .as_str()
            .ok_or_else(|| AppError::internal("Archive batch source is missing"))?,
    );
    let scan = scan_archive_entries(&archive)?;
    let txn = db.begin().await?;
    batches::lock_origins(&txn, batch.id).await?;
    let stored = worker_batches::Model::lock(&txn, batch.id)
        .await?
        .ok_or_else(|| AppError::internal("Archive batch disappeared"))?;
    if stored.cursor != batch.cursor || stored.sealed {
        return Ok(());
    }
    let job = activity_archive_import_jobs::Entity::find_for_batch(&txn, job_id)
        .await?
        .ok_or_else(|| AppError::not_found("Archive job disappeared"))?;
    let mut jobs = Vec::new();
    let mut errors = Vec::new();
    for entry in scan
        .supported_entries
        .iter()
        .skip(batch.cursor as usize)
        .take(16)
    {
        match read_entry_source(&archive, entry) {
            Ok(bytes) => {
                let import = store_entry(&txn, uploads_dir, &job, entry, bytes).await?;
                jobs.push(Job::ProcessActivityImport(ProcessActivityImportTask {
                    user_id: job.user_id,
                    import_id: import.id,
                }));
            }
            Err(error) => errors.push(format!("{}: {}", entry.entry_name, error.message)),
        }
    }
    let next = (batch.cursor + 16).min(scan.supported_entries.len() as i32);
    batches::schedule_page(&txn, &batch, next, next >= scan.supported_entry_count, jobs).await?;
    job.store_queued_page_progress(&txn, &scan, errors).await?;
    txn.commit().await?;
    Ok(())
}
async fn load_or_start(
    db: &DatabaseConnection,
    uploads_dir: &str,
    id: i32,
    job_id: i32,
) -> Result<worker_batches::Model, AppError> {
    if let Some(batch) = worker_batches::Model::find_by_id(db, id).await? {
        return Ok(batch);
    }
    let job = activity_archive_import_jobs::Entity::find_for_batch(db, job_id)
        .await?
        .ok_or_else(|| AppError::not_found("Archive job disappeared"))?;
    let lock = mark_user_activity_import_lock_stage(
        db,
        job.user_id,
        ACTIVITY_IMPORT_LOCK_SOURCE_ARCHIVE_IMPORT,
        ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
    )
    .await?;
    let running = mark_activity_archive_import_job_running(db, &job).await?;
    let downloaded = match download_archive_from_url(uploads_dir, &job.archive_url).await {
        Ok(downloaded) => downloaded,
        Err(error) => {
            mark_activity_archive_import_job_failed(db, &running, None, error.message.clone())
                .await?;
            release_user_activity_import_lock(
                db,
                job.user_id,
                ACTIVITY_IMPORT_LOCK_SOURCE_ARCHIVE_IMPORT,
            )
            .await?;
            return Err(error);
        }
    };
    Ok(batches::start(db,id,job.user_id,"archive",serde_json::json!({"job_id":job_id,"archive_path":downloaded.archive_path,"resolved_url":downloaded.final_url,"lock_id":lock.id})).await?)
}
async fn store_entry(
    db: &impl ConnectionTrait,
    uploads_dir: &str,
    job: &activity_archive_import_jobs::Model,
    entry: &IndexedArchiveActivityEntry,
    bytes: Vec<u8>,
) -> Result<crate::entities::activity_imports::Model, AppError> {
    let import = store_activity_upload_import_with_artifacts(
        db,
        StoreActivityUploadImportRequest {
            uploads_dir,
            user_storage_key: &job.user_storage_key,
            user_id: job.user_id,
            upload: ActivityUploadPayload {
                original_filename: entry.activity_entry.original_filename.clone(),
                format: entry.activity_entry.format.clone(),
                mime_type: None,
                source_correlation_id: None,
                bytes,
            },
            source: "archive_url_import",
            primary_artifact_kind: "original",
            primary_source_quality:
                crate::activity_import_pipeline::original_source_quality_for_format(
                    &entry.activity_entry.format,
                ),
            additional_artifacts: Vec::new(),
        },
    )
    .await?;
    crate::entities::activity_imports::Entity::attach_archive_job(
        db,
        job.user_id,
        import.id,
        job.id,
    )
    .await?;
    Ok(import)
}

fn read_entry_source(
    archive: &Path,
    entry: &IndexedArchiveActivityEntry,
) -> Result<Vec<u8>, AppError> {
    let bytes = read_archive_entry_bytes(archive, &entry.source)?;
    maybe_decode_archive_entry(&entry.activity_entry, bytes).map_err(AppError::bad_request)
}

impl activity_archive_import_jobs::Model {
    async fn store_queued_page_progress(
        &self,
        db: &impl ConnectionTrait,
        scan: &ArchiveScanResult,
        errors: Vec<String>,
    ) -> Result<(), AppError> {
        let failed = errors.len() as i32;
        let samples = decode_error_samples(self.error_samples_json.as_deref())
            .into_iter()
            .chain(errors)
            .take(10)
            .collect::<Vec<_>>();
        let progress = ActivityArchiveImportResponse {
            source: self.archive_url.clone(),
            total_entries: scan.total_entries,
            supported_entry_count: scan.supported_entry_count,
            skipped_unsupported_count: scan.skipped_unsupported_count,
            imported_count: self.imported_count,
            duplicate_count: self.duplicate_count,
            failed_count: self.failed_count + failed,
            error_samples: samples,
        };
        activity_archive_import_jobs::Entity::store_progress(db, self.user_id, self.id, &progress)
            .await?;
        Ok(())
    }
}
pub async fn finish_queued_archive_batch(
    db: &impl ConnectionTrait,
    batch: &worker_batches::Model,
    tasks: &[crate::background_jobs::entities::diagnostic_reads::TaskMetadata],
) -> Result<bool, sea_orm::DbErr> {
    activity_archive_import_jobs::Model::finish_batch(db, batch, tasks).await
}
