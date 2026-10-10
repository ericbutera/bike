use super::{schedule_page, start, worker_batches};
use crate::{
    activity_import_lock::*,
    entities::activities,
    jobs::{Job, ReprocessActivityImportTask},
    workflow_error::WorkflowError,
};
use sea_orm::{DatabaseConnection, EntityTrait};

pub async fn reprocess_page(
    db: &DatabaseConnection,
    task_id: i32,
    user_id: i32,
    archive_only: bool,
    batch_id: Option<i32>,
) -> Result<(), WorkflowError> {
    let id = batch_id.unwrap_or(task_id);
    let batch = match worker_batches::Entity::find_by_id(id).one(db).await? {
        Some(batch) => batch,
        None => {
            let lock = ensure_user_activity_import_lock_stage(
                db,
                user_id,
                ACTIVITY_IMPORT_LOCK_SOURCE_ACTIVITY_REPROCESSING,
                ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
            )
            .await?;
            start(
                db,
                id,
                user_id,
                "reprocess",
                serde_json::json!({"archive_only":archive_only,"lock_id":lock.id}),
            )
            .await?
        }
    };
    if batch.sealed || batch.status != "running" {
        return Ok(());
    }
    let ids = activities::Model::imported_ids_page(db, user_id, batch.cursor, archive_only).await?;
    let next = ids.last().copied().unwrap_or(batch.cursor);
    let sealed = ids.len() < 16;
    let jobs = ids
        .into_iter()
        .map(|activity_id| {
            Job::ReprocessActivityImport(ReprocessActivityImportTask { activity_id })
        })
        .collect();
    schedule_page(db, &batch, next, sealed, jobs).await?;
    Ok(())
}
