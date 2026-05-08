use api::analytics::rebuild_segment_analytics_cache;
use api::tasks::RebuildSegmentAnalyticsTask;
use async_trait::async_trait;
use kaleido::background_jobs::worker::TaskProcessor;
use sea_orm::DatabaseConnection;
use std::error::Error;

pub struct RebuildSegmentAnalytics {
    db: DatabaseConnection,
}

impl RebuildSegmentAnalytics {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TaskProcessor for RebuildSegmentAnalytics {
    fn task_type(&self) -> &str {
        "rebuild_segment_analytics"
    }

    async fn process(
        &self,
        _task_id: i32,
        payload: serde_json::Value,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let data = payload.get("data").unwrap_or(&payload);
        let task: RebuildSegmentAnalyticsTask = serde_json::from_value(data.clone())?;

        rebuild_segment_analytics_cache(&self.db, &task.segment_ids).await?;
        Ok(())
    }
}
