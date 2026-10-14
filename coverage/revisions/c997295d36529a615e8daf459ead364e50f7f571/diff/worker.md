# Diff Coverage
## Diff: 70078f649704a8a21cd025e3c5c2f992dd43f8d4...HEAD, staged, unstaged and untracked changes

- bike-rs/worker/src/temporal_main&#46;rs (94.4%): Missing lines 18,34-36,86,95

## Summary

- **Total**: 108 lines
- **Missing**: 6 lines
- **Coverage**: 94.44%



## bike-rs/worker/src/temporal_main&#46;rs

```
  14     bike_core::provider_metrics::init_provider_metrics();
  15     let sdk_address = std::env::var("TEMPORAL_METRICS_ADDRESS")
  16         .unwrap_or_else(|_| "0.0.0.0:9092".into())
  17         .parse()?;
! 18     let (options, _metrics_server) = strava_temporal::metrics::start(sdk_address)?;
  19     let runtime = Runtime::from_current_tokio(options)?;
  20     let listener = tokio::net::TcpListener::bind("0.0.0.0:9091").await?;
  21     let metrics = axum::serve(
  22         listener,
```


---


```
  30         temporal_namespace = config.namespace,
  31         task_queue = config.task_queue,
  32         "Native Strava Temporal worker started"
  33     );
! 34     let run = async { worker.run().await.map_err(Into::into) };
! 35     supervise(run, events, metrics, shutdown_signal(), shutdown).await
! 36 }
  37 
  38 async fn supervise(
  39     run: impl Future<Output = Result<(), Error>>,
  40     events: impl Future<Output = Result<(), Error>>,
```


---


```
  82 
  83     #[tokio::test]
  84     async fn worker_completion_and_failure_end_the_process() {
  85         supervise(async { Ok(()) }, pending(), pending(), pending(), || {
! 86             panic!("Worker already stopped")
  87         })
  88         .await
  89         .unwrap();
  90         let error = supervise(
```


---


```
  91             async { Err(std::io::Error::other("worker failed").into()) },
  92             pending(),
  93             pending(),
  94             pending(),
! 95             || panic!("Worker already stopped"),
  96         )
  97         .await
  98         .unwrap_err();
  99         assert_eq!(error.to_string(), "worker failed");
```


---


