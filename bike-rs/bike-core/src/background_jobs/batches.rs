//! Bounded producers yield to their children; a durable barrier owns user locks.
mod reprocessing;
pub use reprocessing::reprocess_page;
mod segments;
use super::entities::{background_tasks, batch_tasks, pipeline_outputs, worker_batches};
use chrono::Utc;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbErr, TransactionSession, TransactionTrait};
pub use segments::{affected_segments, segments_page};
use std::future::Future;
tokio::task_local! { static CURRENT:Option<i32>; }
pub fn current() -> Option<i32> {
    CURRENT.try_with(|id| *id).ok().flatten()
}
pub async fn scope<F: Future>(id: Option<i32>, future: F) -> F::Output {
    CURRENT.scope(id, future).await
}
pub async fn record_task(db: &impl ConnectionTrait, batch_id: i32, task_id: i32) -> Result<(), DbErr> {
    worker_batches::Model::lock(db, batch_id)
        .await?
        .filter(|batch| batch.status == "running")
        .ok_or_else(|| DbErr::RecordNotFound("Open batch disappeared".into()))?;
    batch_tasks::Model::attach(db, batch_id, task_id).await?;
    Ok(())
}
pub async fn start(
    db: &DatabaseConnection,
    id: i32,
    user_id: i32,
    kind: &str,
    source: serde_json::Value,
) -> Result<worker_batches::Model, DbErr> {
    let txn = db.begin().await?;
    lock_origins(&txn, id).await?;
    worker_batches::Model::create_if_absent(&txn, id, user_id, kind, source).await?;
    let batch = worker_batches::Model::find_by_id(&txn, id)
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("Batch disappeared".into()))?;
    pipeline_outputs::Model::require(&txn, "batch", id, id.to_string()).await?;
    txn.commit().await?;
    Ok(batch)
}
/// Page cursor, child intents, and the continuation are one atomic commit.
pub async fn schedule_page(
    db: &(impl ConnectionTrait + TransactionTrait),
    batch: &worker_batches::Model,
    next: i32,
    sealed: bool,
    jobs: Vec<super::super::jobs::Job>,
) -> Result<(), DbErr> {
    scope(Some(batch.id), async {
        let txn = db.begin().await?;
        lock_origins(&txn, batch.id).await?;
        let stored = worker_batches::Model::lock(&txn, batch.id)
            .await?
            .ok_or_else(|| DbErr::RecordNotFound("Batch disappeared".into()))?;
        if stored.cursor != batch.cursor || stored.sealed {
            return Ok(());
        }
        for job in jobs {
            background_tasks::Model::enqueue(
                &txn,
                job.task_type().into(),
                serde_json::to_value(job).map_err(|e| DbErr::Custom(e.to_string()))?,
                None,
                3,
            )
            .await?;
        }
        batch.advance_page(&txn, next, sealed).await?;
        if !sealed {
            let parent = background_tasks::Model::find_by_id(&txn, batch.id)
                .await?
                .ok_or_else(|| DbErr::RecordNotFound("Batch producer disappeared".into()))?;
            let mut payload = parent.payload;
            payload["data"]["batch_id"] = batch.id.into();
            background_tasks::Model::enqueue(&txn, parent.task_type, payload, None, 3).await?;
        }
        txn.commit().await
    })
    .await
}
/// Failed/canceled children produce a terminal partial result, never success.
pub async fn reconcile(db: &DatabaseConnection) -> Result<(), DbErr> {
    let mut after_id = 0;
    loop {
        let candidates = worker_batches::Model::running_page(db, after_id, 64).await?;
        if candidates.is_empty() {
            break;
        }
        for batch in candidates {
            after_id = batch.id;
            close_batch(db, batch).await?;
        }
    }
    cleanup_archives(db).await
}
async fn close_batch(db: &DatabaseConnection, batch: worker_batches::Model) -> Result<(), DbErr> {
    let txn = db.begin().await?;
    lock_origins(&txn, batch.id).await?;
    let Some(batch) = worker_batches::Model::lock(&txn, batch.id).await? else {
        return Ok(());
    };
    if batch.status != "running" {
        return Ok(());
    }
    let tasks = background_tasks::Model::metadata_for_batch(&txn, batch.id).await?;
    if tasks
        .iter()
        .any(|task| ["pending", "processing"].contains(&task.status.as_str()))
    {
        return Ok(());
    }
    let failed = tasks.iter().any(|task| task.status != "completed");
    if !batch.sealed && !failed {
        return Ok(());
    }
    if !failed && segments::schedule_segment_caches(&txn, &batch).await? {
        txn.commit().await?;
        return Ok(());
    }
    let failed = finish_batch(&txn, &batch, &tasks, failed).await?;
    txn.commit().await?;
    tracing::info!(
        batch_id = batch.id,
        user_id = batch.user_id,
        failed,
        "Worker batch reached its child barrier"
    );
    Ok(())
}

async fn finish_batch(
    db: &impl ConnectionTrait,
    batch: &worker_batches::Model,
    tasks: &[super::entities::diagnostic_reads::TaskMetadata],
    failed: bool,
) -> Result<bool, DbErr> {
    let archive_failed = crate::archive_import::finish_queued_archive_batch(db, batch, tasks).await?;
    let failed = failed || archive_failed;
    let now = Utc::now();
    batch.finish(db, failed, now).await?;
    let lock_source = match batch.kind.as_str() {
        "archive" => "archive_import",
        "segments" => "segment_regeneration",
        _ => "activity_reprocessing",
    };
    let lock_id = batch.source["lock_id"].as_i64().and_then(|id| i32::try_from(id).ok());
    crate::entities::activity_import_locks::Model::release_batch_lock(db, lock_id, batch.user_id, lock_source).await?;
    if !failed {
        pipeline_outputs::Model::publish(db, "batch", batch.id, &batch.id.to_string(), now).await?;
    }
    Ok(failed)
}
/// Enqueue locks a pipeline before joining its batch. Producers and barriers use
/// the same ordered locks so a descendant enqueue cannot deadlock with closure.
pub(crate) async fn lock_origins(db: &impl ConnectionTrait, task_id: i32) -> Result<(), DbErr> {
    use super::entities::{pipeline_runs, pipeline_tasks};
    let mut ids = pipeline_tasks::Model::run_ids_for_task(db, task_id).await?;
    if let Some(origin) = super::pipeline::PipelineContext::current() {
        ids.push(origin.run_id);
    }
    ids.sort_unstable();
    ids.dedup();
    for id in ids {
        pipeline_runs::Model::lock(db, &id).await?;
    }
    Ok(())
}

/// Archive files are deleted only after the durable child barrier closes. Keeping
/// the path until removal succeeds makes cleanup restartable after a crash.
async fn cleanup_archives(db: &DatabaseConnection) -> Result<(), DbErr> {
    let batches = worker_batches::Model::cleanup_candidates(db).await?;
    for mut batch in batches {
        let Some(path) = batch.source["archive_path"].as_str() else {
            continue;
        };
        match tokio::fs::remove_file(path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                tracing::error!(batch_id=batch.id, %error, "Unable to remove completed archive");
                continue;
            }
        }
        batch
            .source
            .as_object_mut()
            .ok_or_else(|| DbErr::Custom("Invalid batch source".into()))?
            .remove("archive_path");
        batch.store_source(db).await?;
    }
    Ok(())
}
