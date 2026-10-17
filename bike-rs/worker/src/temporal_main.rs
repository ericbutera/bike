use bike_core::{config::Config, observability};
use std::future::{Future, IntoFuture};
use strava_temporal::client::Error;
use strava_temporal::client::{connect, worker, TemporalConfig};
use temporalio_sdk::Runtime;

#[tokio::main]
async fn main() -> Result<(), strava_temporal::client::Error> {
    let _observability = observability::init_observability("bike-temporal");
    let config = TemporalConfig::from_env();
    let app = Config::init_from_env();
    let db = bike_core::db::connect_database(&app.database_url).await?;
    let client = connect(&config).await?;
    bike_core::provider_metrics::init_provider_metrics();
    let sdk_address = std::env::var("TEMPORAL_METRICS_ADDRESS")
        .unwrap_or_else(|_| "0.0.0.0:9092".into())
        .parse()?;
    let (options, _metrics_server) = worker::temporal_metrics::start(sdk_address)?;
    let runtime = Runtime::from_current_tokio(options)?;
    let listener = tokio::net::TcpListener::bind("0.0.0.0:9091").await?;
    let metrics = axum::serve(
        listener,
        axum::Router::new().route("/metrics", axum::routing::get(provider_metrics)),
    )
    .into_future();
    let mut strava = worker(&runtime, client.clone(), &config, db.clone())?;
    let mut bike = bike_temporal::runtime::worker(&runtime, client, db, app.uploads_dir.clone())?;
    let stop_strava = strava.shutdown_handle();
    let stop_bike = bike.shutdown_handle();
    let shutdown = || {
        stop_strava();
        stop_bike();
    };
    tracing::info!(
        temporal_namespace = config.namespace,
        task_queue = config.task_queue,
        bike_task_queue = bike_temporal::TASK_QUEUE,
        "Temporal worker started for Strava and Bike"
    );
    let queue_stop_strava = strava.shutdown_handle();
    let queue_stop_bike = bike.shutdown_handle();
    let run = run_queues(
        async { strava.run().await.map_err(Into::into) },
        async { bike.run().await.map_err(Into::into) },
        queue_stop_strava,
        queue_stop_bike,
    );
    supervise(run, metrics, shutdown_signal(), shutdown).await
}

/// Either queue ending stops/drains its sibling before the process exits.
async fn run_queues(
    strava: impl Future<Output = Result<(), Error>>,
    bike: impl Future<Output = Result<(), Error>>,
    stop_strava: impl FnOnce(),
    stop_bike: impl FnOnce(),
) -> Result<(), Error> {
    tokio::pin!(strava);
    tokio::pin!(bike);
    tokio::select! {
        result = &mut strava => {
            stop_bike();
            let drained = bike.await;
            result?;
            drained?;
        }
        result = &mut bike => {
            stop_strava();
            let drained = strava.await;
            result?;
            drained?;
        }
    }
    Ok(())
}

async fn supervise(
    run: impl Future<Output = Result<(), Error>>,
    metrics: impl Future<Output = Result<(), std::io::Error>>,
    signal: impl Future<Output = Result<(), std::io::Error>>,
    shutdown: impl FnOnce(),
) -> Result<(), Error> {
    tokio::pin!(run);
    tokio::pin!(metrics);
    tokio::pin!(signal);
    tokio::select! {
        result = &mut run => result?,
        result = &mut signal => { shutdown(); run.await?; result?; }
        result = &mut metrics => { shutdown(); run.await?; result?; }
    }
    Ok(())
}

async fn provider_metrics() -> Result<([(axum::http::HeaderName, &'static str); 1], String), axum::http::StatusCode> {
    let mut body = String::new();
    prometheus::TextEncoder::new()
        .encode_utf8(&prometheus::gather(), &mut body)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(([(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")], body))
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
    async fn either_queue_completing_or_failing_drains_its_sibling() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };
        for strava_finishes in [true, false] {
            for fail in [true, false] {
                let (send, receive) = tokio::sync::oneshot::channel();
                let drained = Arc::new(AtomicBool::new(false));
                let finished = drained.clone();
                let ending = async move {
                    if fail {
                        Err(std::io::Error::other("queue failed").into())
                    } else {
                        Ok(())
                    }
                };
                let sibling = async move {
                    receive.await.unwrap();
                    finished.store(true, Ordering::SeqCst);
                    Ok(())
                };
                let stop = || {
                    send.send(()).unwrap();
                };
                let result = if strava_finishes {
                    run_queues(ending, sibling, || panic!("queue already stopped"), stop).await
                } else {
                    run_queues(sibling, ending, stop, || panic!("queue already stopped")).await
                };
                assert_eq!(result.is_err(), fail);
                assert!(drained.load(Ordering::SeqCst));
                if fail {
                    assert_eq!(result.unwrap_err().to_string(), "queue failed");
                }
            }
        }
    }

    #[tokio::test]
    async fn worker_completion_and_failure_end_the_process() {
        supervise(async { Ok(()) }, pending(), pending(), || {
            panic!("Worker already stopped")
        })
        .await
        .unwrap();
        let error = supervise(
            async { Err(std::io::Error::other("worker failed").into()) },
            pending(),
            pending(),
            || panic!("Worker already stopped"),
        )
        .await
        .unwrap_err();
        assert_eq!(error.to_string(), "worker failed");
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
        let (drain, drained) = tokio::sync::oneshot::channel();
        let (stopped, mut stop_received) = tokio::sync::oneshot::channel();
        let mut running = std::pin::pin!(supervise(
            async {
                receive.await.unwrap();
                stopped.send(()).unwrap();
                drained.await.unwrap();
                Ok(())
            },
            pending(),
            async { Ok(()) },
            || {
                send.send(()).unwrap();
            },
        ));
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        assert!(running.as_mut().poll(&mut context).is_pending());
        stop_received.try_recv().unwrap();
        drain.send(()).unwrap();
        running.await.unwrap();
    }

    #[tokio::test]
    async fn provider_endpoint_encodes_the_registered_metrics() {
        bike_core::provider_metrics::init_provider_metrics();
        bike_core::provider_metrics::record_provider_api_request("strava", "activity", "read", "success");
        let (headers, body) = provider_metrics().await.unwrap();
        assert_eq!(headers[0].0, axum::http::header::CONTENT_TYPE);
        assert!(body.contains("bike_rust_provider_api_requests_total"));
    }
}
