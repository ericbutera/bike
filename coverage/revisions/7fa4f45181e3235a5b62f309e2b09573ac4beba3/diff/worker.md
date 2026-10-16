# Diff Coverage
## Diff: 7e43ab6d2d5b70d305a52e5f12bf704593c5fad7...HEAD, staged, unstaged and untracked changes

- bike-rs/worker/src/temporal_main&#46;rs (74.3%): Missing lines 9,11-12,19,28-35,42-48
- bike-rs/worker/src/temporal_metrics&#46;rs (0.0%): Missing lines 19

## Summary

- **Total**: 75 lines
- **Missing**: 20 lines
- **Coverage**: 73.33%



## bike-rs/worker/src/temporal_main&#46;rs

```
   5 use temporalio_sdk::Runtime;
   6 
   7 #[tokio::main]
   8 async fn main() -> Result<(), strava_temporal::client::Error> {
!  9     let _observability = observability::init_observability("bike-temporal");
  10     let config = TemporalConfig::from_env();
! 11     let app = Config::init_from_env();
! 12     let db = bike_core::db::connect_database(&app.database_url).await?;
  13     let client = connect(&config).await?;
  14     strava_temporal::client::ensure_schedule(&client, &config).await?;
  15     bike_core::provider_metrics::init_provider_metrics();
  16     let sdk_address = std::env::var("TEMPORAL_METRICS_ADDRESS")
```


---


```
  15     bike_core::provider_metrics::init_provider_metrics();
  16     let sdk_address = std::env::var("TEMPORAL_METRICS_ADDRESS")
  17         .unwrap_or_else(|_| "0.0.0.0:9092".into())
  18         .parse()?;
! 19     let (options, _metrics_server) = worker::temporal_metrics::start(sdk_address)?;
  20     let runtime = Runtime::from_current_tokio(options)?;
  21     let listener = tokio::net::TcpListener::bind("0.0.0.0:9091").await?;
  22     let metrics = axum::serve(
  23         listener,
```


---


```
  24         axum::Router::new().route("/metrics", axum::routing::get(provider_metrics)),
  25     )
  26     .into_future();
  27     let events = dispatch_events(client.clone(), config.clone(), db.clone());
! 28     let mut strava = worker(&runtime, client.clone(), &config, db.clone())?;
! 29     let mut bike = bike_temporal::runtime::worker(&runtime, client, db, app.uploads_dir.clone())?;
! 30     let stop_strava = strava.shutdown_handle();
! 31     let stop_bike = bike.shutdown_handle();
! 32     let shutdown = || {
! 33         stop_strava();
! 34         stop_bike();
! 35     };
  36     tracing::info!(
  37         temporal_namespace = config.namespace,
  38         task_queue = config.task_queue,
  39         bike_task_queue = bike_temporal::TASK_QUEUE,
```


---


```
  38         task_queue = config.task_queue,
  39         bike_task_queue = bike_temporal::TASK_QUEUE,
  40         "Temporal worker started for Strava and Bike"
  41     );
! 42     let queue_stop_strava = strava.shutdown_handle();
! 43     let queue_stop_bike = bike.shutdown_handle();
! 44     let run = run_queues(
! 45         async { strava.run().await.map_err(Into::into) },
! 46         async { bike.run().await.map_err(Into::into) },
! 47         queue_stop_strava,
! 48         queue_stop_bike,
  49     );
  50     supervise(run, events, metrics, shutdown_signal(), shutdown).await
  51 }
```


---



## bike-rs/worker/src/temporal_metrics&#46;rs

```
  15 }
  16 
  17 pub fn start(
  18     address: std::net::SocketAddr,
! 19 ) -> Result<(RuntimeOptions, MetricsServer), Box<dyn std::error::Error + Send + Sync>> {
  20     let server = start_prometheus_metric_exporter(
  21         PrometheusExporterOptions::builder()
  22             .socket_addr(address)
  23             .counters_total_suffix(true)
```


---


