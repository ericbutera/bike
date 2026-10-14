use bike_core::{config::Config, observability};
use std::future::{Future, IntoFuture};
use strava_temporal::client::Error;
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
    let (options, _metrics_server) = strava_temporal::metrics::start(sdk_address)?;
    let runtime = Runtime::from_current_tokio(options)?;
    let listener = tokio::net::TcpListener::bind("0.0.0.0:9091").await?;
    let metrics = axum::serve(
        listener,
        axum::Router::new().route("/metrics", axum::routing::get(provider_metrics)),
    )
    .into_future();
    let events = dispatch_events(client.clone(), config.clone(), db.clone());
    let mut worker = worker(&runtime, client, &config, db)?;
    let shutdown = worker.shutdown_handle();
    tracing::info!(
        temporal_namespace = config.namespace,
        task_queue = config.task_queue,
        "Native Strava Temporal worker started"
    );
    let run = async { worker.run().await.map_err(Into::into) };
    supervise(run, events, metrics, shutdown_signal(), shutdown).await
}

async fn supervise(
    run: impl Future<Output = Result<(), Error>>,
    events: impl Future<Output = Result<(), Error>>,
    metrics: impl Future<Output = Result<(), std::io::Error>>,
    signal: impl Future<Output = Result<(), std::io::Error>>,
    shutdown: impl FnOnce(),
) -> Result<(), Error> {
    tokio::pin!(run);
    tokio::pin!(events);
    tokio::pin!(metrics);
    tokio::pin!(signal);
    tokio::select! {
        result = &mut run => result?,
        result = &mut events => { shutdown(); run.await?; result?; }
        result = &mut signal => { shutdown(); run.await?; result?; }
        result = &mut metrics => { shutdown(); run.await?; result?; }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::pending;

    #[tokio::test]
    async fn worker_completion_and_failure_end_the_process() {
        supervise(async { Ok(()) }, pending(), pending(), pending(), || {
            panic!("Worker already stopped")
        })
        .await
        .unwrap();
        let error = supervise(
            async { Err(std::io::Error::other("worker failed").into()) },
            pending(),
            pending(),
            pending(),
            || panic!("Worker already stopped"),
        )
        .await
        .unwrap_err();
        assert_eq!(error.to_string(), "worker failed");
    }

    #[tokio::test]
    async fn handoff_termination_drains_worker_and_preserves_error() {
        for result in [Ok(()), Err(std::io::Error::other("handoff failed").into())] {
            let failed = result.is_err();
            let (send, receive) = tokio::sync::oneshot::channel();
            let finished = supervise(
                async {
                    receive.await.unwrap();
                    Ok(())
                },
                async { result },
                pending(),
                pending(),
                || {
                    send.send(()).unwrap();
                },
            )
            .await;
            assert_eq!(finished.is_err(), failed);
        }
    }

    #[tokio::test]
    async fn listener_or_signal_failure_still_drains_worker() {
        for listener in [true, false] {
            let (send, receive) = tokio::sync::oneshot::channel();
            let metrics = async {
                if listener {
                    Err(std::io::Error::other("listener failed"))
                } else {
                    pending().await
                }
            };
            let signal = async {
                if listener {
                    pending().await
                } else {
                    Err(std::io::Error::other("signal failed"))
                }
            };
            let result = supervise(
                async {
                    receive.await.unwrap();
                    Ok(())
                },
                pending(),
                metrics,
                signal,
                || {
                    send.send(()).unwrap();
                },
            )
            .await;
            assert!(result.is_err());
        }
    }

    #[tokio::test]
    async fn normal_shutdown_waits_for_worker_to_finish() {
        let (send, receive) = tokio::sync::oneshot::channel();
        supervise(
            async {
                receive.await.unwrap();
                Ok(())
            },
            pending(),
            pending(),
            async { Ok(()) },
            || {
                send.send(()).unwrap();
            },
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn provider_endpoint_encodes_the_registered_metrics() {
        bike_core::provider_metrics::init_provider_metrics();
        bike_core::provider_metrics::record_provider_api_request(
            "strava", "activity", "read", "success",
        );
        let (headers, body) = provider_metrics().await.unwrap();
        assert_eq!(headers[0].0, axum::http::header::CONTENT_TYPE);
        assert!(body.contains("bike_rust_provider_api_requests_total"));
    }
}
