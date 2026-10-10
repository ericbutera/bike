use super::{schedule_page, scope, start, worker_batches};
use crate::{
    activity_import_lock::*,
    entities::activities,
    jobs::{Job, RebuildSegmentAnalyticsTask, RegenerateActivitySegmentsTask},
    workflow_error::WorkflowError,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QuerySelect,
    Set, TransactionTrait,
};
pub async fn segments_page(
    db: &DatabaseConnection,
    task_id: i32,
    user_id: i32,
    batch_id: Option<i32>,
) -> Result<(), WorkflowError> {
    let id = batch_id.unwrap_or(task_id);
    let batch = match worker_batches::Entity::find_by_id(id).one(db).await? {
        Some(batch) => batch,
        None => {
            let lock = ensure_user_activity_import_lock_stage(
                db,
                user_id,
                ACTIVITY_IMPORT_LOCK_SOURCE_SEGMENT_REGENERATION,
                ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
            )
            .await?;
            start(
                db,
                id,
                user_id,
                "segments",
                serde_json::json!({"segment_ids":[],"caches_scheduled":false,"lock_id":lock.id}),
            )
            .await?
        }
    };
    if batch.sealed || batch.status != "running" {
        return Ok(());
    }
    let ids = activities::Model::cycling_ids_page(db, user_id, batch.cursor).await?;
    let next = ids.last().copied().unwrap_or(batch.cursor);
    let sealed = ids.len() < 16;
    let jobs = ids
        .into_iter()
        .map(|activity_id| {
            Job::RegenerateActivitySegments(RegenerateActivitySegmentsTask { activity_id })
        })
        .collect();
    schedule_page(db, &batch, next, sealed, jobs).await?;
    Ok(())
}
pub async fn affected_segments(db: &DatabaseConnection, ids: &[i32]) -> Result<(), DbErr> {
    let Some(id) = super::current() else {
        return Err(DbErr::Custom("Segment batch context is missing".into()));
    };
    let txn = db.begin().await?;
    let mut batch = worker_batches::Entity::find_by_id(id)
        .lock_exclusive()
        .one(&txn)
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("Segment batch disappeared".into()))?;
    let mut all: Vec<i32> = serde_json::from_value(batch.source["segment_ids"].clone())
        .map_err(|error| DbErr::Custom(error.to_string()))?;
    all.extend_from_slice(ids);
    all.sort_unstable();
    all.dedup();
    batch.source["segment_ids"] = serde_json::json!(all);
    worker_batches::Entity::update_many()
        .col_expr(
            worker_batches::Column::Source,
            sea_orm::sea_query::Expr::value(batch.source),
        )
        .filter(worker_batches::Column::Id.eq(id))
        .exec(&txn)
        .await?;
    txn.commit().await
}
/// Aggregate caches run once after all per-activity matching children close.
pub async fn schedule_segment_caches(
    db: &(impl ConnectionTrait + TransactionTrait),
    batch: &worker_batches::Model,
) -> Result<bool, DbErr> {
    if batch.kind != "segments" || batch.source["caches_scheduled"] == true {
        return Ok(false);
    }
    let ids: Vec<i32> = serde_json::from_value(batch.source["segment_ids"].clone())
        .map_err(|error| DbErr::Custom(error.to_string()))?;
    let parent = super::background_tasks::Entity::find_by_id(batch.id)
        .one(db)
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("Batch producer disappeared".into()))?;
    let origin = crate::background_jobs::pipeline::PipelineContext::from_payload(&parent.payload)
        .map_err(|error| DbErr::Custom(error.to_string()))?
        .map(|origin| origin.for_task(batch.id));
    crate::background_jobs::pipeline::PipelineContext::scope(
        origin,
        scope(Some(batch.id), async {
            for ids in ids.chunks(16) {
                let job = Job::RebuildSegmentAnalytics(RebuildSegmentAnalyticsTask {
                    segment_ids: ids.to_vec(),
                });
                super::background_tasks::Model::enqueue(
                    db,
                    job.task_type().into(),
                    serde_json::to_value(job).map_err(|error| DbErr::Custom(error.to_string()))?,
                    None,
                    3,
                )
                .await?;
            }
            let mut source = batch.source.clone();
            source["caches_scheduled"] = true.into();
            worker_batches::Entity::update_many()
                .set(worker_batches::ActiveModel {
                    source: Set(source),
                    ..Default::default()
                })
                .filter(worker_batches::Column::Id.eq(batch.id))
                .exec(db)
                .await?;
            Ok(!ids.is_empty())
        }),
    )
    .await
}
