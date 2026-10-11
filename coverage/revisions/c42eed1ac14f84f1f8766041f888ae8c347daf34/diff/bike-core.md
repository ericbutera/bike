# Diff Coverage
## Diff: 7094b5d003751e97b6e714459a2fad50d7a0c6fd...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-core/src/activity_import_pipeline/stage_execution&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/execution&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/worker/catalog&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/worker/task_worker&#46;rs (16.7%): Missing lines 162-166
- bike-rs/bike-core/src/bin/recover-generated-imports&#46;rs (0.0%): Missing lines 141-144
- bike-rs/bike-core/src/entities/activities&#46;rs (100%)
- bike-rs/bike-core/src/entities/activity_import_artifacts&#46;rs (100%)
- bike-rs/bike-core/src/entities/strava_activity_states&#46;rs (100%)
- bike-rs/bike-core/src/entities/strava_connections&#46;rs (100%)
- bike-rs/bike-core/src/entities/strava_workflows&#46;rs (98.7%): Missing lines 56
- bike-rs/bike-core/src/jobs/adapter&#46;rs (100%)
- bike-rs/bike-core/src/strava&#46;rs (45.3%): Missing lines 355,1436-1442,1454-1459,1482-1495,1503-1508,1837
- bike-rs/bike-core/src/strava_activity&#46;rs (95.9%): Missing lines 74,130,144-146
- bike-rs/bike-core/src/strava_client&#46;rs (70.5%): Missing lines 135,164-185,193-200,215-221,262-266,279-285,306-307,309-310,312,314-317,363,455,490,541,544
- bike-rs/bike-core/src/strava_credentials&#46;rs (86.7%): Missing lines 11-13,23-25
- bike-rs/bike-core/src/strava_objects&#46;rs (92.8%): Missing lines 50,54,85,108-110,123,127,165,193,262-263,282-287
- bike-rs/bike-core/src/strava_transport&#46;rs (94.4%): Missing lines 82-86,114-116,177
- bike-rs/bike-core/src/strava_workflow/activity&#46;rs (84.8%): Missing lines 113,129-130,149-151,172-183,200,206,209,231,261,305-315,317-343,374,401
- bike-rs/bike-core/src/strava_workflow/child&#46;rs (97.9%): Missing lines 59
- bike-rs/bike-core/src/strava_workflow/collection&#46;rs (87.7%): Missing lines 35-37,39,41,43-46,70,100,102-104,116-117,138,154
- bike-rs/bike-core/src/strava_workflow/import_objects&#46;rs (100%)
- bike-rs/bike-core/src/strava_workflow/inbox&#46;rs (97.6%): Missing lines 28,33,88,145,173,183,227,310
- bike-rs/bike-core/src/strava_workflow/intents&#46;rs (91.9%): Missing lines 48-50
- bike-rs/bike-core/src/strava_workflow/mod&#46;rs (94.1%): Missing lines 63,84-87,99,124-125
- bike-rs/bike-core/src/strava_workflow/revocation&#46;rs (96.2%): Missing lines 29,44-45
- bike-rs/bike-core/src/strava_workflow/stage&#46;rs (97.6%): Missing lines 164,195-197

## Summary

- **Total**: 2403 lines
- **Missing**: 255 lines
- **Coverage**: 89.39%



## bike-rs/bike-core/src/background_jobs/worker/task_worker&#46;rs

```
  158             task.attempt = task.attempts,
  159             pipeline_run_id = origin.map(|context| context.run_id.as_str()),
  160             pipeline_started_at = origin.map(|context| context.pipeline_started_at.to_rfc3339()),
  161             request_id = origin.and_then(|context| context.request_id.as_deref()),
! 162             correlation_id = strava["correlation_id"].as_str(),
! 163             workflow_id = strava["workflow_id"].as_str(),
! 164             parent_workflow_id = strava["parent_workflow_id"].as_str(),
! 165             strava_activity_id = strava["strava_activity_id"].as_i64(),
! 166             athlete_id = strava["athlete_id"].as_i64(),
  167             trace_id = tracing::field::Empty,
  168             span_id = tracing::field::Empty,
  169             task.status = tracing::field::Empty,
  170             "otel.status_code" = tracing::field::Empty,
```


---



## bike-rs/bike-core/src/bin/recover-generated-imports&#46;rs

```
  137             }
  138         }
  139     }
  140     if run.refresh_recent && recent_gaps {
! 141         let connection = bike_core::strava::load_connection(&db, run.user_id)
! 142             .await?
! 143             .ok_or("Strava connection was not found")?;
! 144         strava_workflow::queue_sync(&db, &connection, false).await?;
  145     }
  146     println!(
  147         "{}",
  148         serde_json::json!({"next_after_id": last_id, "applied": run.apply, "failed": failed, "recent_refresh_requested": run.refresh_recent && recent_gaps})
```


---



## bike-rs/bike-core/src/entities/strava_workflows&#46;rs

