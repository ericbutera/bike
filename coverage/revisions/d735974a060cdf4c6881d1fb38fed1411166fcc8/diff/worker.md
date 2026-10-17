# Diff Coverage
## Diff: 3b8291e82ba259a28a05e4b3dc13ddb9dd759efb...HEAD, staged, unstaged and untracked changes

- bike-rs/worker/src/temporal_main&#46;rs (91.7%): Missing lines 48

## Summary

- **Total**: 12 lines
- **Missing**: 1 line
- **Coverage**: 91.67%



## bike-rs/worker/src/temporal_main&#46;rs

```
  44         async { bike.run().await.map_err(Into::into) },
  45         queue_stop_strava,
  46         queue_stop_bike,
  47     );
! 48     supervise(run, metrics, shutdown_signal(), shutdown).await
  49 }
  50 
  51 /// Either queue ending stops/drains its sibling before the process exits.
  52 async fn run_queues(
```


---


