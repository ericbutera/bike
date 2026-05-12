use api::activity_lifecycle::process_user_segment_regeneration;
use api::tasks::RegenerateUserSegmentsTask;
use async_trait::async_trait;
use kaleido::background_jobs::worker::TaskProcessor;
use sea_orm::DatabaseConnection;
use std::error::Error;

pub struct RegenerateUserSegments {
    db: DatabaseConnection,
}

impl RegenerateUserSegments {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TaskProcessor for RegenerateUserSegments {
    fn task_type(&self) -> &str {
        "regenerate_user_segments"
    }

    async fn process(
        &self,
        _task_id: i32,
        payload: serde_json::Value,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let data = payload.get("data").unwrap_or(&payload);
        let task: RegenerateUserSegmentsTask = serde_json::from_value(data.clone())?;

        process_user_segment_regeneration(&self.db, task.user_id)
            .await
            .map_err(|error| std::io::Error::other(error.message))?;

        Ok(())
    }
}
