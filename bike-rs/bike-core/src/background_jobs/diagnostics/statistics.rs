use super::super::entities::{background_tasks, processor_registry};
use chrono::Utc;
use sea_orm::{DatabaseConnection, DbBackend, DbErr, EntityTrait, FromQueryResult, Statement};
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
        Ok(processor_registry::Entity::find()
            .all(db)
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
#[derive(FromQueryResult)]
struct ProcessorDistribution {
    task_type: String,
    samples: i64,
    p50_seconds: Option<f64>,
    p90_seconds: Option<f64>,
}

/// Ordered-set aggregates cover the full selected population, grouped in one query.
async fn distributions(
    db: &DatabaseConnection,
    hours: i64,
    outcome: &str,
    signal: &str,
) -> Result<HashMap<String, Distribution>, DbErr> {
    if db.get_database_backend() != DbBackend::Postgres {
        return Ok(HashMap::new());
    }
    let (expression, table, time, condition) = match signal {
        "logical" => (
            "EXTRACT(EPOCH FROM (t.completed_at-t.created_at))",
            "background_tasks t",
            "t.completed_at",
            "t.status='completed'",
        ),
        "wait" => (
            "EXTRACT(EPOCH FROM (a.started_at-a.eligible_at))",
            "background_task_attempts a JOIN background_tasks t ON t.id=a.task_id",
            "a.finished_at",
            "a.eligible_at IS NOT NULL AND a.outcome=$2",
        ),
        _ => (
            "EXTRACT(EPOCH FROM (a.finished_at-a.started_at))",
            "background_task_attempts a JOIN background_tasks t ON t.id=a.task_id",
            "a.finished_at",
            "a.outcome=$2",
        ),
    };
    let sql = format!("SELECT task_type, COUNT(*)::bigint samples, percentile_cont(0.5) WITHIN GROUP (ORDER BY duration) p50_seconds, percentile_cont(0.9) WITHIN GROUP (ORDER BY duration) p90_seconds FROM (SELECT t.task_type, {expression} duration FROM {table} WHERE {time}>=$1 AND {time}<=CURRENT_TIMESTAMP AND {condition}) durations WHERE duration>=0 GROUP BY task_type");
    let mut values = vec![(Utc::now() - chrono::Duration::hours(hours)).into()];
    if signal != "logical" {
        values.push(outcome.into());
    }
    let rows = ProcessorDistribution::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        sql,
        values,
    ))
    .all(db)
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
