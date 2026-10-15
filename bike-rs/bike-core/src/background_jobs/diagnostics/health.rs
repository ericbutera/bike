use super::super::{
    entities::{background_tasks, pipeline_outputs, pipeline_runs, task_attempts},
    worker::WorkerMetrics,
};
use chrono::{DateTime, Utc};
use sea_orm::{DatabaseConnection, DbErr};

pub async fn sample(
    db: &DatabaseConnection,
    metrics: &WorkerMetrics,
    task_types: &[String],
) -> Result<(), DbErr> {
    sample_snapshots(db, metrics, task_types).await?;
    super::super::batches::reconcile(db).await?;
    super::reconcile_available(db, metrics).await?;
    for kind in [
        "activity", "fitness", "heatmap", "segments", "import", "batch",
    ] {
        let count = pipeline_outputs::Model::pending_count(db, kind).await?;
        metrics.set_pending_outputs(kind, count as i64);
    }
    for entrypoint in [
        "request",
        "scheduler",
        "worker_startup",
        "heatmap_reconciliation",
        "admin",
        "admin_rerun",
        "upload",
    ] {
        let timestamp = pipeline_runs::Model::oldest_unready(db, entrypoint).await?;
        metrics.set_unready_age(entrypoint, age(timestamp, Utc::now()));
    }
    Ok(())
}

fn age(timestamp: Option<DateTime<Utc>>, now: DateTime<Utc>) -> i64 {
    timestamp
        .map(|time| (now - time).num_seconds().max(0))
        .unwrap_or(0)
}

async fn sample_snapshots(
    db: &DatabaseConnection,
    metrics: &WorkerMetrics,
    task_types: &[String],
) -> Result<(), DbErr> {
    let now = Utc::now();
    for task_type in task_types {
        for state in [
            "queued",
            "scheduled",
            "retrying",
            "running",
            "failed",
            "completed",
        ] {
            metrics.set_state(task_type, state, 0);
        }
        for signal in ["eligible", "runtime", "heartbeat", "progress"] {
            metrics.set_age(task_type, signal, 0);
        }
    }
    for count in background_tasks::Model::state_counts(db).await? {
        if task_types.contains(&count.task_type) {
            metrics.set_state(&count.task_type, &count.state, count.count);
        }
    }
    for queued in background_tasks::Model::eligible_ages(db).await? {
        if task_types.contains(&queued.task_type) {
            metrics.set_age(&queued.task_type, "eligible", age(queued.eligible_at, now));
        }
    }
    for active in task_attempts::Model::active_ages(db).await? {
        if !task_types.contains(&active.task_type) {
            continue;
        }
        for (signal, timestamp) in [
            ("runtime", active.runtime),
            ("heartbeat", active.heartbeat),
            ("progress", active.progress),
        ] {
            metrics.set_age(&active.task_type, signal, age(timestamp, now));
        }
    }
    Ok(())
}
