# Diff Coverage
## Diff: 3b8291e82ba259a28a05e4b3dc13ddb9dd759efb...HEAD, staged, unstaged and untracked changes

- bike-rs/integrations/src/bin/recover-generated-imports&#46;rs (0.0%): Missing lines 138
- bike-rs/integrations/src/strava/connection&#46;rs (0.0%): Missing lines 158-163
- bike-rs/integrations/src/strava/events/intake&#46;rs (100%)
- bike-rs/integrations/src/strava/events/submission&#46;rs (99.0%): Missing lines 55

## Summary

- **Total**: 135 lines
- **Missing**: 8 lines
- **Coverage**: 94.07%



## bike-rs/integrations/src/bin/recover-generated-imports&#46;rs

```
  134         let user = bike_core::auth::entities::users::Model::find_by_id(&db, run.user_id).await?;
  135         let connection = strava::load_connection(&db, run.user_id)
  136             .await?
  137             .ok_or("Connect Strava first")?;
! 138         strava::events::request_sync(&strava::events::StravaIntake::new(db.clone()), &user, &connection).await?;
  139     }
  140     println!(
  141         "{}",
  142         serde_json::json!({"next_after_id": last_id, "applied": run.apply, "failed": failed, "recent_refresh_requested": run.refresh_recent && recent_gaps})
```


---



## bike-rs/integrations/src/strava/connection&#46;rs

```
  154 
  155         let connection = upsert_connection_from_token(db, verified_state.user_id, token_response).await?;
  156 
  157         let user = bike_core::auth::entities::users::Model::find_by_id(db, connection.user_id).await?;
! 158         crate::strava::events::request_sync(
! 159             &crate::strava::events::StravaIntake::new(db.clone()),
! 160             &user,
! 161             &connection,
! 162         )
! 163         .await?;
  164         Ok::<_, AppError>(connection)
  165     }
  166     .await;
```


---



## bike-rs/integrations/src/strava/events/submission&#46;rs

```
  51 pub async fn connect_with_options(options: ConnectionOptions, namespace: &str) -> Result<Client, Error> {
  52     Ok(Client::new(
  53         Connection::connect(options).await?,
  54         ClientOptions::new(namespace.to_owned()).build(),
! 55     )?)
  56 }
  57 
  58 pub fn search_attributes(event: &StravaEvent) -> SearchAttributes {
  59     let mut values = vec![
```


---


