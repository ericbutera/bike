# Diff Coverage
## Diff: 7fa4f45181e3235a5b62f309e2b09573ac4beba3...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-core/src/analytics&#46;rs (100%)
- bike-rs/bike-core/src/entities/activities&#46;rs (94.4%): Missing lines 590
- bike-rs/bike-core/src/entities/analytics_user_states&#46;rs (100%)
- bike-rs/bike-core/src/entities/segments&#46;rs (100%)
- bike-rs/bike-core/src/entities/segments/mutations&#46;rs (50.0%): Missing lines 215
- bike-rs/bike-core/src/strava/native/persistence&#46;rs (0.0%): Missing lines 334-335
- bike-rs/bike-core/src/strava/native/strava_native_unit/upsert&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/strava_native_unit/workflow_spans&#46;rs (100%)
- bike-rs/bike-core/src/test_support/analytics&#46;rs (100%)
- bike-rs/bike-core/src/test_support/segments&#46;rs (100%)
- bike-rs/bike-core/src/test_support/trace&#46;rs (75.0%): Missing lines 76

## Summary

- **Total**: 109 lines
- **Missing**: 5 lines
- **Coverage**: 95.41%



## bike-rs/bike-core/src/entities/activities&#46;rs

```
  586         db: &impl ConnectionTrait,
  587         activity_ids: &[i32],
  588     ) -> Result<(), DbErr> {
  589         if activity_ids.is_empty() {
! 590             return Ok(());
  591         }
  592         Entity::find()
  593             .select_only()
  594             .column(Column::Id)
```


---



## bike-rs/bike-core/src/entities/segments/mutations&#46;rs

```
  211         } else {
  212             Vec::new()
  213         };
  214         let txn = db.begin().await?;
! 215         activities::Model::lock_for_analytics(&txn, &activity_ids).await?;
  216         let mut active_segment = self.into_active_model().with_builder_link(source);
  217         if should_update_title {
  218             active_segment.title = Set(source.title.clone());
  219         }
```


---



## bike-rs/bike-core/src/strava/native/persistence&#46;rs

```
  330         if strava_activity_states::Model::supersedes(&txn, &change.event, false).await? {
  331             return Ok(change);
  332         }
  333         if let Some(id) = change.bike_activity_id {
! 334             if let Some(activity) =
! 335                 activities::Model::lock_owned(&txn, id, change.event.bike_user_id).await?
  336             {
  337                 let derived = crate::activity_data::deserialize_derived_activity_data(
  338                     activity.derived_data_json.as_ref(),
  339                 );
```


---



## bike-rs/bike-core/src/test_support/trace&#46;rs

```
  72         .collect();
  73     assert!(
  74         !queries.is_empty(),
  75         "Expected database work below {name}; captured: {:?}",
! 76         spans.iter().map(|span| &span.name).collect::<Vec<_>>()
  77     );
  78     for query in queries {
  79         assert!(query.start_time >= stage.start_time);
  80         assert!(query.end_time <= stage.end_time);
```


---


