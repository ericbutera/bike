use crate::background_jobs::worker::task_worker::WorkerError;
use async_trait::async_trait;
use sea_orm::DatabaseConnection;

#[async_trait]
pub trait WorkerStartupHook: Send + Sync {
    fn name(&self) -> &str;

    async fn run(&self, db: &DatabaseConnection) -> Result<(), WorkerError>;
}
