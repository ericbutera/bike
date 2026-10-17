# Diff Coverage
## Diff: e401699acdc0b5280a64d13d689fc3efd11cdd43...HEAD, staged, unstaged and untracked changes

- bike-rs/worker/src/temporal_main&#46;rs (96.6%): Missing lines 14

## Summary

- **Total**: 29 lines
- **Missing**: 1 line
- **Coverage**: 96.55%



## bike-rs/worker/src/temporal_main&#46;rs

```
  10     let config = TemporalConfig::from_env();
  11     let app = Config::init_from_env();
  12     let db = bike_core::db::connect_database(&app.database_url).await?;
  13     let client = connect(&config).await?;
! 14     ensure_schedules(&client, &config).await?;
  15     bike_core::provider_metrics::init_provider_metrics();
  16     let sdk_address = std::env::var("TEMPORAL_METRICS_ADDRESS")
  17         .unwrap_or_else(|_| "0.0.0.0:9092".into())
  18         .parse()?;
```


---


