use api::activity_import_pipeline::recover_abandoned_manual_activity_imports_after_worker_start;
use api::tasks::TaskQueue;
use async_trait::async_trait;
use chrono::Utc;
use kaleido::background_jobs::worker::{WorkerError, WorkerStartupHook};
use sea_orm::DatabaseConnection;

pub struct RecoverManualActivityImportsOnStartup;

#[async_trait]
impl WorkerStartupHook for RecoverManualActivityImportsOnStartup {
    fn name(&self) -> &str {
        "recover_manual_activity_imports"
    }

    async fn run(&self, db: &DatabaseConnection) -> Result<(), WorkerError> {
        let task_queue = TaskQueue::new(db.clone());
        let recovered_count = recover_abandoned_manual_activity_imports_after_worker_start(
            db,
            &task_queue,
            Utc::now(),
        )
        .await
        .map_err(|error| std::io::Error::other(error.message))?;

        if recovered_count > 0 {
            tracing::warn!(
                recovered_count,
                "requeued manual activity imports abandoned by a previous worker"
            );
        }

        Ok(())
    }
}
