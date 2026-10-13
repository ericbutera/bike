use bike_core::{config::Config, observability};
use std::future::IntoFuture;
use strava_temporal::client::{connect, dispatch_events, worker, TemporalConfig};
use temporalio_sdk::Runtime;

#[tokio::main]
async fn main() -> Result<(), strava_temporal::client::Error> {
    let _observability = observability::init_observability("bike-strava-temporal");
    let config = TemporalConfig::from_env();
    let db = bike_core::db::connect_database(&Config::init_from_env().database_url).await?;
    let client = connect(&config).await?;
    strava_temporal::client::ensure_schedule(&client, &config).await?;
    bike_core::provider_metrics::init_provider_metrics();
    let sdk_address = std::env::var("TEMPORAL_METRICS_ADDRESS")
        .unwrap_or_else(|_| "0.0.0.0:9092".into())
        .parse()?;
    let (options, metrics_server) = strava_temporal::metrics::start(sdk_address)?;
    let runtime = Runtime::from_current_tokio(options)?;
    let listener = tokio::net::TcpListener::bind("0.0.0.0:9091").await?;
    let metrics = axum::serve(
        listener,
        axum::Router::new().route("/metrics", axum::routing::get(provider_metrics)),
    )
    .into_future();
    tokio::pin!(metrics);
    let events = dispatch_events(client.clone(), config.clone(), db.clone());
    tokio::pin!(events);
    let mut worker = worker(&runtime, client, &config, db)?;
    let shutdown = worker.shutdown_handle();
    tracing::info!(
        temporal_namespace = config.namespace,
        task_queue = config.task_queue,
        "Native Strava Temporal worker started"
    );
    let run = worker.run();
    tokio::pin!(run);
    tokio::select! {
        result = &mut run => result?,
        result = &mut events => { shutdown(); run.await?; result?; }
        result = shutdown_signal() => { result?; shutdown(); run.await?; }
        result = &mut metrics => { shutdown(); run.await?; result?; }
    }
    metrics_server.abort_handle.abort();
    Ok(())
}

async fn provider_metrics(
) -> Result<([(axum::http::HeaderName, &'static str); 1], String), axum::http::StatusCode> {
    let mut body = String::new();
    prometheus::TextEncoder::new()
        .encode_utf8(&prometheus::gather(), &mut body)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4",
        )],
        body,
    ))
}

async fn shutdown_signal() -> Result<(), std::io::Error> {
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tokio::select! { result = tokio::signal::ctrl_c() => result, _ = terminate.recv() => Ok(()) }
}
