use api::strava::process_strava_sync;
use async_trait::async_trait;
use bike_core::config::Config;
use bike_core::jobs::StravaSyncTask;
use bike_core::observability;
use kaleido::background_jobs::worker::TaskProcessor;
use sea_orm::DatabaseConnection;
use std::error::Error;
use tracing::field;
use tracing::Instrument;

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
        let span = tracing::info_span!(
            "worker.strava_sync",
            task_type = "strava_sync",
            connection_id = task.connection_id,
            trace_id = field::Empty,
            span_id = field::Empty,
        );
        observability::set_span_parent_from_carrier(&span, task.trace_context.as_ref());
        observability::record_span_trace_context(&span);

        process_strava_sync(&self.db, &self.uploads_dir, task.connection_id)
            .instrument(span)
            .await
            .map_err(|error| std::io::Error::other(error.message))?;

        Ok(())
    }
}
