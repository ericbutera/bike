# Diff Coverage
## Diff: fc9bf1566dd44e6fe89206859d0c4b2d5164b63e...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-core/src/activity_lifecycle&#46;rs (95.5%): Missing lines 25
- bike-rs/bike-core/src/activity_parser/strava&#46;rs (100%)
- bike-rs/bike-core/src/entities/activities/history&#46;rs (100%)
- bike-rs/bike-core/src/entities/activities/parsed&#46;rs (100%)

## Summary

- **Total**: 121 lines
- **Missing**: 1 line
- **Coverage**: 99.17%



## bike-rs/bike-core/src/activity_lifecycle&#46;rs

```
  21     let outcome = activity.update_details(db, update).await?;
  22     let updated = outcome.activity;
  23     if outcome.sport_changed {
  24         if updated.is_bike_activity() {
! 25             crate::activity_training_analysis::rebuild_activity_training_analysis_cache(db, &[updated.id]).await?;
  26         }
  27         tasks
  28             .submit_command(
  29                 crate::jobs::MaintenanceTask::ActivitySegments(crate::bike_tasks::ActivityTask {
```


---


