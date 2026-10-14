# Diff Coverage
## Diff: 70078f649704a8a21cd025e3c5c2f992dd43f8d4...HEAD, staged, unstaged and untracked changes

- bike-rs/strava-temporal/src/activities&#46;rs (90.5%): Missing lines 383,396
- bike-rs/strava-temporal/src/client&#46;rs (91.7%): Missing lines 24-25,191
- bike-rs/strava-temporal/src/handoff_metrics&#46;rs (100%)
- bike-rs/strava-temporal/src/metrics&#46;rs (100%)

## Summary

- **Total**: 146 lines
- **Missing**: 5 lines
- **Coverage**: 96.58%



## bike-rs/strava-temporal/src/activities&#46;rs

```
  379         for (status, terminal) in [(400, true), (404, true), (429, false), (500, false)] {
  380             let mut error = WorkflowError::internal("fixture failure");
  381             error.status = status.try_into().unwrap();
  382             let ActivityError::Application(failure) = activity_error(error) else {
! 383                 panic!("Expected application failure")
  384             };
  385             assert_eq!(failure.is_non_retryable(), terminal);
  386             assert_eq!(failure.type_name(), Some("StravaOperationFailure"));
  387         }
```


---


```
  392         for seconds in [120, -120] {
  393             let mut error = WorkflowError::bad_request("fixture provider pause");
  394             error.retry_at = Some(chrono::Utc::now() + chrono::Duration::seconds(seconds));
  395             let ActivityError::Application(failure) = activity_error(error) else {
! 396                 panic!("Expected application failure")
  397             };
  398             assert!(!failure.is_non_retryable());
  399             let delay = failure.next_retry_delay().unwrap();
  400             if seconds < 0 {
```


---



## bike-rs/strava-temporal/src/client&#46;rs

```
  20     pub task_queue: String,
  21 }
  22 impl TemporalConfig {
  23     pub fn from_env() -> Self {
! 24         Self::from_lookup(|name| std::env::var(name).ok())
! 25     }
  26 
  27     fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Self {
  28         Self {
  29             address: lookup("TEMPORAL_ADDRESS").unwrap_or_else(|| "http://temporal:7233".into()),
```


---


```
  187     let mut next_sample = tokio::time::Instant::now();
  188     loop {
  189         if tokio::time::Instant::now() >= next_sample {
  190             if let Err(error) = metrics.sample(&db, chrono::Utc::now()).await {
! 191                 tracing::error!(error = %error, "Strava handoff backlog sample failed");
  192             }
  193             next_sample = tokio::time::Instant::now() + Duration::from_secs(30);
  194         }
  195         dispatch_pending(&client, &config, &db).await?;
```


---


