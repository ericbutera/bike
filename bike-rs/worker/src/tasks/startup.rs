use async_trait::async_trait;
use bike_core::activity_import_recovery::recover_abandoned_activity_imports_after_worker_start;
use bike_core::background_jobs::worker::{WorkerError, WorkerStartupHook};
use bike_core::jobs::JobQueue;
use chrono::Utc;
use sea_orm::DatabaseConnection;

pub struct RecoverActivityImportsOnStartup;

#[async_trait]
impl WorkerStartupHook for RecoverActivityImportsOnStartup {
    fn name(&self) -> &str {
        "recover_activity_imports"
    }

    async fn run(&self, db: &DatabaseConnection) -> Result<(), WorkerError> {
        let task_queue = JobQueue::new(db.clone());
        let recovered_count =
            recover_abandoned_activity_imports_after_worker_start(db, &task_queue, Utc::now())
                .await
                .map_err(|error| std::io::Error::other(error.message))?;

        if recovered_count > 0 {
            tracing::warn!(
                recovered_count,
                "requeued activity imports abandoned by a previous worker"
            );
        }

        Ok(())
    }
}
