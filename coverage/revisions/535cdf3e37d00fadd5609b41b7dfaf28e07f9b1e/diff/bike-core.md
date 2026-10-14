# Diff Coverage
## Diff: c997295d36529a615e8daf459ead364e50f7f571...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-core/src/activity_data&#46;rs (91.7%): Missing lines 120
- bike-rs/bike-core/src/activity_sport&#46;rs (100%)
- bike-rs/bike-core/src/analytics&#46;rs (100%)
- bike-rs/bike-core/src/entities/activities/parsed&#46;rs (100%)
- bike-rs/bike-core/src/entities/activity_training_analyses&#46;rs (100%)
- bike-rs/bike-core/src/strava/events/intake&#46;rs (100%)
- bike-rs/bike-core/src/strava/events/intake/strava_webhook_unit&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/mod&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/persistence&#46;rs (80.3%): Missing lines 148-150,325-326,329-335
- bike-rs/bike-core/src/strava/native/persistence/strava_native_unit_deletion&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/strava_native_unit/handlers&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/strava_native_unit/upsert&#46;rs (100%)
- bike-rs/bike-core/src/strava/payload&#46;rs (100%)
- bike-rs/bike-core/src/strava/provider_payload&#46;rs (100%)

## Summary

- **Total**: 427 lines
- **Missing**: 13 lines
- **Coverage**: 96.96%



## bike-rs/bike-core/src/activity_data&#46;rs

```
  116 impl Default for StoredActivityDerivedData {
  117     fn default() -> Self {
  118         Self {
  119             v: STORAGE_FORMAT_VERSION,
! 120             source_metadata: None,
  121             recording: Default::default(),
  122             laps: Vec::new(),
  123             chart_points: Vec::new(),
  124             route_points: Vec::new(),
```


---



## bike-rs/bike-core/src/strava/native/persistence&#46;rs

```
  144         let mut change = ActivityChange::unchanged(input.event.clone(), ActivityStatus::Unchanged);
  145         change.event.bike_activity_id = Some(activity.id);
  146         change.bike_activity_id = Some(activity.id);
  147         change.import_id = activity.activity_import_id;
! 148         change.dirty_from = activity
! 149             .is_bike_activity()
! 150             .then_some(activity.started_at.date_naive());
  151         strava_activity_states::Model::apply_event(
  152             db,
  153             &input.event,
  154             Some(input.source.sha256.clone()),
```


---


```
  321                     chrono::Utc::now(),
  322                 )
  323                 .await?;
  324                 crate::analytics::rebuild_activity_analytics_cache(&txn, &[id]).await?;
! 325                 crate::entities::activity_training_analyses::Model::clear_for_non_cycling(
! 326                     &txn, &activity,
  327                 )
  328                 .await?;
! 329                 if activity.is_bike_activity() {
! 330                     crate::activity_training_analysis::rebuild_activity_training_analysis_cache(
! 331                         &txn,
! 332                         &[id],
! 333                     )
! 334                     .await?;
! 335                 }
  336             }
  337         }
  338         txn.commit().await?;
  339         Ok(change)
```


---


