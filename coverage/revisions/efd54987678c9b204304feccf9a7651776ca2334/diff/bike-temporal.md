# Diff Coverage
## Diff: a530c81618ed97838a4d44ff83c6632044788ac4...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-temporal/src/activities/batches&#46;rs (100%)
- bike-rs/bike-temporal/src/operations/batches&#46;rs (100%)
- bike-rs/bike-temporal/src/runtime&#46;rs (0.0%): Missing lines 54
- bike-rs/bike-temporal/src/workflows/activity_pages&#46;rs (100%)

## Summary

- **Total**: 93 lines
- **Missing**: 1 line
- **Coverage**: 98.92%



## bike-rs/bike-temporal/src/runtime&#46;rs

```
  50         .register_workflow::<BackfillUserXcTraining>()?
  51         .register_workflow::<ActivityArchiveImport>()?
  52         .register_workflow::<ReconcileHeatmaps>()?
  53         .register_workflow::<BackfillActivityClimbs>()?
! 54         .register_workflow::<BackfillPersonalAchievements>()?
  55         .register_activities(ImportActivities::new(operations.clone()))
  56         .register_activities(BatchActivities::new(operations.clone()))
  57         .register_activities(DerivedActivities::new(operations))
  58         .graceful_shutdown_period(Duration::from_secs(10))
```


---


