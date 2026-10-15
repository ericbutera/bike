//! Bounded diagnostics over the shared durable queue. Domain source data is never copied here.
mod graph;
mod health;
mod statistics;
#[cfg(test)]
mod statistics_tests;
pub use graph::{GraphCursor, PipelineGraph, PipelinePage};
pub use health::sample;
pub use statistics::{Distribution, ProcessorSummary};

use super::{
    entities::{background_tasks, pipeline_outputs, pipeline_runs, pipeline_tasks},
    worker::WorkerMetrics,
};
use chrono::Utc;
use sea_orm::{DatabaseConnection, DbErr, TransactionTrait};

/// Reconcile only closed task trees with explicit, revision-matched output evidence.
/// CAS ensures a publication is counted once across replicas, using stored timestamps.
pub async fn reconcile_available(
    db: &DatabaseConnection,
    metrics: &WorkerMetrics,
) -> Result<(), DbErr> {
    let candidates = pipeline_runs::Model::ready_candidates(db).await?;
    for run in candidates {
        close_ready_run(db, metrics, run).await?;
    }
    Ok(())
}

async fn close_ready_run(
    db: &DatabaseConnection,
    metrics: &WorkerMetrics,
    run: pipeline_runs::Model,
) -> Result<(), DbErr> {
    let transaction = db.begin().await?;
    let Some(run) = pipeline_runs::Model::lock(&transaction, &run.id).await? else {
        return Ok(());
    };
    if run.available_at.is_some() {
        return Ok(());
    }
    let outputs = pipeline_outputs::Model::for_run(&transaction, &run.id).await?;
    if outputs.is_empty()
        || outputs
            .iter()
            .any(|output| output.required && output.available_at.is_none())
    {
        return Ok(());
    }
    let links = pipeline_tasks::Model::task_ids(&transaction, &run.id).await?;
    let unfinished = background_tasks::Model::unfinished_in(&transaction, links).await?;
    if unfinished {
        return Ok(());
    }
    let Some(available_at) = outputs
        .iter()
        .filter_map(|output| output.available_at)
        .max()
    else {
        return Ok(());
    };
    let changed = run.make_available(&transaction, available_at).await?;
    if changed {
        transaction.commit().await?;
        metrics.record_available(
            &run.entrypoint,
            (available_at - run.pipeline_started_at).num_milliseconds() as f64 / 1000.0,
        );
    }
    Ok(())
}

/// Closed diagnostics expire after 30 days. Accepted but unready runs, active
/// tasks and authoritative import artifacts/stages are deliberately retained.
pub async fn retain(db: &DatabaseConnection) -> Result<(), DbErr> {
    let cutoff = Utc::now() - chrono::Duration::days(30);
    prune_roots(db, cutoff).await?;
    prune_attempts(db, cutoff).await
}
async fn prune_roots(db: &DatabaseConnection, cutoff: chrono::DateTime<Utc>) -> Result<(), DbErr> {
    let roots = pipeline_runs::Model::expired_roots(db, cutoff).await?;
    let transaction = db.begin().await?;
    super::entities::pipeline_tasks::Model::delete_for_runs(&transaction, &roots).await?;
    super::entities::pipeline_outputs::Model::delete_for_runs(&transaction, &roots).await?;
    super::entities::pipeline_subjects::Model::delete_for_runs(&transaction, &roots).await?;
    super::entities::pipeline_runs::Model::delete_for_runs(&transaction, &roots).await?;
    transaction.commit().await
}
async fn prune_attempts(
    db: &DatabaseConnection,
    cutoff: chrono::DateTime<Utc>,
) -> Result<(), DbErr> {
    let closed = background_tasks::Model::retired_history_tasks(db, cutoff).await?;
    super::entities::work_units::Model::delete_for_tasks(db, &closed).await?;
    super::entities::task_attempts::Model::delete_for_tasks(db, &closed).await?;
    super::entities::task_anomalies::Model::delete_for_tasks(db, &closed).await?;
    Ok(())
}

#[cfg(test)]
mod tests;

pub async fn ready_count(db: &DatabaseConnection) -> Result<u64, DbErr> {
    background_tasks::Model::ready_count(db).await
}
