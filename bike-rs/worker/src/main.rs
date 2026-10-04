use bike_core::background_jobs::background_tasks;
use bike_core::background_jobs::worker::{
    spawn_metrics_server, TaskWorker, WorkerConfig, WorkerConfigDefaults, WorkerMetrics,
};
use bike_core::config::Config;
use bike_core::observability;
use bike_core::provider_metrics;
use prometheus::{register_int_gauge, IntGauge};
use sea_orm::{ColumnTrait, Condition, EntityTrait, PaginatorTrait, QueryFilter};
use std::sync::Arc;
use std::time::Duration;
use worker::tasks::{register_default_processors, register_email_processors};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _observability = observability::init_observability("bike-rust-worker");
    provider_metrics::init_provider_metrics();

    let cfg = Config::init_from_env();
    let db = bike_core::db::connect_database(&cfg.database_url).await?;

    let worker_config = WorkerConfig::from_env(WorkerConfigDefaults {
        metrics_port: 9091,
        batch_size: 50,
        poll_interval_secs: 10,
    });

    let worker = TaskWorker::new(db.clone())
        .with_batch_size(worker_config.batch_size)
        .with_poll_interval(Duration::from_secs(worker_config.poll_interval_secs));

    let worker = register_email_processors(worker, cfg)?;
    let worker = register_default_processors(worker, db.clone()).await?;

    let metrics = Arc::new(WorkerMetrics::new("bike_rust_worker"));
    let queue_depth = register_int_gauge!(
        "bike_rust_worker_queue_depth",
        "Current number of pending background tasks ready to be claimed."
    )?;
    spawn_queue_depth_sampler(db.clone(), queue_depth);
    worker::tasks::spawn_heatmap_reconciliation(db.clone());
    let task_types = worker.registered_task_types();
    let task_type_refs: Vec<&str> = task_types.iter().map(String::as_str).collect();
    metrics.warmup_task_types(&task_type_refs);
    spawn_metrics_server(worker_config.metrics_port, metrics.clone());
    let worker = worker.with_metrics(metrics);

    tracing::info!("worker started");
    worker.run().await;

    Ok(())
}

fn spawn_queue_depth_sampler(db: sea_orm::DatabaseConnection, queue_depth: IntGauge) {
    tokio::spawn(async move {
        loop {
            let ready = background_tasks::Entity::find()
                .filter(background_tasks::Column::Status.eq("pending"))
                .filter(
                    Condition::any()
                        .add(background_tasks::Column::ScheduledFor.is_null())
                        .add(background_tasks::Column::ScheduledFor.lte(chrono::Utc::now())),
                )
                .count(&db)
                .await;
            match ready {
                Ok(count) => queue_depth.set(count.min(i64::MAX as u64) as i64),
                Err(error) => tracing::warn!(%error, "failed to sample worker queue depth"),
            }
            tokio::time::sleep(Duration::from_secs(15)).await;
        }
    });
}
