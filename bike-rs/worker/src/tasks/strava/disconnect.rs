use super::{process_workflow, WorkerResult};
use async_trait::async_trait;
use bike_core::{background_jobs::worker::TaskProcessor, strava::workflow::Action};
use sea_orm::DatabaseConnection;

pub struct Disconnect {
    db: DatabaseConnection,
}

impl Disconnect {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TaskProcessor for Disconnect {
    fn task_type(&self) -> &str {
        Action::Disconnect.task_type()
    }

    async fn process(&self, _task_id: i32, payload: serde_json::Value) -> WorkerResult<()> {
        process_workflow(&self.db, payload, Action::Disconnect).await
    }
}
