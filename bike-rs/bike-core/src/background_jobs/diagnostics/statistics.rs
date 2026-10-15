use super::super::entities::{background_tasks, processor_registry};
use chrono::Utc;
use sea_orm::{DatabaseConnection, DbErr, FromQueryResult};
use serde::Serialize;
use std::collections::HashMap;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema, FromQueryResult, Default)]
pub struct Distribution {
    pub samples: i64,
    pub p50_seconds: Option<f64>,
    pub p90_seconds: Option<f64>,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct ProcessorSummary {
    pub task_type: String,
    pub registered_at: chrono::DateTime<Utc>,
    pub queued: i64,
    pub running: i64,
    pub scheduled: i64,
    pub failed: i64,
    pub retrying: i64,
    pub attempt: Distribution,
    pub eligible_wait: Distribution,
    pub logical_completion: Distribution,
    pub window_hours: i64,
    pub outcome: String,
}
impl ProcessorSummary {
    pub async fn list(
        db: &DatabaseConnection,
        window_hours: i64,
        outcome: &str,
    ) -> Result<Vec<Self>, DbErr> {
        let counts = background_tasks::Model::state_counts(db).await?;
        let mut attempts = distributions(db, window_hours, outcome, "attempt").await?;
        let mut waits = distributions(db, window_hours, outcome, "wait").await?;
        let mut logical = distributions(db, window_hours, "completed", "logical").await?;
        Ok(processor_registry::Model::registered(db)
            .await?
            .into_iter()
            .map(|processor| {
                let count = |state: &str| {
                    counts
                        .iter()
                        .filter(|row| row.task_type == processor.task_type && row.state == state)
                        .map(|row| row.count)
                        .sum()
                };
                Self {
                    queued: count("queued"),
                    running: count("running"),
                    scheduled: count("scheduled"),
                    failed: count("failed"),
                    retrying: count("retrying"),
                    attempt: attempts.remove(&processor.task_type).unwrap_or_default(),
                    eligible_wait: waits.remove(&processor.task_type).unwrap_or_default(),
                    logical_completion: logical.remove(&processor.task_type).unwrap_or_default(),
                    task_type: processor.task_type,
                    registered_at: processor.observed_at,
                    window_hours,
                    outcome: outcome.into(),
                }
            })
            .collect())
    }
}

/// Ordered-set aggregates cover the full selected population, grouped in one query.
async fn distributions(
    db: &DatabaseConnection,
    hours: i64,
    outcome: &str,
    signal: &str,
) -> Result<HashMap<String, Distribution>, DbErr> {
    let rows = super::super::entities::task_attempts::Model::processor_distributions(
        db, hours, outcome, signal,
    )
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.task_type,
                Distribution {
                    samples: row.samples,
                    p50_seconds: row.p50_seconds,
                    p90_seconds: row.p90_seconds,
                },
            )
        })
        .collect())
}
