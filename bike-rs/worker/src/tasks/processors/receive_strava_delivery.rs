use async_trait::async_trait;
use bike_core::{
    background_jobs::worker::{TaskProcessor, WorkerError},
    config::Config,
    jobs::JobQueue,
    strava_gateway_delivery::intent,
};
use sea_orm::DatabaseConnection;
pub struct ReceiveStravaDelivery {
    db: DatabaseConnection,
}
impl ReceiveStravaDelivery {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}
#[async_trait]
impl TaskProcessor for ReceiveStravaDelivery {
    fn task_type(&self) -> &str {
        "receive_strava_delivery"
    }
    async fn process(&self, _task_id: i32, payload: serde_json::Value) -> Result<(), WorkerError> {
        let id = payload["data"]["delivery_id"]
            .as_str()
            .ok_or_else(|| std::io::Error::other("Missing delivery ID"))?;
        intent::execute(
            &self.db,
            &JobQueue::new(self.db.clone()),
            &Config::get().uploads_dir,
            id,
        )
        .await
        .map_err(|error| std::io::Error::other(error.message).into())
    }
}