```
  52                 let at: DateTimeUtc = serde_json::from_value(pause["retry_at"].clone())
  53                     .map_err(|error| DbErr::Custom(error.to_string()))?;
  54                 if at > chrono::Utc::now() {
  55                     return Ok(Some(pause.clone()));
! 56                 }
  57             }
  58         }
  59         Ok(None)
  60     }
```


---



## bike-rs/bike-core/src/strava&#46;rs

```
  351 
  352 pub async fn ensure_webhook_subscription_registered(
  353     db: &DatabaseConnection,
  354 ) -> Result<(), AppError> {
! 355     if !crate::strava_workflow::provider_work_enabled() {
  356         return Ok(());
  357     }
  358     ensure_webhook_subscription(db, Config::get()).await
  359 }
```


---


```
  1432         active_model.athlete_first_name = Set(token_response.athlete.firstname);
  1433         active_model.athlete_last_name = Set(token_response.athlete.lastname);
  1434         active_model.athlete_profile_medium_url = Set(token_response.athlete.profile_medium);
  1435         active_model.scopes = Set(token_response.scope);
! 1436         active_model.access_token = Set(crate::strava_credentials::seal(
! 1437             token_response.access_token.as_bytes(),
! 1438         )?);
! 1439         active_model.refresh_token = Set(crate::strava_credentials::seal(
! 1440             token_response.refresh_token.as_bytes(),
! 1441         )?);
! 1442         active_model.created_at = Set(Utc::now());
  1443         active_model.expires_at = Set(expires_at);
  1444         active_model.update(db).await.map_err(AppError::from)
  1445     } else {
  1446         strava_connections::ActiveModel {
```


---


```
  1450             athlete_first_name: Set(token_response.athlete.firstname),
  1451             athlete_last_name: Set(token_response.athlete.lastname),
  1452             athlete_profile_medium_url: Set(token_response.athlete.profile_medium),
  1453             scopes: Set(token_response.scope),
! 1454             access_token: Set(crate::strava_credentials::seal(
! 1455                 token_response.access_token.as_bytes(),
! 1456             )?),
! 1457             refresh_token: Set(crate::strava_credentials::seal(
! 1458                 token_response.refresh_token.as_bytes(),
! 1459             )?),
  1460             expires_at: Set(expires_at),
  1461             last_sync_status: Set(STRAVA_SYNC_STATUS_NEVER.to_string()),
  1462             last_sync_imported_count: Set(0),
  1463             last_sync_duplicate_count: Set(0),
```


---


```
  1478     if connection.expires_at > Utc::now() + Duration::minutes(5) {
  1479         return Ok(connection);
  1480     }
  1481 
! 1482     let refresh_token = crate::strava_credentials::open(&connection.refresh_token)?;
! 1483     let archive = crate::strava_client::RequestArchive {
! 1484         store: crate::strava_objects::ObjectStore::new(&Config::get().uploads_dir),
! 1485         key: format!(
! 1486             "strava/{}/credentials/{}/refresh/{}",
! 1487             connection.athlete_id,
! 1488             connection.created_at.timestamp_micros(),
! 1489             crate::strava_objects::checksum(
! 1490                 format!("{}:{}", refresh_token, connection.expires_at.timestamp()).as_bytes()
! 1491             )
! 1492         ),
! 1493     };
! 1494     let token_response = archive
! 1495         .scope(client.refresh_access_token(&refresh_token))
  1496         .await?;
  1497     let expires_at = DateTime::<Utc>::from_timestamp(token_response.expires_at, 0)
  1498         .ok_or_else(|| AppError::internal("Strava returned an invalid token expiration"))?;
  1499     let mut active_model: strava_connections::ActiveModel = connection.clone().into();
```


---


```
  1499     let mut active_model: strava_connections::ActiveModel = connection.clone().into();
  1500     active_model.scopes = Set(token_response
  1501         .scope
  1502         .unwrap_or_else(|| connection.scopes.clone()));
! 1503     active_model.access_token = Set(crate::strava_credentials::seal(
! 1504         token_response.access_token.as_bytes(),
! 1505     )?);
! 1506     active_model.refresh_token = Set(crate::strava_credentials::seal(
! 1507         token_response.refresh_token.as_bytes(),
! 1508     )?);
  1509     active_model.expires_at = Set(expires_at);
  1510 
  1511     if let Some(athlete) = token_response.athlete {
  1512         active_model.athlete_id = Set(athlete.id);
```


---


```
  1833     {
  1834         return Ok(ResolvedStravaConnectionSyncState {
  1835             connection: connection.clone(),
  1836             active_sync_status: Some(if run.status == "running" {
! 1837                 STRAVA_SYNC_STATUS_RUNNING
  1838             } else {
  1839                 STRAVA_SYNC_STATUS_QUEUED
  1840             }),
  1841         });
```


---



## bike-rs/bike-core/src/strava_activity&#46;rs

