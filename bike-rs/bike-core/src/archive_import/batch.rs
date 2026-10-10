use super::*;
use crate::background_jobs::{batches, entities::worker_batches};
use crate::jobs::{Job, ProcessActivityImportTask};
use sea_orm::{ColumnTrait, ConnectionTrait, QueryFilter, QuerySelect, TransactionTrait};
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
    let job = activity_archive_import_jobs::Entity::find_by_id(job_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Archive job disappeared"))?;
    let txn = db.begin().await?;
    batches::lock_origins(&txn, batch.id).await?;
    let stored = worker_batches::Entity::find_by_id(batch.id)
        .lock_exclusive()
        .one(&txn)
        .await?
        .ok_or_else(|| AppError::internal("Archive batch disappeared"))?;
    if stored.cursor != batch.cursor || stored.sealed {
        return Ok(());
    }
    let mut jobs = Vec::new();
    for entry in scan
        .supported_entries
        .iter()
        .skip(batch.cursor as usize)
        .take(16)
    {
        let import = store_entry(&txn, uploads_dir, &job, &archive, entry).await?;
        jobs.push(Job::ProcessActivityImport(ProcessActivityImportTask {
            user_id: job.user_id,
            import_id: import.id,
        }));
    }
    let next = (batch.cursor + 16).min(scan.supported_entries.len() as i32);
    batches::schedule_page(&txn, &batch, next, next >= scan.supported_entry_count, jobs).await?;
    activity_archive_import_jobs::Entity::update_many()
        .set(activity_archive_import_jobs::ActiveModel {
            total_entries: Set(scan.total_entries),
            supported_entry_count: Set(scan.supported_entry_count),
            skipped_unsupported_count: Set(scan.skipped_unsupported_count),
            updated_at: Set(Utc::now()),
            ..Default::default()
        })
        .filter(activity_archive_import_jobs::Column::Id.eq(job_id))
        .exec(&txn)
        .await?;
    txn.commit().await?;
    Ok(())
}
async fn load_or_start(
    db: &DatabaseConnection,
    uploads_dir: &str,
    id: i32,
    job_id: i32,
) -> Result<worker_batches::Model, AppError> {
    if let Some(batch) = worker_batches::Entity::find_by_id(id).one(db).await? {
        return Ok(batch);
    }
    let job = activity_archive_import_jobs::Entity::find_by_id(job_id)
        .one(db)
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
    archive: &Path,
    entry: &IndexedArchiveActivityEntry,
) -> Result<crate::entities::activity_imports::Model, AppError> {
    let bytes = read_archive_entry_bytes(archive, &entry.source)?;
    let bytes =
        maybe_decode_archive_entry(&entry.activity_entry, bytes).map_err(AppError::bad_request)?;
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
pub async fn finish_queued_archive_batch(
    db: &impl ConnectionTrait,
    batch: &worker_batches::Model,
    tasks: &[crate::background_jobs::entities::diagnostic_reads::TaskMetadata],
) -> Result<bool, sea_orm::DbErr> {
    if batch.kind != "archive" {
        return Ok(false);
    }
    let id = batch.source["job_id"]
        .as_i64()
        .and_then(|id| i32::try_from(id).ok())
        .ok_or_else(|| sea_orm::DbErr::Custom("Archive job ID missing".into()))?;
    let job = activity_archive_import_jobs::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| sea_orm::DbErr::RecordNotFound("Archive job missing".into()))?;
    let imports = crate::entities::activity_imports::Entity::find()
        .filter(crate::entities::activity_imports::Column::ArchiveJobId.eq(id))
        .select_only()
        .column(crate::entities::activity_imports::Column::Status)
        .into_tuple::<String>()
        .all(db)
        .await?;
    let imported = imports
        .iter()
        .filter(|status| status.as_str() == "processed")
        .count() as i32;
    let duplicate = imports
        .iter()
        .filter(|status| status.as_str() == "duplicate")
        .count() as i32;
    let failed = (job.supported_entry_count - imported - duplicate).max(0);
    let errors = tasks
        .iter()
        .filter_map(|task| task.error.as_ref())
        .take(10)
        .collect::<Vec<_>>();
    activity_archive_import_jobs::Entity::update_many()
        .set(activity_archive_import_jobs::ActiveModel {
            imported_count: Set(imported),
            duplicate_count: Set(duplicate),
            failed_count: Set(failed),
            status: Set(if failed == 0 {
                "succeeded"
            } else if imported + duplicate > 0 {
                "partial"
            } else {
                "failed"
            }
            .into()),
            failure_message: Set(errors.first().map(|error| (*error).clone())),
            error_samples_json: Set(Some(
                serde_json::to_string(&errors)
                    .map_err(|error| sea_orm::DbErr::Custom(error.to_string()))?,
            )),
            resolved_url: Set(batch.source["resolved_url"].as_str().map(str::to_owned)),
            finished_at: Set(Some(Utc::now())),
            updated_at: Set(Utc::now()),
            ..Default::default()
        })
        .filter(activity_archive_import_jobs::Column::Id.eq(id))
        .exec(db)
        .await?;
    Ok(failed > 0)
}
