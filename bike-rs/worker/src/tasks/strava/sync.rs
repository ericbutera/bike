use super::{legacy, process_workflow, WorkerResult};
use async_trait::async_trait;
use bike_core::{background_jobs::worker::TaskProcessor, strava::workflow::Action};
use sea_orm::DatabaseConnection;

pub struct StravaSync {
    db: DatabaseConnection,
}
impl StravaSync {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TaskProcessor for StravaSync {
    fn task_type(&self) -> &str {
        Action::Sync.task_type()
    }
    async fn process(&self, _task_id: i32, payload: serde_json::Value) -> WorkerResult<()> {
        let data = payload.get("data").unwrap_or(&payload);
        if data.get("workflow_id").is_none() {
            return legacy::queue_connection_sync(&self.db, data).await;
        }
        process_workflow(&self.db, payload, Action::Sync).await
    }
}
