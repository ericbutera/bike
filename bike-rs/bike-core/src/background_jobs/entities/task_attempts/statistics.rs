use super::*;
use chrono::Utc;
use sea_orm::{DbBackend, FromQueryResult, Statement};
#[derive(FromQueryResult)]
pub struct ProcessorDistribution {
    pub task_type: String,
    pub samples: i64,
    pub p50_seconds: Option<f64>,
    pub p90_seconds: Option<f64>,
}
impl Model {
    pub async fn processor_distributions(
        db: &DatabaseConnection,
        hours: i64,
        outcome: &str,
        signal: &str,
    ) -> Result<Vec<ProcessorDistribution>, DbErr> {
        if db.get_database_backend() != DbBackend::Postgres {
            return Ok(Vec::new());
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
        Ok(rows)
    }
}
