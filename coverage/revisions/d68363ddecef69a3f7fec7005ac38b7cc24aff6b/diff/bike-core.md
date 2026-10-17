# Diff Coverage
## Diff: 8feed897db4c26335b6c9c3486bd3eb738d585f1...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-core/src/bin/recover-generated-imports&#46;rs (0.0%): Missing lines 134-138
- bike-rs/bike-core/src/entities/strava_activity_states&#46;rs (100%)
- bike-rs/bike-core/src/strava/connection&#46;rs (0.0%): Missing lines 178-180
- bike-rs/bike-core/src/strava/events&#46;rs (100%)
- bike-rs/bike-core/src/strava/events/intake&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/dependencies&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/mod&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/persistence&#46;rs (95.2%): Missing lines 154
- bike-rs/bike-core/src/test_support/strava&#46;rs (100%)

## Summary

- **Total**: 266 lines
- **Missing**: 9 lines
- **Coverage**: 96.62%



## bike-rs/bike-core/src/bin/recover-generated-imports&#46;rs

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



## bike-rs/bike-core/src/strava/connection&#46;rs

```
  174         }
  175 
  176         let connection = upsert_connection_from_token(db, verified_state.user_id, token_response).await?;
  177 
! 178         let user = crate::auth::entities::users::Model::find_by_id(db, connection.user_id).await?;
! 179         crate::strava::events::request_sync(db, &user, &connection).await?;
! 180         Ok::<_, AppError>(connection)
  181     }
  182     .await;
  183 
  184     match result {
```


---



## bike-rs/bike-core/src/strava/native/persistence&#46;rs

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


