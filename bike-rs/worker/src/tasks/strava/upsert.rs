use super::{process_workflow, WorkerResult};
use async_trait::async_trait;
use bike_core::{background_jobs::worker::TaskProcessor, strava::workflow::Action};
use sea_orm::DatabaseConnection;

pub struct UpsertActivity {
    db: DatabaseConnection,
}

impl UpsertActivity {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TaskProcessor for UpsertActivity {
    fn task_type(&self) -> &str {
        Action::Upsert.task_type()
    }

    async fn process(&self, _task_id: i32, payload: serde_json::Value) -> WorkerResult<()> {
        process_workflow(&self.db, payload, Action::Upsert).await
    }
}
