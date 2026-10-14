# Diff Coverage
## Diff: 7d811e85cc1f1992a99002b0d609e7cbd9d9bdef...HEAD, staged, unstaged and untracked changes

- bike-rs/worker/src/tasks/mod&#46;rs (100%)
- bike-rs/worker/src/tasks/processors/activity_archive_import&#46;rs (100%)
- bike-rs/worker/src/tasks/processors/backfill_user_xc_training&#46;rs (100%)
- bike-rs/worker/src/tasks/processors/email_notification&#46;rs (0.0%): Missing lines 40
- bike-rs/worker/src/tasks/processors/prepare_heatmap&#46;rs (100%)
- bike-rs/worker/src/tasks/processors/process_activity_import&#46;rs (100%)
- bike-rs/worker/src/tasks/processors/rebuild_fitness_freshness&#46;rs (100%)
- bike-rs/worker/src/tasks/processors/rebuild_segment_analytics&#46;rs (100%)
- bike-rs/worker/src/tasks/processors/regenerate_activity_segments&#46;rs (100%)
- bike-rs/worker/src/tasks/processors/regenerate_segment_efforts&#46;rs (100%)
- bike-rs/worker/src/tasks/processors/regenerate_user_segments&#46;rs (100%)
- bike-rs/worker/src/tasks/processors/reprocess_activity_import&#46;rs (100%)
- bike-rs/worker/src/tasks/processors/reprocess_user_activity_imports&#46;rs (100%)
- bike-rs/worker/src/temporal_main&#46;rs (0.0%): Missing lines 7-22,24-30,35-41,43-45,47-60,62-65

## Summary

- **Total**: 65 lines
- **Missing**: 52 lines
- **Coverage**: 20.0%



## bike-rs/worker/src/tasks/processors/email_notification&#46;rs

```
  36 
  37 #[async_trait]
  38 impl TaskProcessor for EmailNotification {
  39     fn task_type(&self) -> &str {
! 40         JobType::EmailNotification.as_str()
  41     }
  42 
  43     async fn process(
  44         &self,
```


---



## bike-rs/worker/src/temporal_main&#46;rs

```
   3 use strava_temporal::client::{connect, dispatch_events, worker, TemporalConfig};
   4 use temporalio_sdk::Runtime;
   5 
   6 #[tokio::main]
!  7 async fn main() -> Result<(), strava_temporal::client::Error> {
!  8     let _observability = observability::init_observability("bike-strava-temporal");
!  9     let config = TemporalConfig::from_env();
! 10     let db = bike_core::db::connect_database(&Config::init_from_env().database_url).await?;
! 11     let client = connect(&config).await?;
! 12     strava_temporal::client::ensure_schedule(&client, &config).await?;
! 13     bike_core::provider_metrics::init_provider_metrics();
! 14     let sdk_address = std::env::var("TEMPORAL_METRICS_ADDRESS")
! 15         .unwrap_or_else(|_| "0.0.0.0:9092".into())
! 16         .parse()?;
! 17     let (options, metrics_server) = strava_temporal::metrics::start(sdk_address)?;
! 18     let runtime = Runtime::from_current_tokio(options)?;
! 19     let listener = tokio::net::TcpListener::bind("0.0.0.0:9091").await?;
! 20     let metrics = axum::serve(
! 21         listener,
! 22         axum::Router::new().route("/metrics", axum::routing::get(provider_metrics)),
  23     )
! 24     .into_future();
! 25     tokio::pin!(metrics);
! 26     let events = dispatch_events(client.clone(), config.clone(), db.clone());
! 27     tokio::pin!(events);
! 28     let mut worker = worker(&runtime, client, &config, db)?;
! 29     let shutdown = worker.shutdown_handle();
! 30     tracing::info!(
  31         temporal_namespace = config.namespace,
  32         task_queue = config.task_queue,
  33         "Native Strava Temporal worker started"
  34     );
! 35     let run = worker.run();
! 36     tokio::pin!(run);
! 37     tokio::select! {
! 38         result = &mut run => result?,
! 39         result = &mut events => { shutdown(); run.await?; result?; }
! 40         result = shutdown_signal() => { result?; shutdown(); run.await?; }
! 41         result = &mut metrics => { shutdown(); run.await?; result?; }
  42     }
! 43     metrics_server.abort_handle.abort();
! 44     Ok(())
! 45 }
  46 
! 47 async fn provider_metrics(
! 48 ) -> Result<([(axum::http::HeaderName, &'static str); 1], String), axum::http::StatusCode> {
! 49     let mut body = String::new();
! 50     prometheus::TextEncoder::new()
! 51         .encode_utf8(&prometheus::gather(), &mut body)
! 52         .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
! 53     Ok((
! 54         [(
! 55             axum::http::header::CONTENT_TYPE,
! 56             "text/plain; version=0.0.4",
! 57         )],
! 58         body,
! 59     ))
! 60 }
  61 
! 62 async fn shutdown_signal() -> Result<(), std::io::Error> {
! 63     let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
! 64     tokio::select! { result = tokio::signal::ctrl_c() => result, _ = terminate.recv() => Ok(()) }
! 65 }
```


---


