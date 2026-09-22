use async_trait::async_trait;
use bike_core::jobs::RegenerateSegmentEffortsTask;
use bike_core::segment_regeneration::regenerate_segment_efforts;
use kaleido::background_jobs::worker::TaskProcessor;
use sea_orm::DatabaseConnection;
use std::error::Error;

pub struct RegenerateSegmentEfforts {
    db: DatabaseConnection,
}

impl RegenerateSegmentEfforts {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TaskProcessor for RegenerateSegmentEfforts {
    fn task_type(&self) -> &str {
        "regenerate_segment_efforts"
    }

    async fn process(
        &self,
        _task_id: i32,
        payload: serde_json::Value,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let data = payload.get("data").unwrap_or(&payload);
        let task: RegenerateSegmentEffortsTask = serde_json::from_value(data.clone())?;

        regenerate_segment_efforts(&self.db, task.segment_id)
            .await
            .map_err(|error| std::io::Error::other(error.message))?;

        Ok(())
    }
}
