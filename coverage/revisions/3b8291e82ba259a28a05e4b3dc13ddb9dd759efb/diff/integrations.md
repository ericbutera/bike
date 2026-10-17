# Diff Coverage
## Diff: e401699acdc0b5280a64d13d689fc3efd11cdd43...HEAD, staged, unstaged and untracked changes

- bike-rs/integrations/src/bin/recover-generated-imports&#46;rs (0.0%): Missing lines 134-138
- bike-rs/integrations/src/strava/connection&#46;rs (0.0%): Missing lines 157-159
- bike-rs/integrations/src/strava/events/intake&#46;rs (100%)
- bike-rs/integrations/src/strava/logging&#46;rs (100%)
- bike-rs/integrations/src/strava/native/dependencies&#46;rs (100%)
- bike-rs/integrations/src/strava/native/mod&#46;rs (100%)
- bike-rs/integrations/src/strava/native/persistence&#46;rs (82.9%): Missing lines 52,128-129,154,249-250,288-289,299,311-312,314,316
- bike-rs/integrations/src/strava/oauth_state&#46;rs (100%)
- bike-rs/integrations/src/strava/objects&#46;rs (100%)
- bike-rs/integrations/src/strava/transport&#46;rs (0.0%): Missing lines 120

## Summary

- **Total**: 157 lines
- **Missing**: 22 lines
- **Coverage**: 85.99%



## bike-rs/integrations/src/bin/recover-generated-imports&#46;rs

```
  130             }
  131         }
  132     }
  133     if run.refresh_recent && recent_gaps {
! 134         let user = bike_core::auth::entities::users::Model::find_by_id(&db, run.user_id).await?;
! 135         let connection = strava::load_connection(&db, run.user_id)
! 136             .await?
! 137             .ok_or("Connect Strava first")?;
! 138         strava::events::request_sync(&db, &user, &connection).await?;
  139     }
  140     println!(
  141         "{}",
  142         serde_json::json!({"next_after_id": last_id, "applied": run.apply, "failed": failed, "recent_refresh_requested": run.refresh_recent && recent_gaps})
```


---



## bike-rs/integrations/src/strava/connection&#46;rs

```
  153         }
  154 
  155         let connection = upsert_connection_from_token(db, verified_state.user_id, token_response).await?;
  156 
! 157         let user = bike_core::auth::entities::users::Model::find_by_id(db, connection.user_id).await?;
! 158         crate::strava::events::request_sync(db, &user, &connection).await?;
! 159         Ok::<_, AppError>(connection)
  160     }
  161     .await;
  162 
  163     match result {
```


---



## bike-rs/integrations/src/strava/native/persistence&#46;rs

```
  48         }
  49         let existing =
  50             activities::Model::lock_strava_activity(&txn, input.event.bike_user_id, input.event.activity_id()?).await?;
  51         if let Some(change) = Self::persist_matching_content(&txn, &input, existing.as_ref()).await? {
! 52             txn.commit().await?;
  53             return Ok(change);
  54         }
  55         let upload = super::super::build_activity_upload(&summary.activity, &summary.streams)?;
  56         let previous_segment_ids = match &existing {
```


---


```
  124             return Ok(None);
  125         };
  126         if !strava_activity_states::Model::matches_content(db, &input.event, &input.source.sha256).await? {
  127             return Ok(None);
! 128         }
! 129         Self::persist_unchanged(db, input, activity).await.map(Some)
  130     }
  131 
  132     async fn persist_unchanged(
  133         db: &DatabaseTransaction,
```


---


```
  150 
  151     pub async fn capture_deletion(&self, mut event: StravaEvent) -> Result<ActivityChange, WorkflowError> {
  152         self.connection(&event).await?;
  153         if event.source_version.is_some() {
! 154             event.source_version = Some(strava_activity_states::Model::capture_version(&self.db, &event).await?);
  155         }
  156         let Some(activity) =
  157             activities::Model::strava_activity(&self.db, event.bike_user_id, event.activity_id()?).await?
  158         else {
```


---


```
  245         previous: Option<&activities::Model>,
  246     ) -> Result<(), WorkflowError> {
  247         let state = strava_activity_states::Model::apply_event(db, &change.event, None, true).await?;
  248         if let Some(day) = change.dirty_from {
! 249             bike_core::analytics::mark_user_fitness_dirty(db, change.event.bike_user_id, day, chrono::Utc::now())
! 250                 .await?;
  251         }
  252         activity_event_history::Model::record_strava(db, change, &state, previous, None).await?;
  253         strava_events::Model::record_mutation(db, change).await?;
  254         Ok(())
```


---


```
  284                     .await?;
  285                 bike_core::analytics::rebuild_activity_analytics_cache(&txn, &[id]).await?;
  286                 bike_core::entities::activity_training_analyses::Model::clear_for_non_cycling(&txn, &activity).await?;
  287                 if activity.is_bike_activity() {
! 288                     bike_core::activity_training_analysis::rebuild_activity_training_analysis_cache(&txn, &[id])
! 289                         .await?;
  290                 }
  291             }
  292         }
  293         txn.commit().await?;
```


---


```
  295     }
  296 
  297     pub async fn rebuild_fitness(&self, change: ActivityChange) -> Result<ActivityChange, WorkflowError> {
  298         if change.dirty_from.is_some() {
! 299             bike_core::analytics::rebuild_fitness_freshness_cache(&self.db, change.event.bike_user_id).await?;
  300         }
  301         Ok(change)
  302     }
```


---


```
  307     }
  308 
  309     pub async fn publish_heatmap(&self, change: ActivityChange) -> Result<ActivityChange, WorkflowError> {
  310         if let Some(id) = change.bike_activity_id {
! 311             if let Some(projection) =
! 312                 bike_core::entities::heatmap_projections::Model::for_activity(&self.db, id).await?
  313             {
! 314                 bike_core::heatmaps::preparation::prepare_activity(
  315                     &self.db,
! 316                     bike_core::heatmaps::projection::PendingProjection {
  317                         activity_id: id,
  318                         generation: projection.generation,
  319                     },
  320                 )
```


---



## bike-rs/integrations/src/strava/transport&#46;rs

```
  116         .map(|(name, value)| (name.to_string(), STANDARD.encode(value.as_bytes())))
  117         .collect()
  118 }
  119 
! 120 fn storage_error(error: bike_core::workflow_error::WorkflowError) -> reqwest_middleware::Error {
  121     reqwest_middleware::Error::Middleware(anyhow::anyhow!(error.to_string()))
  122 }
  123 
  124 #[cfg(test)]
```


---


