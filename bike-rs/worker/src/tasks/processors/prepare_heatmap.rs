use async_trait::async_trait;
use bike_core::background_jobs::worker::TaskProcessor;
use bike_core::heatmaps::{
    preparation::prepare_activity,
    projection::{PendingProjection, Projection},
};
use bike_core::jobs::adapter::PrepareHeatmapTask;
use sea_orm::DatabaseConnection;
use std::error::Error;

pub struct PrepareHeatmap {
    db: DatabaseConnection,
}

impl PrepareHeatmap {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TaskProcessor for PrepareHeatmap {
    fn task_type(&self) -> &str {
        "prepare_heatmap"
    }

    async fn process(
        &self,
        _task_id: i32,
        payload: serde_json::Value,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let task: PrepareHeatmapTask =
            serde_json::from_value(payload.get("data").unwrap_or(&payload).clone())?;
        let pending = PendingProjection {
            activity_id: task.activity_id,
            generation: task.generation,
        };
        if let Err(error) = prepare_activity(&self.db, pending).await {
            Projection::record_failure(
                &self.db,
                &PendingProjection {
                    activity_id: task.activity_id,
                    generation: task.generation,
                },
            )
            .await?;
            return Err(error.into());
        }
        Ok(())
    }
}

/// The projection table is the outbox. Restarting this loop never scans route
/// JSON, and queueing is atomic with leasing each pending generation.
pub fn spawn_heatmap_reconciliation(db: DatabaseConnection) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));
        loop {
            interval.tick().await;
            let result = async {
                if Projection::enabled(&db).await? {
                    Projection::enqueue_pending(&db).await?;
                }
                Ok::<_, sea_orm::DbErr>(())
            }
            .await;
            if let Err(error) = result {
                tracing::warn!(%error, "heatmap reconciliation failed");
            }
        }
    });
}
