use api::analytics::rebuild_fitness_freshness_cache;
use api::tasks::RebuildFitnessFreshnessTask;
use async_trait::async_trait;
use kaleido::background_jobs::worker::TaskProcessor;
use sea_orm::DatabaseConnection;
use std::error::Error;

pub struct RebuildFitnessFreshness {
    db: DatabaseConnection,
}

impl RebuildFitnessFreshness {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TaskProcessor for RebuildFitnessFreshness {
    fn task_type(&self) -> &str {
        "rebuild_fitness_freshness"
    }

    async fn process(
        &self,
        _task_id: i32,
        payload: serde_json::Value,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let data = payload.get("data").unwrap_or(&payload);
        let task: RebuildFitnessFreshnessTask = serde_json::from_value(data.clone())?;

        rebuild_fitness_freshness_cache(&self.db, task.user_id).await?;
        Ok(())
    }
}
