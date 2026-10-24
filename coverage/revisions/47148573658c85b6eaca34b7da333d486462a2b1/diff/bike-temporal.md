# Diff Coverage
## Diff: 0f794ebf3fc17af3579a9fcf5eedc2031695504f...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-temporal/src/runtime&#46;rs (0.0%): Missing lines 31,60-61
- bike-rs/bike-temporal/src/workflows/recordings&#46;rs (100%)

## Summary

- **Total**: 106 lines
- **Missing**: 3 lines
- **Coverage**: 97.17%



## bike-rs/bike-temporal/src/runtime&#46;rs

```
  27     db: sea_orm::DatabaseConnection,
  28     uploads_dir: String,
  29 ) -> Result<Worker, crate::client::Error> {
  30     let objects = ObjectStore::filesystem(&uploads_dir, ObjectOwner::ActivityImports);
! 31     let recordings = crate::workflows::RecordingActivities::new(db.clone(), objects.clone());
  32     let operations = Arc::new(BikeOperations::new(db, objects));
  33     let options = WorkerOptions::new(TASK_QUEUE)
  34         .tuner(
  35             TunerHolder::builder()
```


---


```
  56         .register_workflow::<BackfillActivityCards>()?
  57         .register_workflow::<BackfillPersonalAchievements>()?
  58         .register_workflow::<BackfillOverallAchievements>()?
  59         .register_workflow::<BackfillEarnedAwards>()?
! 60         .register_workflow::<crate::workflows::BackfillActivityRecordings>()?
! 61         .register_activities(recordings)
  62         .register_activities(ImportActivities::new(operations.clone()))
  63         .register_activities(BatchActivities::new(operations.clone()))
  64         .register_activities(DerivedActivities::new(operations))
  65         .graceful_shutdown_period(Duration::from_secs(10))
```


---


