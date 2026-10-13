//! Registered Strava operations; provider and storage behavior belongs to core.
mod backfill;
mod create;
mod delete;
mod disconnect;
mod legacy;
mod retention;
mod schedule;
mod sync;
mod update;
mod upsert;

pub use backfill::Backfill;
pub use create::CreateActivity;
pub use delete::DeleteActivity;
pub use disconnect::Disconnect;
pub use legacy::LegacyWorkflow;
pub use retention::StravaRetention;
pub use schedule::StravaReconcile;
pub use sync::StravaSync;
pub use update::UpdateActivity;
pub use upsert::UpsertActivity;

use bike_core::{
    background_jobs::worker::{TaskProcessor, TaskWorker},
    strava::workflow::{self, Action, Execution, WorkflowTask},
};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

type WorkerResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub fn processor(db: DatabaseConnection, action: Action) -> Arc<dyn TaskProcessor> {
    match action {
        Action::Create => Arc::new(CreateActivity::new(db)),
        Action::Update => Arc::new(UpdateActivity::new(db)),
        Action::Upsert => Arc::new(UpsertActivity::new(db)),
        Action::Delete => Arc::new(DeleteActivity::new(db)),
        Action::Sync => Arc::new(StravaSync::new(db)),
        Action::Backfill => Arc::new(Backfill::new(db)),
        Action::Disconnect => Arc::new(Disconnect::new(db)),
    }
}

pub(super) fn workflow_task(payload: serde_json::Value) -> WorkerResult<WorkflowTask> {
    let data = payload.get("data").unwrap_or(&payload);
    let task: WorkflowTask = serde_json::from_value(data.clone())?;
    if task.workflow_id.trim().is_empty() {
        return Err(std::io::Error::other("Missing Strava workflow ID").into());
    }
    Ok(task)
}

async fn process_workflow(
    db: &DatabaseConnection,
    payload: serde_json::Value,
    action: Action,
) -> WorkerResult<()> {
    let task = workflow_task(payload)?;
    finish_workflow(
        db,
        &task.workflow_id,
        workflow::execute_for(db, &task.workflow_id, action).await,
    )
    .await
}

async fn finish_workflow(
    db: &DatabaseConnection,
    id: &str,
    result: Result<Execution, bike_core::workflow_error::WorkflowError>,
) -> WorkerResult<()> {
    match result {
        Ok(Execution::Completed) => Ok(()),
        Ok(Execution::WaitingForChildren) => {
            workflow::wait_for_children(db, id).await?;
            Ok(())
        }
        Err(error) if error.retry_at.is_some() => {
            workflow::resume_at(db, id, error.retry_at.unwrap()).await?;
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

pub(super) fn register(mut worker: TaskWorker, db: DatabaseConnection) -> TaskWorker {
    for action in Action::ALL {
        worker = worker.register_processor(processor(db.clone(), action));
    }
    let retention = Arc::new(StravaRetention::new(db.clone()));
    worker
        .register_startup_hook(retention.clone())
        .register_processor(retention)
        .register_processor(Arc::new(StravaReconcile::new(db.clone())))
        .register_processor(Arc::new(LegacyWorkflow::new(db)))
}

#[cfg(test)]
mod tests;
