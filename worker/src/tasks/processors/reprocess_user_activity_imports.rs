use api::activity_lifecycle::process_user_activity_import_reprocessing;
use api::config::Config;
use api::tasks::{ReprocessUserActivityImportsTask, TaskQueue};
use async_trait::async_trait;
use kaleido::background_jobs::worker::TaskProcessor;
use sea_orm::DatabaseConnection;
use std::error::Error;

pub struct ReprocessUserActivityImports {
    db: DatabaseConnection,
    tasks: TaskQueue,
    uploads_dir: String,
}

impl ReprocessUserActivityImports {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            tasks: TaskQueue::new(db.clone()),
            db,
            uploads_dir: Config::get().uploads_dir.clone(),
        }
    }
}

#[async_trait]
impl TaskProcessor for ReprocessUserActivityImports {
    fn task_type(&self) -> &str {
        "reprocess_user_activity_imports"
    }

    async fn process(
        &self,
        _task_id: i32,
        payload: serde_json::Value,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let data = payload.get("data").unwrap_or(&payload);
        let task: ReprocessUserActivityImportsTask = serde_json::from_value(data.clone())?;

        process_user_activity_import_reprocessing(
            &self.db,
            &self.uploads_dir,
            &self.tasks,
            task.user_id,
        )
        .await
        .map_err(|error| std::io::Error::other(error.message))?;

        Ok(())
    }
}
