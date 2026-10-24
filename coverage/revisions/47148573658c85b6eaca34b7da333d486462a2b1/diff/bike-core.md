# Diff Coverage
## Diff: 0f794ebf3fc17af3579a9fcf5eedc2031695504f...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-core/src/activity_data&#46;rs (100%)
- bike-rs/bike-core/src/activity_reprocessing&#46;rs (100%)
- bike-rs/bike-core/src/entities/activities/parsed&#46;rs (100%)
- bike-rs/bike-core/src/entities/activities/reprocessing&#46;rs (90.9%): Missing lines 80,113
- bike-rs/bike-core/src/entities/activity_recordings&#46;rs (100%)
- bike-rs/bike-core/src/test_support/storage&#46;rs (100%)

## Summary

- **Total**: 201 lines
- **Missing**: 2 lines
- **Coverage**: 99.0%



## bike-rs/bike-core/src/entities/activities/reprocessing&#46;rs

```
  76             return Ok(None);
  77         };
  78         let expected_updated_at = activity.updated_at;
  79         let Some(activity) = Self::lock_owned(db, activity.id, import.user_id).await? else {
! 80             return Err(WorkflowError::not_found("Duplicate activity was deleted"));
  81         };
  82         if activity.updated_at != expected_updated_at {
  83             return Err(WorkflowError::internal(
  84                 "Activity changed while resolving duplicate recording; retry import",
```


---


```
  109         if derived.recording != previous {
  110             let stored = crate::activity_data::StoredActivityDerivedData::from(&derived);
  111             self.derived_data_json = Some(
  112                 match self.derived_data_json.as_ref().and_then(|data| data.source_metadata()) {
! 113                     Some(metadata) => stored.with_source_metadata(metadata.clone()),
  114                     None => stored,
  115                 },
  116             );
  117         }
```


---


