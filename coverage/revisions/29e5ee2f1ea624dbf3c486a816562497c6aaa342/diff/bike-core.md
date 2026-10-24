# Diff Coverage
## Diff: 02b6c648340c26af077702f775a062a30c5c54dc...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-core/src/entities/activity_recordings&#46;rs (100%)
- bike-rs/bike-core/src/entities/segment_efforts&#46;rs (100%)
- bike-rs/bike-core/src/entities/segment_efforts/reconciliation&#46;rs (100%)
- bike-rs/bike-core/src/entities/segment_user_summaries&#46;rs (100%)
- bike-rs/bike-core/src/entities/segments&#46;rs (100%)
- bike-rs/bike-core/src/segment_regeneration&#46;rs (86.7%): Missing lines 49,58
- bike-rs/bike-core/src/segment_support&#46;rs (100%)
- bike-rs/bike-core/src/segment_support/persistence&#46;rs (100%)
- bike-rs/bike-core/src/test_support/storage&#46;rs (100%)

## Summary

- **Total**: 334 lines
- **Missing**: 2 lines
- **Coverage**: 99.4%



## bike-rs/bike-core/src/segment_regeneration&#46;rs

```
  45 }
  46 
  47 pub async fn regenerate_segment_efforts(
  48     db: &DatabaseConnection,
! 49     objects: &ObjectStore,
  50     segment_id: i32,
  51 ) -> Result<(), SegmentRegenerationError> {
  52     let segment = segments::Model::find_by_id(db, segment_id)
  53         .await?
```


---


```
  54         .ok_or_else(|| SegmentRegenerationError::internal(format!("segment {segment_id} was not found")))?;
  55     let route_points = deserialize_segment_route_points(segment.route_data_json.as_ref());
  56 
  57     let affected_activity_ids =
! 58         replace_segment_efforts_for_segment(db, objects, segment.user_id, segment.id, &route_points).await?;
  59     rebuild_segment_results(db, &[segment.id], &affected_activity_ids).await?;
  60 
  61     Ok(())
  62 }
```


---


