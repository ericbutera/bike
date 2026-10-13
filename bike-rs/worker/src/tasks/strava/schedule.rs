use async_trait::async_trait;
use bike_core::{background_jobs::worker::TaskProcessor, strava};
use sea_orm::DatabaseConnection;

pub struct StravaReconcile {
    db: DatabaseConnection,
}
impl StravaReconcile {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TaskProcessor for StravaReconcile {
    fn task_type(&self) -> &str {
        "strava_reconcile"
    }
    fn schedule(&self) -> Option<&str> {
        Some("0 0 */6 * * *")
    }
    async fn process(
        &self,
        _task_id: i32,
        _payload: serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let count = strava::workflow::queue_reconciliation(&self.db).await?;
        tracing::info!(
            connections = count,
            "Checked connections for native Strava reconciliation"
        );
        Ok(())
    }
}
