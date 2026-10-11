# Diff Coverage
## Diff: 7094b5d003751e97b6e714459a2fad50d7a0c6fd...HEAD, staged, unstaged and untracked changes

- bike-rs/api/src/controllers/activity_imports&#46;rs (100%)
- bike-rs/api/src/controllers/mod&#46;rs (100%)
- bike-rs/api/src/controllers/strava&#46;rs (96.6%): Missing lines 391

## Summary

- **Total**: 52 lines
- **Missing**: 1 line
- **Coverage**: 98.08%



## bike-rs/api/src/controllers/strava&#46;rs

```
  387     let waiting = match connection {
  388         Some(connection) => {
  389             bike_core::entities::strava_workflows::Model::waiting(db, connection.id).await?
  390         }
! 391         None => None,
  392     };
  393 
  394     Ok(StravaConnectionResponse {
  395         configured: config.strava_enabled(),
```


---


