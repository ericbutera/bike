use async_trait::async_trait;
use bike_core::{
    background_jobs::worker::{TaskProcessor, WorkerError},
    jobs::RegenerateActivitySegmentsTask,
};
use sea_orm::DatabaseConnection;
pub struct RegenerateActivitySegments {
    db: DatabaseConnection,
}
impl RegenerateActivitySegments {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}
#[async_trait]
impl TaskProcessor for RegenerateActivitySegments {
    fn task_type(&self) -> &str {
        "regenerate_activity_segments"
    }
    async fn process(&self, _task_id: i32, payload: serde_json::Value) -> Result<(), WorkerError> {
        let task: RegenerateActivitySegmentsTask =
            serde_json::from_value(payload.get("data").unwrap_or(&payload).clone())?;
        bike_core::segment_regeneration::process_activity_segment_regeneration(
            &self.db,
            task.activity_id,
        )
        .await?;
        Ok(())
    }
}
