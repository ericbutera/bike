# Diff Coverage
## Diff: 7094b5d003751e97b6e714459a2fad50d7a0c6fd...HEAD, staged, unstaged and untracked changes

- bike-rs/worker/src/tasks/mod&#46;rs (100%)
- bike-rs/worker/src/tasks/processors/strava_reconcile&#46;rs (100%)
- bike-rs/worker/src/tasks/processors/strava_sync&#46;rs (88.9%): Missing lines 52
- bike-rs/worker/src/tasks/processors/strava_workflow&#46;rs (88.9%): Missing lines 27

## Summary

- **Total**: 33 lines
- **Missing**: 2 lines
- **Coverage**: 93.94%



## bike-rs/worker/src/tasks/processors/strava_sync&#46;rs

```
  48                 .one(&self.db)
  49                 .await?
  50             {
  51                 strava_workflow::queue_sync(&self.db, &connection, false).await?;
! 52             }
  53             Ok::<(), Box<dyn Error + Send + Sync>>(())
  54         }
  55         .instrument(span)
  56         .await?;
```


---



## bike-rs/worker/src/tasks/processors/strava_workflow&#46;rs

```
  23         let data = payload.get("data").unwrap_or(&payload);
  24         let id = data
  25             .get("workflow_id")
  26             .and_then(serde_json::Value::as_str)
! 27             .ok_or_else(|| std::io::Error::other("Missing Strava workflow ID"))?;
  28         if let Err(error) = strava_workflow::execute(&self.db, id).await {
  29             if let Some(at) = error.retry_at {
  30                 strava_workflow::resume_at(&self.db, id, at).await?;
  31             } else {
```


---