```
  70                 imported.affected_segment_ids,
  71                 Some(imported.fitness_dirty_from_day),
  72             )
  73             .await?;
! 74         }
  75         Ok(())
  76     }
  77 }
```


---


```
  126     let relative_path = parent.join(format!("strava_{}.json", Uuid::new_v4()));
  127     let full_path = Path::new(uploads_dir).join(&relative_path);
  128     if let Some(parent) = full_path.parent() {
  129         tokio::fs::create_dir_all(parent).await?;
! 130     }
  131     tokio::fs::write(&full_path, &payload.bytes).await?;
  132     let transaction = db.begin().await?;
  133     let artifact = activity_import_artifacts::Entity::store_strava_payload(
  134         &transaction,
```


---


```
  140     if !activity_import
  141         .promote_source(&transaction, &artifact)
  142         .await?
  143     {
! 144         return Err(AppError::bad_request(
! 145             "Strava import changed during payload replacement",
! 146         ));
  147     }
  148     transaction.commit().await?;
  149     activity_imports::Entity::find_owned(db, activity_import.user_id, activity_import.id)
  150         .await?
```


---



## bike-rs/bike-core/src/strava_client&#46;rs

```
  131                 .build(),
  132             db: db.clone(),
  133             config,
  134             base_url: if config.environment == "production" {
! 135                 "https://www.strava.com".into()
  136             } else {
  137                 std::env::var("STRAVA_API_ORIGIN")
  138                     .unwrap_or_else(|_| "https://www.strava.com".into())
  139             },
```


---


```
  160     pub(crate) async fn exchange_authorization_code(
  161         &self,
  162         code: &str,
  163     ) -> Result<StravaAuthorizationTokenResponse, AppError> {
! 164         let archive = RequestArchive {
! 165             store: ObjectStore::new(&self.config.uploads_dir),
! 166             key: format!(
! 167                 "strava/credentials/authorization/{}",
! 168                 crate::strava_objects::checksum(code.as_bytes())
! 169             ),
! 170         };
! 171         archive
! 172             .scope(self.send_json(
! 173                 strava_api::oauth_token(
! 174                     &self.oauth_sdk(None),
! 175                     &self.config.strava_client_id,
! 176                     &self.config.strava_client_secret,
! 177                     "authorization_code",
! 178                     Some(code),
! 179                     None,
! 180                 ),
! 181                 StravaRequestClass::OverallOnly,
! 182                 "exchange_authorization_code",
! 183                 "exchange a Strava authorization code",
! 184             ))
! 185             .await
  186     }
  187 
  188     pub(crate) async fn refresh_access_token(
  189         &self,
```


---


```
  189         &self,
  190         refresh_token: &str,
  191     ) -> Result<StravaRefreshTokenResponse, AppError> {
  192         self.send_json(
! 193             strava_api::oauth_token(
! 194                 &self.oauth_sdk(None),
! 195                 &self.config.strava_client_id,
! 196                 &self.config.strava_client_secret,
! 197                 "refresh_token",
! 198                 None,
! 199                 Some(refresh_token),
! 200             ),
  201             StravaRequestClass::OverallOnly,
  202             "refresh_access_token",
  203             "refresh a Strava access token",
  204         )
```


---


```
  211         after_epoch: Option<i64>,
  212         page: usize,
  213         per_page: usize,
  214     ) -> Result<Vec<StravaActivitySummary>, AppError> {
! 215         let configuration = self.sdk(Some(access_token));
! 216         let request = activities_api::get_logged_in_athlete_activities(
! 217             &configuration,
! 218             None,
! 219             after_epoch.map(|value| value as i32),
! 220             Some(page as i32),
! 221             Some(per_page as i32),
  222         );
  223 
  224         self.send_json(
  225             request,
```


---


```
  258     pub(crate) async fn list_push_subscriptions(
  259         &self,
  260     ) -> Result<Vec<StravaPushSubscription>, AppError> {
  261         self.send_json(
! 262             strava_api::list_push_subscriptions(
! 263                 &self.sdk(None),
! 264                 &self.config.strava_client_id,
! 265                 &self.config.strava_client_secret,
! 266             ),
  267             StravaRequestClass::Read,
  268             "list_push_subscriptions",
  269             "list Strava webhook subscriptions",
  270         )
```


---


```
  275         &self,
  276     ) -> Result<StravaPushSubscription, AppError> {
  277         let callback_url = self.config.strava_webhook_callback_url();
  278         self.send_json(
! 279             strava_api::create_push_subscription(
! 280                 &self.sdk(None),
! 281                 &self.config.strava_client_id,
! 282                 &self.config.strava_client_secret,
! 283                 &callback_url,
! 284                 &self.config.strava_webhook_verify_token,
! 285             ),
  286             StravaRequestClass::OverallOnly,
  287             "create_push_subscription",
  288             "create a Strava webhook subscription",
  289         )
```


