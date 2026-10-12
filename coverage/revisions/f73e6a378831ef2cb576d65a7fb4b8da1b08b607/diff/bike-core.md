# Diff Coverage
## Diff: 13c8f8a2b45bb340cceb7cdff0b3d4fccad2971c...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-core/src/activity_import_pipeline&#46;rs (100%)
- bike-rs/bike-core/src/strava&#46;rs (84.6%): Missing lines 370-372,380-382,396-399,434,437,483,486
- bike-rs/bike-core/src/strava_activity&#46;rs (91.7%): Missing lines 70
- bike-rs/bike-core/src/strava_objects&#46;rs (100%)
- bike-rs/bike-core/src/strava_workflow/activity&#46;rs (90.2%): Missing lines 66,179-180,188,325-326,328-332,382
- bike-rs/bike-core/src/strava_workflow/child&#46;rs (100%)
- bike-rs/bike-core/src/strava_workflow/collection&#46;rs (90.2%): Missing lines 71-73,117,120,172
- bike-rs/bike-core/src/strava_workflow/inbox&#46;rs (100%)
- bike-rs/bike-core/src/strava_workflow/mod&#46;rs (100%)
- bike-rs/bike-core/src/strava_workflow/rebuild&#46;rs (100%)
- bike-rs/bike-core/src/strava_workflow/revocation&#46;rs (91.7%): Missing lines 40
- bike-rs/bike-core/src/strava_workflow/sources&#46;rs (95.3%): Missing lines 56-59
- bike-rs/bike-core/src/strava_workflow/stage&#46;rs (100%)

## Summary

- **Total**: 526 lines
- **Missing**: 38 lines
- **Coverage**: 92.78%



## bike-rs/bike-core/src/strava&#46;rs

```
  366 fn ensure_strava_configured(config: &Config) -> Result<(), AppError> {
  367     if config.strava_enabled() {
  368         Ok(())
  369     } else {
! 370         Err(AppError::bad_request(
! 371             "Strava integration is not configured on this Bike deployment",
! 372         ))
  373     }
  374 }
  375 
  376 fn ensure_strava_webhook_configured(config: &Config) -> Result<(), AppError> {
```


---


```
  376 fn ensure_strava_webhook_configured(config: &Config) -> Result<(), AppError> {
  377     if config.strava_webhook_enabled() {
  378         Ok(())
  379     } else {
! 380         Err(AppError::bad_request(
! 381             "Strava webhook integration is not configured on this Bike deployment",
! 382         ))
  383     }
  384 }
  385 
  386 async fn upsert_connection_from_token(
```


---


```
  392         .filter(strava_connections::Column::AthleteId.eq(token_response.athlete.id))
  393         .one(db)
  394         .await?
  395     {
! 396         if existing_for_athlete.user_id != user_id {
! 397             return Err(AppError::bad_request(
! 398                 "That Strava athlete is already connected to another Bike account",
! 399             ));
  400         }
  401     }
  402 
  403     let expires_at = DateTime::<Utc>::from_timestamp(token_response.expires_at, 0)
```


---


```
  430             athlete_profile_medium_url: Set(token_response.athlete.profile_medium),
  431             scopes: Set(token_response.scope),
  432             access_token: Set(crate::strava_credentials::seal(
  433                 token_response.access_token.as_bytes(),
! 434             )?),
  435             refresh_token: Set(crate::strava_credentials::seal(
  436                 token_response.refresh_token.as_bytes(),
! 437             )?),
  438             expires_at: Set(expires_at),
  439             last_sync_status: Set(STRAVA_SYNC_STATUS_NEVER.to_string()),
  440             last_sync_imported_count: Set(0),
  441             last_sync_duplicate_count: Set(0),
```


---


```
  479         .scope
  480         .unwrap_or_else(|| connection.scopes.clone()));
  481     active_model.access_token = Set(crate::strava_credentials::seal(
  482         token_response.access_token.as_bytes(),
! 483     )?);
  484     active_model.refresh_token = Set(crate::strava_credentials::seal(
  485         token_response.refresh_token.as_bytes(),
! 486     )?);
  487     active_model.expires_at = Set(expires_at);
  488 
  489     if let Some(athlete) = token_response.athlete {
  490         active_model.athlete_id = Set(athlete.id);
```


---



## bike-rs/bike-core/src/strava_activity&#46;rs

```
  66                 imported.affected_segment_ids,
  67                 imported.fitness_dirty_from_day,
  68             ));
  69         }
! 70         Ok(Rebuild::default())
  71     }
  72 }
  73 
  74 async fn update_existing_activity(
```


