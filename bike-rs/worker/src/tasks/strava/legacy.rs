//! Consume historical queue records without producing more generic workflows.
use super::{finish_workflow, workflow_task, WorkerResult};
use async_trait::async_trait;
use bike_core::{
    background_jobs::worker::TaskProcessor, entities::strava_connections, jobs::StravaSyncTask,
    strava::workflow,
};
use sea_orm::{DatabaseConnection, EntityTrait};
use tracing::Instrument;

pub struct LegacyWorkflow {
    db: DatabaseConnection,
}
impl LegacyWorkflow {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TaskProcessor for LegacyWorkflow {
    fn task_type(&self) -> &str {
        "strava_workflow"
    }
    async fn process(&self, _task_id: i32, payload: serde_json::Value) -> WorkerResult<()> {
        let task = workflow_task(payload)?;
        finish_workflow(
            &self.db,
            &task.workflow_id,
            workflow::execute(&self.db, &task.workflow_id).await,
        )
        .await
    }
}

pub(super) async fn queue_connection_sync(
    db: &DatabaseConnection,
    data: &serde_json::Value,
) -> WorkerResult<()> {
    let task: StravaSyncTask = serde_json::from_value(data.clone())?;
    let span = tracing::info_span!(
        "worker.strava_sync.legacy",
        connection_id = task.connection_id,
        trace_id = tracing::field::Empty,
        span_id = tracing::field::Empty
    );
    bike_core::observability::set_span_parent_from_carrier(&span, task.trace_context.as_ref());
    bike_core::observability::record_span_trace_context(&span);
    async {
        if let Some(connection) = strava_connections::Entity::find_by_id(task.connection_id)
            .one(db)
            .await?
        {
            workflow::queue_sync(db, &connection, false).await?;
        }
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    }
    .instrument(span)
    .await
}