---


```
  302         R: DeserializeOwned,
  303     {
  304         if let Some(archive) = RequestArchive::current() {
  305             if let Some(saved) = archive.store.response(&archive.key).await? {
! 306                 tracing::info!(operation, object_path = %saved.body.path, outcome = "reused", "Reusing saved Strava response");
! 307                 let bytes = archive.store.response_bytes(&saved).await?;
  308                 // Replays honor the generated response model as live calls do.
! 309                 let _: R = decode_response(&bytes, action)?;
! 310                 return decode_response(&bytes, action);
  311             }
! 312         }
  313         if !crate::strava_workflow::provider_work_enabled() {
! 314             return Err(AppError::too_many_requests(
! 315                 "Strava provider work is paused for cutover",
! 316                 Some(Utc::now() + Duration::minutes(1)),
! 317             ));
  318         }
  319         let span = tracing::info_span!(
  320             "strava.client.call",
  321             "otel.kind" = "client",
```


---


```
  359                     {
  360                         http_span.record("retry_at", field::display(retry_at));
  361                     }
  362                 }
! 363                 None => {
  364                     http_span.record("status", "transport_error");
  365                 }
  366             }
  367             self.parse_json_response(
```


---


```
  451                 operation,
  452                 request_class.as_str(),
  453                 "transport_error",
  454             );
! 455             tracing::error!(
  456                 action,
  457                 operation,
  458                 "Generated Strava SDK request failed; inspect the retained exchange"
  459             );
```


---


```
  486             ));
  487         }
  488 
  489         if !sdk_accepted {
! 490             return Err(AppError::internal(format!("Generated Strava SDK rejected the response while trying to {action}; inspect the retained exchange")));
  491         }
  492         decode_response(&bytes, action)
  493     }
```


---


```
  537 }
  538 
  539 fn decode_response<T: DeserializeOwned>(bytes: &[u8], action: &str) -> Result<T, AppError> {
  540     serde_json::from_slice(bytes).map_err(|error| {
! 541         AppError::internal(format!(
  542             "Failed to parse saved Strava response while trying to {action}: {error}"
  543         ))
! 544     })
  545 }
  546 
  547 pub(crate) async fn retain_response(
  548     store: &ObjectStore,
```


---



## bike-rs/bike-core/src/strava_credentials&#46;rs

```
   7 use base64::{engine::general_purpose::STANDARD, Engine};
   8 
   9 fn cipher() -> Result<Aes256Gcm, WorkflowError> {
  10     let key = match std::env::var("STRAVA_TOKEN_ENCRYPTION_KEY") {
! 11         Ok(key) => STANDARD
! 12             .decode(key)
! 13             .map_err(|_| WorkflowError::internal("Invalid Strava credential encryption key"))?,
  14         Err(_) if Config::get().environment != "production" => {
  15             use sha2::Digest;
  16             sha2::Sha256::digest(format!(
  17                 "bike:strava:credentials:{}",
```


---


```
  19             ))
  20             .to_vec()
  21         }
  22         Err(_) => {
! 23             return Err(WorkflowError::internal(
! 24                 "STRAVA_TOKEN_ENCRYPTION_KEY is required",
! 25             ))
  26         }
  27     };
  28     Aes256Gcm::new_from_slice(&key)
  29         .map_err(|_| WorkflowError::internal("Strava credential key must contain 32 bytes"))
```


---



## bike-rs/bike-core/src/strava_objects&#46;rs

```
  46 
  47     pub async fn read(&self, object: &ObjectRef) -> Result<Vec<u8>, WorkflowError> {
  48         let path = self.path(&object.path)?;
  49         let bytes = fs::read(path).await.map_err(|error| {
! 50             WorkflowError::internal(format!(
  51                 "Saved Strava object {} is unavailable: {error}",
  52                 object.path
  53             ))
! 54         })?;
  55         if checksum(&bytes) != object.sha256 || bytes.len() != object.bytes {
  56             return Err(WorkflowError::internal(format!(
  57                 "Saved Strava object {} failed checksum verification",
  58                 object.path
```


---


```
  81     pub async fn reference(&self, key: &str) -> Result<Option<ObjectRef>, WorkflowError> {
  82         let bytes = match fs::read(self.path(&format!("{key}/reference.json"))?).await {
  83             Ok(bytes) => bytes,
  84             Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
! 85             Err(error) => return Err(io_error(error)),
  86         };
  87         let reference: ObjectRef = serde_json::from_slice(&bytes).map_err(json_error)?;
  88         self.read(&reference).await?;
  89         Ok(Some(reference))
```


---


```
  104             .reference(key)
  105             .await?
  106             .ok_or_else(|| WorkflowError::internal("Strava reference was not retained"))?;
  107         if saved.sha256 != reference.sha256 {
! 108             return Err(WorkflowError::conflict(
! 109                 "Saved Strava source reference is immutable",
! 110             ));
  111         }
  112         Ok(())
  113     }
```


