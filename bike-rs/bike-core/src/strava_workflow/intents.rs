//! Derived-job enqueue and its stage checkpoint commit together.
use crate::{
    background_jobs::{background_tasks, TaskError},
    entities::strava_workflows,
    strava_objects::checksum,
};
use sea_orm::{EntityTrait, TransactionTrait};

tokio::task_local! { static WORKFLOW: String; }
pub(crate) async fn scope<F: std::future::Future>(id: &str, future: F) -> F::Output {
    WORKFLOW.scope(id.to_owned(), future).await
}

pub(crate) async fn enqueue(
    db: &sea_orm::DatabaseConnection,
    task_type: &str,
    payload: &serde_json::Value,
) -> Result<bool, TaskError> {
    let Ok(id) = WORKFLOW.try_with(Clone::clone) else {
        return Ok(false);
    };
    let key = format!(
        "job:{task_type}:{}",
        checksum(&serde_json::to_vec(payload)?)
    );
    let txn = db.begin().await.map_err(storage_error)?;
    let row = strava_workflows::Entity::find_by_id(id)
        .one(&txn)
        .await
        .map_err(storage_error)?
        .ok_or(TaskError::NotFound)?;
    if row.checkpoint.get(&key).is_none() {
        let mut correlated = payload.clone();
        correlated["strava"] = row.worker_context();
        let task = background_tasks::Model::enqueue(&txn, task_type.into(), correlated, None, 5)
            .await
            .map_err(storage_error)?;
        let mut checkpoint = row.checkpoint.clone();
        checkpoint[&key] =
            serde_json::json!({"task_id":task.id,"strava_activity_id":row.activity_id});
        row.checkpoint(&txn, checkpoint)
            .await
            .map_err(storage_error)?;
    }
    txn.commit().await.map_err(storage_error)?;
    Ok(true)
}
fn storage_error(error: sea_orm::DbErr) -> TaskError {
    TaskError::Storage(error.to_string())
}
