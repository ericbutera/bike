use api::archive_import::process_activity_archive_import_job;
use api::config::Config;
use api::tasks::ActivityArchiveImportTask;
use async_trait::async_trait;
use kaleido::background_jobs::worker::TaskProcessor;
use sea_orm::DatabaseConnection;
use std::error::Error;

pub struct ActivityArchiveImport {
    db: DatabaseConnection,
    uploads_dir: String,
}

impl ActivityArchiveImport {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            uploads_dir: Config::get().uploads_dir.clone(),
        }
    }
}

#[async_trait]
impl TaskProcessor for ActivityArchiveImport {
    fn task_type(&self) -> &str {
        "activity_archive_import"
    }

    async fn process(
        &self,
        _task_id: i32,
        payload: serde_json::Value,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let data = payload.get("data").unwrap_or(&payload);
        let task: ActivityArchiveImportTask = serde_json::from_value(data.clone())?;

        process_activity_archive_import_job(&self.db, &self.uploads_dir, task.job_id)
            .await
            .map_err(|error| std::io::Error::other(error.message))?;

        Ok(())
    }
}