---


```
  119             Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
  120                 let capture = match fs::read(self.path(&format!("{key}/capture.json"))?).await {
  121                     Ok(bytes) => serde_json::from_slice(&bytes).map_err(json_error)?,
  122                     Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
! 123                     Err(error) => return Err(io_error(error)),
  124                 };
  125                 return self.materialize(key, capture).await.map(Some);
  126             }
! 127             Err(error) => return Err(io_error(error)),
  128         };
  129         let response: ResponseObject = serde_json::from_slice(&bytes).map_err(json_error)?;
  130         self.read(&response.body).await?;
  131         Ok(Some(response))
```


---


```
  161         bytes: &[u8],
  162         encrypted: bool,
  163     ) -> Result<ResponseObject, WorkflowError> {
  164         if let Some(saved) = self.response(key).await? {
! 165             return Ok(saved);
  166         }
  167         use base64::Engine;
  168         let capture = Capture {
  169             status,
```


---


```
  189             let ciphertext = String::from_utf8(bytes)
  190                 .map_err(|_| WorkflowError::internal("Invalid encrypted Strava response"))?;
  191             crate::strava_credentials::open(&ciphertext).map(String::into_bytes)
  192         } else {
! 193             Ok(bytes)
  194         }
  195     }
  196 
  197     async fn materialize(
```


---


```
  258         file.sync_all().await.map_err(io_error)?;
  259         // Linking publishes a complete file without replacing an existing object.
  260         match fs::hard_link(&temporary, path).await {
  261             Ok(()) => Ok(()),
! 262             Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
! 263             Err(error) => Err(io_error(error)),
  264         }
  265     }
  266     .await;
  267     let cleanup = fs::remove_file(&temporary).await;
```


---


```
  278     .map_err(io_error)?;
  279     Ok(())
  280 }
  281 
! 282 fn io_error(error: std::io::Error) -> WorkflowError {
! 283     WorkflowError::internal(error.to_string())
! 284 }
! 285 fn json_error(error: serde_json::Error) -> WorkflowError {
! 286     WorkflowError::internal(error.to_string())
! 287 }
  288 
  289 #[cfg(test)]
  290 mod tests {
  291     use super::*;
```


---



## bike-rs/bike-core/src/strava_transport&#46;rs

```
  78             .await
  79             .map_err(storage_error)?;
  80         RESPONSE
  81             .try_with(|slot| *slot.borrow_mut() = Some(raw))
! 82             .map_err(|_| {
! 83                 reqwest_middleware::Error::Middleware(anyhow::anyhow!(
! 84                     "Strava SDK call has no capture context"
! 85                 ))
! 86             })?;
  87         Ok(http::Response::from_parts(parts, reqwest::Body::from(bytes)).into())
  88     }
  89 }
```


---


```
  110         .map(|(name, value)| (name.to_string(), STANDARD.encode(value.as_bytes())))
  111         .collect()
  112 }
  113 
! 114 fn storage_error(error: crate::workflow_error::WorkflowError) -> reqwest_middleware::Error {
! 115     reqwest_middleware::Error::Middleware(anyhow::anyhow!(error.to_string()))
! 116 }
  117 
  118 #[cfg(test)]
  119 mod tests {
  120     use super::*;
```


---


```
  173             .route("/api/v3/activities/:id", get(activity))
  174             .route("/oauth/token", post(oauth));
  175         let server = tokio::spawn(async {
  176             axum::serve(listener, app).await.unwrap();
! 177         });
  178         let config = Configuration {
  179             client,
  180             base_path: format!("{origin}/api/v3"),
  181             oauth_access_token: Some("fixture-access".into()),
```


---



## bike-rs/bike-core/src/strava_workflow/activity&#46;rs

```
  109                         .ok_or_else(|| WorkflowError::internal("Apply stage is missing"))?;
  110                     self.stage(node, &self.row.id, async { Ok(effect.clone()) })
  111                         .await?;
  112                 }
! 113                 _ => return Err(WorkflowError::internal("Unsupported activity DAG stage")),
  114             }
  115         }
  116         Ok(())
  117     }
```


---


```
  125                 .transpose()
  126                 .map_err(json_error)?
  127                 .unwrap_or_else(|| serde_json::json!({}));
  128             let fields = activity.as_object().ok_or_else(|| {
! 129                 WorkflowError::bad_request("Strava activity summary is not an object")
! 130             })?;
  131             for (key, field) in fields {
  132                 value[key] = field.clone();
  133             }
  134             serde_json::from_value(value).map_err(json_error)?
```


---


```
  145             .scope(self.client.get_activity(&self.token, id))
  146             .await?
  147         };
  148         if activity.id != id {
! 149             return Err(WorkflowError::bad_request(
! 150                 "Strava response activity ID does not match the workflow",
! 151             ));
  152         }
  153         Ok(activity)
  154     }
```


---


