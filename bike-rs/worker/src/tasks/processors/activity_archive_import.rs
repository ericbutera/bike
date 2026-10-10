use async_trait::async_trait;
use bike_core::archive_import::queue_archive_page;
use bike_core::background_jobs::worker::TaskProcessor;
use bike_core::config::Config;
use bike_core::jobs::ActivityArchiveImportTask;
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
        task_id: i32,
        payload: serde_json::Value,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let data = payload.get("data").unwrap_or(&payload);
        let task: ActivityArchiveImportTask = serde_json::from_value(data.clone())?;

        queue_archive_page(
            &self.db,
            &self.uploads_dir,
            task_id,
            task.job_id,
            data["batch_id"]
                .as_i64()
                .and_then(|id| i32::try_from(id).ok()),
        )
        .await
        .map_err(|error| std::io::Error::other(error.message))?;

        Ok(())
    }
}
