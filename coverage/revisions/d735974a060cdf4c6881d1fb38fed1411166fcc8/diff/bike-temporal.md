# Diff Coverage
## Diff: 3b8291e82ba259a28a05e4b3dc13ddb9dd759efb...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-temporal/src/operations&#46;rs (100%)
- bike-rs/bike-temporal/src/runtime&#46;rs (0.0%): Missing lines 30
- bike-rs/bike-temporal/src/workflows/process_activity_import&#46;rs (100%)
- bike-rs/bike-temporal/src/workflows/reprocess_activity_import&#46;rs (100%)
- bike-rs/bike-temporal/src/workflows/steps/derived&#46;rs (100%)

## Summary

- **Total**: 11 lines
- **Missing**: 1 line
- **Coverage**: 90.91%



## bike-rs/bike-temporal/src/runtime&#46;rs

```
  26     db: sea_orm::DatabaseConnection,
  27     uploads_dir: String,
  28 ) -> Result<Worker, crate::client::Error> {
  29     let objects = ObjectStore::filesystem(&uploads_dir, ObjectOwner::ActivityImports);
! 30     let operations = Arc::new(BikeOperations::new(db, objects));
  31     let options = WorkerOptions::new(TASK_QUEUE)
  32         .tuner(
  33             TunerHolder::builder()
  34                 .workflow_task_slot_supplier(FixedSizeSlotSupplier::new(16))
```


---


