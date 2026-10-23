# Diff Coverage
## Diff: 44d6d41bb1521ce763a727d6d14e99fbda6d379f...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-temporal/src/activities/batches&#46;rs (100%)
- bike-rs/bike-temporal/src/operations/batches&#46;rs (100%)
- bike-rs/bike-temporal/src/runtime&#46;rs (0.0%): Missing lines 56
- bike-rs/bike-temporal/src/workflows/activity_pages&#46;rs (100%)

## Summary

- **Total**: 93 lines
- **Missing**: 1 line
- **Coverage**: 98.92%



## bike-rs/bike-temporal/src/runtime&#46;rs

```
  52         .register_workflow::<ActivityArchiveImport>()?
  53         .register_workflow::<ReconcileHeatmaps>()?
  54         .register_workflow::<BackfillActivityClimbs>()?
  55         .register_workflow::<BackfillPersonalAchievements>()?
! 56         .register_workflow::<BackfillOverallAchievements>()?
  57         .register_activities(ImportActivities::new(operations.clone()))
  58         .register_activities(BatchActivities::new(operations.clone()))
  59         .register_activities(DerivedActivities::new(operations))
  60         .graceful_shutdown_period(Duration::from_secs(10))
```


---


