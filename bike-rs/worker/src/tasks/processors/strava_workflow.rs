use async_trait::async_trait;
use bike_core::{background_jobs::worker::TaskProcessor, strava_workflow};
use sea_orm::DatabaseConnection;

pub struct StravaWorkflow {
    db: DatabaseConnection,
}
impl StravaWorkflow {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}
#[async_trait]
impl TaskProcessor for StravaWorkflow {
    fn task_type(&self) -> &str {
        "strava_workflow"
    }
    async fn process(
        &self,
        _task_id: i32,
        payload: serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let data = payload.get("data").unwrap_or(&payload);
        let id = data
            .get("workflow_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| std::io::Error::other("Missing Strava workflow ID"))?;
        if let Err(error) = strava_workflow::execute(&self.db, id).await {
            if let Some(at) = error.retry_at {
                strava_workflow::resume_at(&self.db, id, at).await?;
            } else {
                return Err(error.into());
            }
        }
        Ok(())
    }
}
