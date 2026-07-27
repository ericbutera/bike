use api::activity_lifecycle::process_single_activity_import_reprocessing;
use api::config::Config;
use api::tasks::{ReprocessActivityImportTask, TaskQueue};
use async_trait::async_trait;
use kaleido::background_jobs::worker::TaskProcessor;
use sea_orm::DatabaseConnection;
use std::error::Error;

pub struct ReprocessActivityImport {
    db: DatabaseConnection,
    tasks: TaskQueue,
    uploads_dir: String,
}

impl ReprocessActivityImport {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            tasks: TaskQueue::new(db.clone()),
            db,
            uploads_dir: Config::get().uploads_dir.clone(),
        }
    }
}

#[async_trait]
impl TaskProcessor for ReprocessActivityImport {
    fn task_type(&self) -> &str {
        "reprocess_activity_import"
    }

    async fn process(
        &self,
        _task_id: i32,
        payload: serde_json::Value,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let data = payload.get("data").unwrap_or(&payload);
        let task: ReprocessActivityImportTask = serde_json::from_value(data.clone())?;

        process_single_activity_import_reprocessing(
            &self.db,
            &self.uploads_dir,
            &self.tasks,
            task.activity_id,
        )
        .await
        .map_err(|error| std::io::Error::other(error.message))?;

        Ok(())
    }
}