```
  168         }
  169         // Previously retained imports are valid local sources too. Preserve their
  170         // legacy provenance rather than claiming they contain original HTTP bytes.
  171         if let Some(existing) = self.legacy_payload(activity.id).await? {
! 172             if stream_revision(&existing.activity)? == revision {
! 173                 self.store
! 174                     .transform(
! 175                         &format!(
! 176                             "strava/{}/{}/transforms/legacy-streams/v1/{revision}",
! 177                             self.row.athlete_id, activity.id
! 178                         ),
! 179                         &existing.streams,
! 180                     )
! 181                     .await?;
! 182                 return Ok(existing.streams);
! 183             }
  184         }
  185         archive
  186             .scope(self.client.get_activity_streams(&self.token, activity.id))
  187             .await
```


---


```
  196         else {
  197             return Ok(None);
  198         };
  199         let Some(import_id) = activity.activity_import_id else {
! 200             return Ok(None);
  201         };
  202         let Some(import) =
  203             activity_imports::Entity::find_owned(self.db, self.connection.user_id, import_id)
  204                 .await?
```


---


```
  202         let Some(import) =
  203             activity_imports::Entity::find_owned(self.db, self.connection.user_id, import_id)
  204                 .await?
  205         else {
! 206             return Ok(None);
  207         };
  208         if import.format != "json" {
! 209             return Ok(None);
  210         }
  211         let bytes = tokio::fs::read(
  212             std::path::Path::new(&crate::config::Config::get().uploads_dir)
  213                 .join(import.storage_path),
```


---


```
  227             .is_some()
  228             {
  229                 self.delete().await?;
  230             } else {
! 231                 self.save_state(Some(hash.into()), false).await?;
  232             }
  233             return Ok(Effect {
  234                 outcome: "excluded".into(),
  235                 hash: None,
```


---


```
  257                 });
  258                 if !changed {
  259                     self.resume_import(&activity, import_id).await?;
  260                 }
! 261             }
  262         }
  263         if changed {
  264             let objects = super::import_objects::ImportObjects {
  265                 store: self.store.clone(),
```


---


```
  301                 .await?
  302                 .ok_or_else(|| WorkflowError::internal("Strava import record was not found"))?;
  303         if import.status == crate::activity_import_lifecycle::ACTIVITY_IMPORT_STATUS_PROCESSED {
  304             return Ok(());
! 305         }
! 306         let profile =
! 307             crate::training_profile::load_training_profile(self.db, self.connection.user_id)
! 308                 .await?;
! 309         let objects = super::import_objects::ImportObjects {
! 310             store: self.store.clone(),
! 311             athlete_id: self.row.athlete_id,
! 312             activity_id: self
! 313                 .row
! 314                 .activity_id
! 315                 .ok_or_else(|| WorkflowError::internal("Missing activity correlation ID"))?,
  316         };
! 317         let result = objects
! 318             .scope(
! 319                 crate::activity_import_pipeline::reprocess_activity_from_import(
! 320                     self.db,
! 321                     &crate::config::Config::get().uploads_dir,
! 322                     self.connection.user_id,
! 323                     activity.clone(),
! 324                     import,
! 325                     Some(&profile),
! 326                 ),
! 327             )
! 328             .await?;
! 329         crate::activity_import_lifecycle::complete_activity_imports(
! 330             self.db,
! 331             &JobQueue::new(self.db.clone()),
! 332             self.connection.user_id,
! 333             &[import_id],
! 334             result.affected_segment_ids,
! 335             Some(
! 336                 activity
! 337                     .started_at
! 338                     .date_naive()
! 339                     .min(result.fitness_dirty_from_day),
! 340             ),
! 341         )
! 342         .await?;
! 343         Ok(())
  344     }
  345 
  346     async fn delete(&self) -> Result<(), WorkflowError> {
  347         let mut capture: Option<DeleteCapture> = None;
```


---


```
  370                                     self.connection.user_id,
  371                                     activity.clone(),
  372                                 )
  373                                 .await?;
! 374                             }
  375                         }
  376                         self.save_state(None, true).await?;
  377                         Ok(Effect {
  378                             outcome: if capture.activity.is_some() {
```


---


```
  397                         .ok_or_else(|| WorkflowError::internal("Missing delete checkpoint"))?;
  398                     self.stage("delete_derived", &self.row.id, self.delete_derived(capture))
  399                         .await?;
  400                 }
! 401                 _ => return Err(WorkflowError::internal("Unsupported delete DAG stage")),
  402             }
  403         }
  404         Ok(())
  405     }
```


---



## bike-rs/bike-core/src/strava_workflow/child&#46;rs

```
  55                 result.as_ref().err().map(|error| error.message.clone()),
  56             )
  57             .await?;
  58             result?;
! 59         }
  60         strava_workflows::Entity::find_by_id(id)
  61             .one(self.db)
  62             .await?
  63             .ok_or_else(|| WorkflowError::internal("Strava child workflow disappeared"))
```


