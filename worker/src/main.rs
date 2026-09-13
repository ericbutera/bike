use api::config::Config;
use api::observability;
use kaleido::background_jobs::worker::{
    spawn_metrics_server, TaskWorker, WorkerConfig, WorkerConfigDefaults, WorkerMetrics,
};
use std::sync::Arc;
use std::time::Duration;
use worker::tasks::{register_auth_email_processors, register_default_processors};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _observability = observability::init_observability("bike-worker");

    let cfg = Config::init_from_env();
    let db = api::storage::connect_database(&cfg.database_url).await?;

    let worker_config = WorkerConfig::from_env(WorkerConfigDefaults {
        metrics_port: 9091,
        batch_size: 50,
        poll_interval_secs: 10,
    });

    let worker = TaskWorker::new(db.clone())
        .with_batch_size(worker_config.batch_size)
        .with_poll_interval(Duration::from_secs(worker_config.poll_interval_secs));

    let worker = register_auth_email_processors(worker, cfg)?;
    let worker = register_default_processors(worker, db.clone()).await?;

    let metrics = Arc::new(WorkerMetrics::new("bike_worker"));
    let task_types = worker.registered_task_types();
    let task_type_refs: Vec<&str> = task_types.iter().map(String::as_str).collect();
    metrics.warmup_task_types(&task_type_refs);
    spawn_metrics_server(worker_config.metrics_port, metrics.clone());
    let worker = worker.with_metrics(metrics);

    tracing::info!("worker started");
    worker.run().await;

    Ok(())
}
