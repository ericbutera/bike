use api::config::Config;
use api::strava::process_strava_sync;
use api::tasks::StravaSyncTask;
use async_trait::async_trait;
use kaleido::background_jobs::worker::TaskProcessor;
use sea_orm::DatabaseConnection;
use std::error::Error;

pub struct StravaSync {
    db: DatabaseConnection,
    uploads_dir: String,
}

impl StravaSync {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            uploads_dir: Config::get().uploads_dir.clone(),
        }
    }
}

#[async_trait]
impl TaskProcessor for StravaSync {
    fn task_type(&self) -> &str {
        "strava_sync"
    }

    async fn process(
        &self,
        _task_id: i32,
        payload: serde_json::Value,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let data = payload.get("data").unwrap_or(&payload);
        let task: StravaSyncTask = serde_json::from_value(data.clone())?;

        process_strava_sync(&self.db, &self.uploads_dir, task.connection_id)
            .await
            .map_err(|error| std::io::Error::other(error.message))?;

        Ok(())
    }
}
