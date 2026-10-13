# Diff Coverage
## Diff: 83498dc9713c86f8deba96555290751c6712047a...HEAD, staged, unstaged and untracked changes

- bike-rs/worker/src/main&#46;rs (0.0%): Missing lines 17
- bike-rs/worker/src/tasks/mod&#46;rs (100%)
- bike-rs/worker/src/tasks/strava/backfill&#46;rs (100%)
- bike-rs/worker/src/tasks/strava/create&#46;rs (100%)
- bike-rs/worker/src/tasks/strava/delete&#46;rs (100%)
- bike-rs/worker/src/tasks/strava/disconnect&#46;rs (100%)
- bike-rs/worker/src/tasks/strava/legacy&#46;rs (96.3%): Missing lines 55
- bike-rs/worker/src/tasks/strava/mod&#46;rs (100%)
- bike-rs/worker/src/tasks/strava/sync&#46;rs (100%)
- bike-rs/worker/src/tasks/strava/update&#46;rs (100%)
- bike-rs/worker/src/tasks/strava/upsert&#46;rs (100%)

## Summary

- **Total**: 142 lines
- **Missing**: 2 lines
- **Coverage**: 98.59%



## bike-rs/worker/src/main&#46;rs

```
  13 #[tokio::main]
  14 async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  15     let _observability = observability::init_observability("bike-rust-worker");
  16     provider_metrics::init_provider_metrics();
! 17     bike_core::strava::retention::init_metrics();
  18 
  19     let cfg = Config::init_from_env();
  20     let db = bike_core::db::connect_database(&cfg.database_url).await?;
```


---



## bike-rs/worker/src/tasks/strava/legacy&#46;rs

```
  51             .one(db)
  52             .await?
  53         {
  54             workflow::queue_sync(db, &connection, false).await?;
! 55         }
  56         Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
  57     }
  58     .instrument(span)
  59     .await
```


---