---



## bike-rs/bike-core/src/strava_workflow/activity&#46;rs

```
  62         }
  63         match self.row.action.as_str() {
  64             "delete" => self.delete().await,
  65             "insert" | "update" | "upsert" => self.upsert().await,
! 66             _ => Err(WorkflowError::bad_request("Unknown Strava workflow action")),
  67         }
  68     }
  69 
  70     async fn upsert(&self) -> Result<(), WorkflowError> {
```


---


```
  175         // Previously retained imports are valid local sources too. Preserve their
  176         // legacy provenance rather than claiming they contain original HTTP bytes.
  177         if let Some(existing) = self.legacy_payload(activity.id).await? {
  178             if stream_revision(&existing.activity)? == revision {
! 179                 return self
! 180                     .store
  181                     .transform(
  182                         &format!(
  183                             "strava/{}/{}/transforms/legacy-streams/v1/{revision}",
  184                             self.row.athlete_id, activity.id
```


---


```
  184                             self.row.athlete_id, activity.id
  185                         ),
  186                         &existing.streams,
  187                     )
! 188                     .await;
  189             }
  190         }
  191         self.fetch_source(archive, |token| async move {
  192             self.client.get_activity_streams(&token, activity.id).await
```


---


```
  321                     Some(&profile),
  322                 ),
  323             )
  324             .await?;
! 325         Ok(Rebuild::import(
! 326             import_id,
  327             result.affected_segment_ids,
! 328             activity
! 329                 .started_at
! 330                 .date_naive()
! 331                 .min(result.fitness_dirty_from_day),
! 332         ))
  333     }
  334 
  335     async fn delete(&self) -> Result<(), WorkflowError> {
  336         let capture = self
```


---


```
  378                     self.connection.user_id,
  379                     activity.clone(),
  380                 )
  381                 .await?;
! 382             }
  383         }
  384         self.save_state(None, true).await?;
  385         Ok(Effect {
  386             outcome: if capture.activity.is_some() {
```


---



## bike-rs/bike-core/src/strava_workflow/collection&#46;rs

```
  67     }
  68 
  69     fn after(&self) -> i64 {
  70         self.source.after.unwrap_or_else(|| {
! 71             (self.row.accepted_at - chrono::Duration::days(super::INITIAL_BACKFILL_DAYS))
! 72                 .timestamp()
! 73         })
  74     }
  75 
  76     async fn pull_pages(&self) -> Result<Vec<DiscoveryPage>, WorkflowError> {
  77         let mut pages = Vec::new();
```


---


```
  113                     .get("start_date")
  114                     .and_then(serde_json::Value::as_str)
  115                     .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
  116                     .ok_or_else(|| {
! 117                         WorkflowError::bad_request(
  118                             "Strava listing activity has no valid start date",
  119                         )
! 120                     })?
  121                     .with_timezone(&chrono::Utc);
  122                 if started_at.timestamp() <= self.after() {
  123                     continue;
  124                 }
```


---


```
  168             .as_object()
  169             .and_then(|stages| stages.iter().find(|(key, _)| key.starts_with("apply:")))
  170             .map(|(_, value)| value)
  171         else {
! 172             return Ok(None);
  173         };
  174         let object = serde_json::from_value(saved.clone()).map_err(json_error)?;
  175         self.store.read_json(&object).await.map(Some)
  176     }
```


---



## bike-rs/bike-core/src/strava_workflow/revocation&#46;rs

```
  36             let archive = self.archive(None, "", "deauthorization");
  37             if archive.store.response(&archive.key).await?.is_none() {
  38                 let token = self.access_token().await?;
  39                 archive.scope(self.client.deauthorize(&token)).await?;
! 40             }
  41         }
  42         Ok(serde_json::json!({"outcome":"revoked","requested_remote":self.source.revoke_remote}))
  43     }
```


---



## bike-rs/bike-core/src/strava_workflow/sources&#46;rs

```
  52     }
  53 
  54     pub(super) async fn access_token(&self) -> Result<String, WorkflowError> {
  55         if !super::provider_work_enabled() {
! 56             return Err(WorkflowError::too_many_requests(
! 57                 "Strava provider work is paused for cutover",
! 58                 Some(chrono::Utc::now() + chrono::Duration::minutes(1)),
! 59             ));
  60         }
  61         // Child stages share the connection lease, but may have refreshed it already.
  62         let connection = strava_connections::Entity::find_by_id(self.connection.id)
  63             .one(self.db)
```


---


