use async_trait::async_trait;
use bike_core::activity_lifecycle::process_user_activity_import_reprocessing;
use bike_core::background_jobs::worker::TaskProcessor;
use bike_core::config::Config;
use bike_core::jobs::{JobQueue, ReprocessUserActivityImportsTask};
use sea_orm::DatabaseConnection;
use std::error::Error;

pub struct ReprocessUserActivityImports {
    db: DatabaseConnection,
    tasks: JobQueue,
    uploads_dir: String,
    archive_fits_only: bool,
}

impl ReprocessUserActivityImports {
    pub fn new(db: DatabaseConnection) -> Self {
        Self::with_scope(db, false)
    }

    pub fn archive_fits_only(db: DatabaseConnection) -> Self {
        Self::with_scope(db, true)
    }

    fn with_scope(db: DatabaseConnection, archive_fits_only: bool) -> Self {
        Self {
            tasks: JobQueue::new(db.clone()),
            db,
            uploads_dir: Config::get().uploads_dir.clone(),
            archive_fits_only,
        }
    }
}

#[async_trait]
impl TaskProcessor for ReprocessUserActivityImports {
    fn task_type(&self) -> &str {
        if self.archive_fits_only {
            "reprocess_archive_fit_activity_imports"
        } else {
            "reprocess_user_activity_imports"
        }
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
            self.archive_fits_only,
        )
        .await
        .map_err(|error| std::io::Error::other(error.message))?;

        Ok(())
    }
}
