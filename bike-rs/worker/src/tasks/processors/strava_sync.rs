use async_trait::async_trait;
use bike_core::background_jobs::worker::TaskProcessor;
use bike_core::jobs::StravaSyncTask;
use bike_core::observability;
use bike_core::{entities::strava_connections, strava_workflow};
use sea_orm::{DatabaseConnection, EntityTrait};
use std::error::Error;
use tracing::field;
use tracing::Instrument;

pub struct StravaSync {
    db: DatabaseConnection,
}

impl StravaSync {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
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

        async {
            // Historical queue records feed the native DAG instead of reviving
            // the retired direct synchronization implementation.
            if let Some(connection) = strava_connections::Entity::find_by_id(task.connection_id)
                .one(&self.db)
                .await?
            {
                strava_workflow::queue_sync(&self.db, &connection, false).await?;
            }
            Ok::<(), Box<dyn Error + Send + Sync>>(())
        }
        .instrument(span)
        .await?;

        Ok(())
    }
}
