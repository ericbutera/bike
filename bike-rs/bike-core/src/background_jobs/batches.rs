//! Bounded producers yield to their children; a durable barrier owns user locks.
mod reprocessing;
pub use reprocessing::reprocess_page;
mod segments;
use super::entities::{background_tasks, batch_tasks, pipeline_outputs, worker_batches};
use chrono::Utc;
use sea_orm::{
    sea_query::{Expr, OnConflict},
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QuerySelect,
    Set, TransactionSession, TransactionTrait,
};
pub use segments::{affected_segments, segments_page};
use std::future::Future;
tokio::task_local! { static CURRENT:Option<i32>; }
pub fn current() -> Option<i32> {
    CURRENT.try_with(|id| *id).ok().flatten()
}
pub async fn scope<F: Future>(id: Option<i32>, future: F) -> F::Output {
    CURRENT.scope(id, future).await
}
pub async fn record_task(
    db: &impl ConnectionTrait,
    batch_id: i32,
    task_id: i32,
) -> Result<(), DbErr> {
    worker_batches::Entity::find_by_id(batch_id)
        .lock_exclusive()
        .one(db)
        .await?
        .filter(|batch| batch.status == "running")
        .ok_or_else(|| DbErr::RecordNotFound("Open batch disappeared".into()))?;
    batch_tasks::Entity::insert(batch_tasks::ActiveModel {
        batch_id: Set(batch_id),
        task_id: Set(task_id),
    })
    .on_conflict(
        OnConflict::columns([batch_tasks::Column::BatchId, batch_tasks::Column::TaskId])
            .do_nothing()
            .to_owned(),
    )
    .exec_without_returning(db)
    .await?;
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
    worker_batches::Entity::insert(worker_batches::ActiveModel {
        id: Set(id),
        user_id: Set(user_id),
        kind: Set(kind.into()),
        source: Set(source),
        cursor: Set(0),
        sealed: Set(false),
        status: Set("running".into()),
        updated_at: Set(Utc::now()),
    })
    .on_conflict(
        OnConflict::column(worker_batches::Column::Id)
            .do_nothing()
            .to_owned(),
    )
    .exec_without_returning(&txn)
    .await?;
    let batch = worker_batches::Entity::find_by_id(id)
        .one(&txn)
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
        let stored = worker_batches::Entity::find_by_id(batch.id)
            .lock_exclusive()
            .one(&txn)
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
        worker_batches::Entity::update_many()
            .col_expr(worker_batches::Column::Cursor, Expr::value(next))
            .col_expr(worker_batches::Column::Sealed, Expr::value(sealed))
            .col_expr(worker_batches::Column::UpdatedAt, Expr::value(Utc::now()))
            .filter(worker_batches::Column::Id.eq(batch.id))
            .exec(&txn)
            .await?;
        if !sealed {
            let parent = background_tasks::Entity::find_by_id(batch.id)
                .one(&txn)
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
    let candidates = worker_batches::Entity::find()
        .filter(worker_batches::Column::Status.eq("running"))
        .limit(64)
        .all(db)
        .await?;
    for batch in candidates {
        close_batch(db, batch).await?;
    }
    cleanup_archives(db).await
}
async fn close_batch(db: &DatabaseConnection, batch: worker_batches::Model) -> Result<(), DbErr> {
    let txn = db.begin().await?;
    lock_origins(&txn, batch.id).await?;
    let Some(batch) = worker_batches::Entity::find_by_id(batch.id)
        .lock_exclusive()
        .one(&txn)
        .await?
    else {
        return Ok(());
    };
    if batch.status != "running" {
        return Ok(());
    }
    let ids = batch_tasks::Entity::find()
        .select_only()
        .column(batch_tasks::Column::TaskId)
        .filter(batch_tasks::Column::BatchId.eq(batch.id))
        .into_query();
    let tasks = background_tasks::Model::metadata_query()
        .filter(
            sea_orm::Condition::any()
                .add(background_tasks::Column::Id.eq(batch.id))
                .add(background_tasks::Column::Id.in_subquery(ids)),
        )
        .into_model::<super::entities::diagnostic_reads::TaskMetadata>()
        .all(&txn)
        .await?;
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
    let archive_failed =
        crate::archive_import::finish_queued_archive_batch(db, batch, tasks).await?;
    let failed = failed || archive_failed;
    let now = Utc::now();
    worker_batches::Entity::update_many()
        .col_expr(
            worker_batches::Column::Status,
            Expr::value(if failed { "partial" } else { "completed" }),
        )
        .col_expr(worker_batches::Column::UpdatedAt, Expr::value(now))
        .filter(worker_batches::Column::Id.eq(batch.id))
        .exec(db)
        .await?;
    let lock_source = match batch.kind.as_str() {
        "archive" => "archive_import",
        "segments" => "segment_regeneration",
        _ => "activity_reprocessing",
    };
    let lock_id = batch.source["lock_id"]
        .as_i64()
        .and_then(|id| i32::try_from(id).ok());
    crate::entities::activity_import_locks::Entity::delete_many()
        .filter(crate::entities::activity_import_locks::Column::Id.eq(lock_id))
        .filter(crate::entities::activity_import_locks::Column::UserId.eq(batch.user_id))
        .filter(crate::entities::activity_import_locks::Column::Source.eq(lock_source))
        .exec(db)
        .await?;
    if !failed {
        pipeline_outputs::Model::publish(db, "batch", batch.id, &batch.id.to_string(), now).await?;
    }
    Ok(failed)
}
/// Enqueue locks a pipeline before joining its batch. Producers and barriers use
/// the same ordered locks so a descendant enqueue cannot deadlock with closure.
pub(crate) async fn lock_origins(db: &impl ConnectionTrait, task_id: i32) -> Result<(), DbErr> {
    use super::entities::{pipeline_runs, pipeline_tasks};
    let mut ids = pipeline_tasks::Entity::find()
        .select_only()
        .column(pipeline_tasks::Column::RunId)
        .filter(pipeline_tasks::Column::TaskId.eq(task_id))
        .into_tuple::<String>()
        .all(db)
        .await?;
    if let Some(origin) = super::pipeline::PipelineContext::current() {
        ids.push(origin.run_id);
    }
    ids.sort_unstable();
    ids.dedup();
    for id in ids {
        pipeline_runs::Entity::find_by_id(id)
            .select_only()
            .column(pipeline_runs::Column::Id)
            .lock_exclusive()
            .into_tuple::<String>()
            .one(db)
            .await?;
    }
    Ok(())
}
use sea_orm::QueryTrait;

/// Archive files are deleted only after the durable child barrier closes. Keeping
/// the path until removal succeeds makes cleanup restartable after a crash.
async fn cleanup_archives(db: &DatabaseConnection) -> Result<(), DbErr> {
    let batches = worker_batches::Entity::find()
        .filter(worker_batches::Column::Kind.eq("archive"))
        .filter(worker_batches::Column::Status.is_in(["completed", "partial"]))
        .filter(Expr::cust("source ->> 'archive_path' IS NOT NULL"))
        .order_by_asc(worker_batches::Column::UpdatedAt)
        .limit(64)
        .all(db)
        .await?;
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
        worker_batches::Entity::update_many()
            .col_expr(worker_batches::Column::Source, Expr::value(batch.source))
            .filter(worker_batches::Column::Id.eq(batch.id))
            .exec(db)
            .await?;
    }
    Ok(())
}
use sea_orm::QueryOrder;
#[cfg(test)]
struct BatchFixture {
    db: DatabaseConnection,
    origin: crate::background_jobs::pipeline::PipelineContext,
    parent: background_tasks::Model,
    lock: crate::entities::activity_import_locks::Model,
    batch: worker_batches::Model,
}
#[cfg(test)]
impl BatchFixture {
    async fn create() -> Self {
        use crate::background_jobs::{
            history_tests::{database, enqueue},
            pipeline::PipelineContext,
        };
        use crate::entities::activity_import_locks;
        use sea_orm::ActiveModelTrait;
        let db = database(true).await;
        let origin = PipelineContext::received("admin", None);
        let parent = PipelineContext::scope(Some(origin.clone()), enqueue(&db)).await;
        let lock = activity_import_locks::ActiveModel {
            user_id: Set(7),
            source: Set("activity_reprocessing".into()),
            stage: Set("running".into()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
        let batch = PipelineContext::scope(
            Some(origin.clone()),
            start(
                &db,
                parent.id,
                7,
                "reprocess",
                serde_json::json!({"lock_id":lock.id}),
            ),
        )
        .await
        .unwrap();

        Self {
            db,
            origin,
            parent,
            lock,
            batch,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::background_jobs::{
        history_tests::{database, enqueue},
        pipeline::PipelineContext,
    };
    use crate::{
        entities::activity_import_locks,
        jobs::{Job, RebuildFitnessFreshnessTask},
    };
    use sea_orm::{ActiveModelTrait, PaginatorTrait};
    async fn complete(db: &DatabaseConnection, task: &background_tasks::Model) {
        task.claim(db)
            .await
            .unwrap()
            .unwrap()
            .mark_completed(db)
            .await
            .unwrap();
    }
    #[tokio::test]
    async fn failed_child_intent_rolls_back_the_page_and_cursor() {
        let BatchFixture {
            db,
            origin,
            parent,
            batch,
            ..
        } = BatchFixture::create().await;
        db.execute_unprepared("DROP TABLE worker_batch_tasks")
            .await
            .unwrap();
        let jobs = vec![Job::RebuildFitnessFreshness(RebuildFitnessFreshnessTask {
            user_id: 7,
        })];
        let result = PipelineContext::scope(
            Some(origin.for_task(parent.id)),
            schedule_page(&db, &batch, 16, true, jobs),
        )
        .await;
        assert!(result.is_err());
        assert_eq!(
            background_tasks::Entity::find().count(&db).await.unwrap(),
            1
        );
        let stored = worker_batches::Entity::find_by_id(batch.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.cursor, 0);
        assert!(!stored.sealed);
        assert_eq!(
            super::super::entities::pipeline_tasks::Entity::find()
                .count(&db)
                .await
                .unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn archive_cleanup_preserves_running_sources_and_recovers_missing_files() {
        let BatchFixture { db, batch, .. } = BatchFixture::create().await;
        let path = std::env::temp_dir().join(format!("bike-worker-{}.zip", uuid::Uuid::new_v4()));
        tokio::fs::write(&path, b"owned archive fixture")
            .await
            .unwrap();
        worker_batches::Entity::update_many()
            .col_expr(worker_batches::Column::Kind, Expr::value("archive"))
            .col_expr(
                worker_batches::Column::Source,
                Expr::value(serde_json::json!({"archive_path":path})),
            )
            .filter(worker_batches::Column::Id.eq(batch.id))
            .exec(&db)
            .await
            .unwrap();
        cleanup_archives(&db).await.unwrap();
        assert!(path.exists());
        worker_batches::Entity::update_many()
            .col_expr(worker_batches::Column::Status, Expr::value("partial"))
            .filter(worker_batches::Column::Id.eq(batch.id))
            .exec(&db)
            .await
            .unwrap();
        // A crash after removal but before metadata cleanup must be recoverable.
        tokio::fs::remove_file(&path).await.unwrap();
        cleanup_archives(&db).await.unwrap();
        cleanup_archives(&db).await.unwrap();
        let cleaned = worker_batches::Entity::find_by_id(batch.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert!(cleaned.source.get("archive_path").is_none());
        assert!(!path.exists());
    }
    #[tokio::test]
    async fn producer_yields_and_barrier_waits_for_descendants_before_releasing_its_lock() {
        let BatchFixture {
            db,
            origin,
            parent,
            lock,
            batch,
        } = BatchFixture::create().await;
        let jobs = vec![Job::RebuildFitnessFreshness(RebuildFitnessFreshnessTask {
            user_id: 7,
        })];
        PipelineContext::scope(
            Some(origin.clone().for_task(parent.id)),
            schedule_page(&db, &batch, 16, true, jobs.clone()),
        )
        .await
        .unwrap();
        // Redelivery of the same cursor cannot duplicate children.
        schedule_page(&db, &batch, 16, true, jobs).await.unwrap();
        assert_eq!(batch_tasks::Entity::find().count(&db).await.unwrap(), 1);
        complete(&db, &parent).await;
        reconcile(&db).await.unwrap();
        assert!(activity_import_locks::Entity::find_by_id(lock.id)
            .one(&db)
            .await
            .unwrap()
            .is_some());
        let child = background_tasks::Entity::find()
            .filter(background_tasks::Column::Id.ne(parent.id))
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        let active = child.claim(&db).await.unwrap().unwrap();
        let descendant = PipelineContext::scope(
            Some(origin.for_task(child.id)),
            scope(Some(batch.id), enqueue(&db)),
        )
        .await;
        active.mark_completed(&db).await.unwrap();
        reconcile(&db).await.unwrap();
        assert_eq!(
            worker_batches::Entity::find_by_id(batch.id)
                .one(&db)
                .await
                .unwrap()
                .unwrap()
                .status,
            "running"
        );
        complete(&db, &descendant).await;
        reconcile(&db).await.unwrap();
        assert_eq!(
            worker_batches::Entity::find_by_id(batch.id)
                .one(&db)
                .await
                .unwrap()
                .unwrap()
                .status,
            "completed"
        );
        assert!(activity_import_locks::Entity::find_by_id(lock.id)
            .one(&db)
            .await
            .unwrap()
            .is_none());
        assert!(pipeline_outputs::Entity::find()
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .available_at
            .is_some());
    }
    #[tokio::test]
    async fn partial_barrier_does_not_release_a_replacement_lock_or_publish_success() {
        let db = database(true).await;
        let origin = PipelineContext::received("admin", None);
        let parent = PipelineContext::scope(Some(origin.clone()), enqueue(&db)).await;
        let newer = activity_import_locks::ActiveModel {
            user_id: Set(7),
            source: Set("activity_reprocessing".into()),
            stage: Set("running".into()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
        let batch = PipelineContext::scope(
            Some(origin),
            start(
                &db,
                parent.id,
                7,
                "reprocess",
                serde_json::json!({"lock_id":newer.id+1}),
            ),
        )
        .await
        .unwrap();
        let first = parent
            .claim(&db)
            .await
            .unwrap()
            .unwrap()
            .mark_failed(&db, "source failure".into())
            .await
            .unwrap();
        first
            .claim(&db)
            .await
            .unwrap()
            .unwrap()
            .mark_failed(&db, "exhausted".into())
            .await
            .unwrap();
        reconcile(&db).await.unwrap();
        assert_eq!(
            worker_batches::Entity::find_by_id(batch.id)
                .one(&db)
                .await
                .unwrap()
                .unwrap()
                .status,
            "partial"
        );
        assert!(activity_import_locks::Entity::find_by_id(newer.id)
            .one(&db)
            .await
            .unwrap()
            .is_some());
        assert!(pipeline_outputs::Entity::find()
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .available_at
            .is_none());
    }
}