---



## bike-rs/bike-core/src/strava_workflow/collection&#46;rs

```
  31         )));
  32         running.update(self.db).await?;
  33         let result = self.collection_graph().await;
  34         if let Err(error) = &result {
! 35             let mut failed: strava_connections::ActiveModel = self.connection.clone().into();
! 36             failed.last_sync_status = Set(if error.status
! 37                 == axum::http::StatusCode::TOO_MANY_REQUESTS
  38             {
! 39                 "queued"
  40             } else {
! 41                 "failed"
  42             }
! 43             .into());
! 44             failed.last_sync_message = Set(Some(error.message.clone()));
! 45             failed.last_sync_failed_count = Set(1);
! 46             failed.update(self.db).await?;
  47         }
  48         result
  49     }
```


---


```
  66                         .ok_or_else(|| WorkflowError::internal("Missing children checkpoint"))?;
  67                     self.stage(node, &self.row.id, self.complete(summary))
  68                         .await?;
  69                 }
! 70                 _ => return Err(WorkflowError::internal("Unsupported collection DAG stage")),
  71             }
  72         }
  73         Ok(())
  74     }
```


---


```
   96                 raw: saved.body,
   97             });
   98             if finished {
   99                 return Ok(pages);
! 100             }
  101         }
! 102         Err(WorkflowError::internal(
! 103             "Strava backfill exceeded its page limit",
! 104         ))
  105     }
  106 
  107     async fn children(&self, pages: &[DiscoveryPage]) -> Result<Summary, WorkflowError> {
  108         let mut summary = Summary::default();
```


---


```
  112                 let id = value
  113                     .get("id")
  114                     .and_then(serde_json::Value::as_i64)
  115                     .ok_or_else(|| {
! 116                         WorkflowError::bad_request("Strava listing activity has no ID")
! 117                     })?;
  118                 let hash = checksum(&serde_json::to_vec(value).map_err(json_error)?);
  119                 let child = format!("{}-{id}-{hash}", self.row.id);
  120                 let source = Source {
  121                     revoke_remote: false,
```


---


```
  134                     .map(|at| at.with_timezone(&chrono::Utc));
  135                 summary.latest = summary.latest.max(latest);
  136                 let applied = self.child_applied(&completed).await?;
  137                 if applied {
! 138                     summary.imported += 1;
  139                 } else {
  140                     summary.unchanged += 1;
  141                 }
  142             }
```


---


```
  150             .as_object()
  151             .and_then(|stages| stages.iter().find(|(key, _)| key.starts_with("apply:")))
  152             .map(|(_, value)| value)
  153         else {
! 154             return Ok(false);
  155         };
  156         let object = serde_json::from_value(saved.clone()).map_err(json_error)?;
  157         let output: serde_json::Value =
  158             serde_json::from_slice(&self.store.read(&object).await?).map_err(json_error)?;
```


---



## bike-rs/bike-core/src/strava_workflow/inbox&#46;rs

```
  24             return Err(WorkflowError::bad_request(
  25                 "Strava subscription ID does not match",
  26             ));
  27         }
! 28     }
  29     let Some(connection) = strava_connections::Model::for_athlete(db, event.owner_id).await? else {
  30         return Ok(false);
  31     };
  32     let Some(action) = webhook_action(event) else {
! 33         return Ok(false);
  34     };
  35     let store = ObjectStore::new(&crate::config::Config::get().uploads_dir);
  36     let raw = store
  37         .write(
```


---


```
  84     if strava_workflows::Model::pending_revocation(db, connection)
  85         .await?
  86         .is_some()
  87     {
! 88         return Err(WorkflowError::conflict("Strava disconnect is pending"));
  89     }
  90     let txn = db.begin().await?;
  91     if db.get_database_backend() == sea_orm::DatabaseBackend::Postgres {
  92         // Serialize acceptance, separately from the provider execution lease.
```


---


```
  141     db: &sea_orm::DatabaseConnection,
  142     user_id: i32,
  143 ) -> Result<Option<String>, WorkflowError> {
  144     let Some(connection) = crate::strava::load_connection(db, user_id).await? else {
! 145         return Ok(None);
  146     };
  147     let id = format!(
  148         "disconnect-{}-{}",
  149         connection.athlete_id,
```


---


```
  169 pub async fn queue_reconciliation(
  170     db: &sea_orm::DatabaseConnection,
  171 ) -> Result<usize, WorkflowError> {
  172     if !super::provider_work_enabled() {
! 173         return Ok(0);
  174     }
  175     let mut queued = 0;
  176     for connection in strava_connections::Model::all_connected(db).await? {
  177         if strava_workflows::Model::pending_revocation(db, &connection)
```


---


```
  179             .is_none()
  180         {
  181             queue_sync(db, &connection, false).await?;
  182             queued += 1;
! 183         }
  184     }
  185     Ok(queued)
  186 }
```


