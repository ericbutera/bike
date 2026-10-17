# Diff Coverage
## Diff: 3b8291e82ba259a28a05e4b3dc13ddb9dd759efb...HEAD, staged, unstaged and untracked changes

- bike-rs/api/src/controllers/activities&#46;rs (100%)
- bike-rs/api/src/controllers/strava&#46;rs (100%)
- bike-rs/api/src/storage&#46;rs (50.0%): Missing lines 63

## Summary

- **Total**: 21 lines
- **Missing**: 1 line
- **Coverage**: 95.24%



## bike-rs/api/src/storage&#46;rs

```
  59         let uploads_dir = Config::get().uploads_dir.clone();
  60         std::fs::create_dir_all(&uploads_dir).expect("Failed to create uploads directory");
  61 
  62         Self {
! 63             strava_intake: integrations::strava::events::StravaIntake::new(db.clone()),
  64             task_submission: std::sync::Arc::new(bike_temporal::submission::TaskSubmissionService::from_env()),
  65             map_images: create_map_image_service(Config::get()).await,
  66             heatmaps: std::sync::Arc::new(bike_core::heatmaps::service::HeatmapService::default()),
  67             db,
```


---


