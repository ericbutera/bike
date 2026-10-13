use bike_core::{config::Config, observability};
use temporalio_sdk::Runtime;
use worker::temporal::{connect, worker, TemporalConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _observability = observability::init_observability("bike-temporal-worker");
    let config = TemporalConfig::from_env();
    let db = bike_core::db::connect_database(&Config::init_from_env().database_url).await?;
    let client = connect(&config).await?;
    let runtime = Runtime::from_current_tokio(Default::default())?;
    let mut worker = worker(&runtime, client, &config, db)?;
    let shutdown = worker.shutdown_handle();
    tracing::info!(address = %config.address, namespace = %config.namespace, task_queue = %config.task_queue, "Temporal Strava worker started");
    let running = worker.run();
    tokio::pin!(running);
    tokio::select! {
        result = &mut running => result?,
        result = shutdown_signal() => { result?; shutdown(); running.await?; }
    }
    Ok(())
}

async fn shutdown_signal() -> Result<(), std::io::Error> {
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tokio::select! {
        result = tokio::signal::ctrl_c() => result,
        _ = terminate.recv() => Ok(()),
    }
}