---


```
  223             message: format!("Accepted Strava {action} workflow."),
  224             payload: Some(serde_json::json!({"workflow_id":id,"correlation_id":activity_id.map(|id| id.to_string()).unwrap_or_else(|| id.into()),"strava_activity_id":activity_id,"athlete_id":connection.athlete_id,"action":action,"source":source,"stages":super::graph(action)?})),
  225         }).await?;
  226         tracing::info!(
! 227             correlation_id = %activity_id.map(|id| id.to_string()).unwrap_or_else(|| id.into()),
  228             workflow_id = id,
  229             strava_activity_id = activity_id,
  230             athlete_id = connection.athlete_id,
  231             action,
```


---


```
  306     db: &sea_orm::DatabaseConnection,
  307     user_id: i32,
  308 ) -> Result<Option<strava_workflows::Model>, WorkflowError> {
  309     let Some(connection) = crate::strava::load_connection(db, user_id).await? else {
! 310         return Ok(None);
  311     };
  312     Ok(strava_workflows::Model::active_collection(db, connection.id).await?)
  313 }
```


---



## bike-rs/bike-core/src/strava_workflow/intents&#46;rs

```
  44     }
  45     txn.commit().await.map_err(storage_error)?;
  46     Ok(true)
  47 }
! 48 fn storage_error(error: sea_orm::DbErr) -> TaskError {
! 49     TaskError::Storage(error.to_string())
! 50 }
```


---



## bike-rs/bike-core/src/strava_workflow/mod&#46;rs

```
  59         }
  60         "delete" => vec!["capture_delete", "delete", "derived"],
  61         "sync" | "backfill" => vec!["discover", "children", "complete"],
  62         "deauthorize" => vec!["capture_cleanup", "revoke", "cleanup"],
! 63         _ => return Err(WorkflowError::bad_request("Unknown Strava workflow action")),
  64     };
  65     let mut dag = petgraph::graph::DiGraph::<&str, ()>::new();
  66     let nodes: Vec<_> = stages.iter().map(|stage| dag.add_node(stage)).collect();
  67     for edge in nodes.windows(2) {
```


---


```
  80     if row.status == "completed" {
  81         return Ok(());
  82     }
  83     if !provider_work_enabled() {
! 84         return Err(WorkflowError::too_many_requests(
! 85             "Strava provider work is paused for cutover",
! 86             Some(chrono::Utc::now() + chrono::Duration::minutes(1)),
! 87         ));
  88     }
  89     // One connection owns refresh and provider work. PostgreSQL releases this
  90     // transaction-scoped lease automatically if the worker dies.
  91     let lease = db.begin().await?;
```


---


```
   95                 "SELECT pg_advisory_xact_lock(7391, {})",
   96                 row.connection_id
   97             ))
   98             .await?;
!  99     }
  100     // A concurrent delivery may have completed while this worker waited.
  101     let mut row = strava_workflows::Entity::find_by_id(id)
  102         .one(db)
  103         .await?
```


---


```
  120     let source: Source = serde_json::from_value(row.source.clone()).map_err(stage::json_error)?;
  121     let Some(connection) =
  122         connection.filter(|connection| connection.created_at == source.generation)
  123     else {
! 124         row.finish(db, None).await?;
! 125         return Ok(());
  126     };
  127     let client = StravaApiClient::new(db, Config::get())?;
  128     if row.action != "deauthorize"
  129         && strava_workflows::Model::pending_revocation(db, &connection)
```


---



## bike-rs/bike-core/src/strava_workflow/revocation&#46;rs

```
  25                         .ok_or_else(|| WorkflowError::internal("Missing revocation capture"))?;
  26                     self.stage(stage, &self.row.id, self.cleanup_revocation(ids))
  27                         .await?;
  28                 }
! 29                 _ => return Err(WorkflowError::internal("Unsupported revocation DAG stage")),
  30             }
  31         }
  32         Ok(())
  33     }
```


---


```
  40                 activity
  41                     .source_correlation_id
  42                     .and_then(|id| id.parse().ok())
  43                     .ok_or_else(|| {
! 44                         WorkflowError::internal("Strava activity has no vendor correlation ID")
! 45                     })
  46             })
  47             .collect()
  48     }
```


---



## bike-rs/bike-core/src/strava_workflow/stage&#46;rs

```
  160         integration_events::Entity::record(&txn, self.stage_event(stage, input, object, output)?)
  161             .await?;
  162         txn.commit().await?;
  163         if let Some(execution) = crate::background_jobs::execution::ExecutionContext::current() {
! 164             execution.record_progress(self.db).await;
  165         }
  166         Ok(())
  167     }
```


---


```
  191             ),
  192         })
  193     }
  194 }
! 195 pub(super) fn json_error(error: serde_json::Error) -> WorkflowError {
! 196     WorkflowError::internal(error.to_string())
! 197 }
```


---


