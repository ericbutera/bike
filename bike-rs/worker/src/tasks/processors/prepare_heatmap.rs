use async_trait::async_trait;
use bike_core::background_jobs::worker::TaskProcessor;
use bike_core::heatmaps::{
    preparation::prepare_activity,
    projection::{PendingProjection, Projection},
};
use bike_core::jobs::adapter::{PrepareHeatmapActivity, PrepareHeatmapTask};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use std::error::Error;

pub struct PrepareHeatmap {
    db: DatabaseConnection,
}

impl PrepareHeatmap {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PrepareHeatmapPayload {
    Batch(PrepareHeatmapTask),
    LegacySingle(PrepareHeatmapActivity),
}

impl PrepareHeatmapPayload {
    fn activities(self) -> Vec<PrepareHeatmapActivity> {
        match self {
            Self::Batch(task) => task.activities,
            Self::LegacySingle(activity) => vec![activity],
        }
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
        let task: PrepareHeatmapPayload =
            serde_json::from_value(payload.get("data").unwrap_or(&payload).clone())?;
        let activities = task.activities();
        if activities.is_empty() {
            return Err(std::io::Error::other("PrepareHeatmap task contains no activities").into());
        }

        let mut failures = Vec::new();
        for activity in activities {
            let pending = PendingProjection {
                activity_id: activity.activity_id,
                generation: activity.generation,
            };
            if let Err(error) = prepare_activity(&self.db, pending).await {
                Projection::record_failure(
                    &self.db,
                    &PendingProjection {
                        activity_id: activity.activity_id,
                        generation: activity.generation,
                    },
                )
                .await?;
                failures.push(format!("{}: {error}", activity.activity_id));
            }
        }

        if failures.is_empty() {
            Ok(())
        } else {
            Err(std::io::Error::other(format!(
                "Heatmap preparation failed for {} activities: {}",
                failures.len(),
                failures.join("; ")
            ))
            .into())
        }
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
