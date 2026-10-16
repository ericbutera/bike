# Diff Coverage
## Diff: 7e43ab6d2d5b70d305a52e5f12bf704593c5fad7...HEAD, staged, unstaged and untracked changes

- bike-rs/strava-temporal/src/activities&#46;rs (80.0%): Missing lines 348
- bike-rs/strava-temporal/src/handoff_metrics&#46;rs (86.7%): Missing lines 25,30
- bike-rs/strava-temporal/src/workflows/options&#46;rs (100%)

## Summary

- **Total**: 23 lines
- **Missing**: 3 lines
- **Coverage**: 86.96%



## bike-rs/strava-temporal/src/activities&#46;rs

```
  344                 }
  345             };
  346             match &result {
  347                 Ok(_) => logging.attempt(&info.activity_type, AttemptStatus::Completed),
! 348                 Err(ActivityError::Cancelled { .. }) => logging.attempt(&info.activity_type, AttemptStatus::Cancelled),
  349                 Err(_) => logging.attempt(&info.activity_type, AttemptStatus::Failed),
  350             }
  351             result
  352         })
```


---



## bike-rs/strava-temporal/src/handoff_metrics&#46;rs

```
  21             oldest: gauge(
  22                 registry,
  23                 "bike_rust_strava_handoff_oldest_accepted_timestamp_seconds",
  24                 "Acceptance timestamp of the oldest pending Strava event; zero when empty.",
! 25             )?,
  26             sampled_at: gauge(
  27                 registry,
  28                 "bike_rust_strava_handoff_last_sample_timestamp_seconds",
  29                 "Timestamp of the last successful durable handoff backlog sample; zero before the first sample.",
! 30             )?,
  31         })
  32     }
  33 
  34     pub async fn sample(&self, db: &impl ConnectionTrait, now: DateTime<Utc>) -> Result<(), DbErr> {
```


---


