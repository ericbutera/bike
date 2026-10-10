use async_trait::async_trait;
use bike_core::background_jobs::batches::segments_page;
use bike_core::background_jobs::worker::TaskProcessor;
use bike_core::jobs::RegenerateUserSegmentsTask;
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
        task_id: i32,
        payload: serde_json::Value,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let data = payload.get("data").unwrap_or(&payload);
        let task: RegenerateUserSegmentsTask = serde_json::from_value(data.clone())?;

        segments_page(
            &self.db,
            task_id,
            task.user_id,
            data["batch_id"]
                .as_i64()
                .and_then(|id| i32::try_from(id).ok()),
        )
        .await
        .map_err(|error| std::io::Error::other(error.message))?;

        Ok(())
    }
}
