# Diff Coverage
## Diff: f73e6a378831ef2cb576d65a7fb4b8da1b08b607...HEAD, staged, unstaged and untracked changes

- bike-rs/worker/src/main&#46;rs (0.0%): Missing lines 17
- bike-rs/worker/src/tasks/mod&#46;rs (100%)
- bike-rs/worker/src/tasks/processors/strava_retention&#46;rs (81.2%): Missing lines 41-43

## Summary

- **Total**: 20 lines
- **Missing**: 4 lines
- **Coverage**: 80.0%



## bike-rs/worker/src/main&#46;rs

```
  13 #[tokio::main]
  14 async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  15     let _observability = observability::init_observability("bike-rust-worker");
  16     provider_metrics::init_provider_metrics();
! 17     bike_core::strava_retention::init_metrics();
  18 
  19     let cfg = Config::init_from_env();
  20     let db = bike_core::db::connect_database(&cfg.database_url).await?;
```


---



## bike-rs/worker/src/tasks/processors/strava_retention&#46;rs

```
  37 }
  38 
  39 #[async_trait]
  40 impl WorkerStartupHook for StravaRetention {
! 41     fn name(&self) -> &str {
! 42         "strava_retention"
! 43     }
  44     async fn run(&self, db: &DatabaseConnection) -> Result<(), WorkerError> {
  45         TaskQueue::new(DurableStorage::new(db.clone()))
  46             .enqueue("strava_retention".into(), serde_json::json!({}))
  47             .await?;
```


---


