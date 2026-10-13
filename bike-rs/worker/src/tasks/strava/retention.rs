use async_trait::async_trait;
use bike_core::{
    background_jobs::{
        worker::{TaskProcessor, WorkerError, WorkerStartupHook},
        DurableStorage, TaskQueue,
    },
    config::Config,
    strava,
};
use sea_orm::DatabaseConnection;

pub struct StravaRetention {
    db: DatabaseConnection,
}
impl StravaRetention {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TaskProcessor for StravaRetention {
    fn task_type(&self) -> &str {
        "strava_retention"
    }
    fn schedule(&self) -> Option<&str> {
        Some("0 0 * * * *")
    }
    async fn process(
        &self,
        _task_id: i32,
        _payload: serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        strava::retention::run(&self.db, &Config::get().uploads_dir, chrono::Utc::now()).await?;
        Ok(())
    }
}

#[async_trait]
impl WorkerStartupHook for StravaRetention {
    fn name(&self) -> &str {
        "strava_retention"
    }
    async fn run(&self, db: &DatabaseConnection) -> Result<(), WorkerError> {
        TaskQueue::new(DurableStorage::new(db.clone()))
            .enqueue("strava_retention".into(), serde_json::json!({}))
            .await?;
        Ok(())
    }
}
