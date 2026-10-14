# Diff Coverage
## Diff: 7d811e85cc1f1992a99002b0d609e7cbd9d9bdef...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-core/src/activity_import_pipeline&#46;rs (100%)
- bike-rs/bike-core/src/activity_import_pipeline/replay&#46;rs (100%)
- bike-rs/bike-core/src/activity_import_recovery&#46;rs (100%)
- bike-rs/bike-core/src/activity_lifecycle&#46;rs (43.2%): Missing lines 186-187,193-199,236,243,247,252,781,784-790
- bike-rs/bike-core/src/activity_source_recovery&#46;rs (100%)
- bike-rs/bike-core/src/activity_type&#46;rs (100%)
- bike-rs/bike-core/src/auth/cookies&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/diagnostics/health&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/entities/task_recovery&#46;rs (100%)
- bike-rs/bike-core/src/bin/recover-generated-imports&#46;rs (0.0%): Missing lines 141
- bike-rs/bike-core/src/config&#46;rs (100%)
- bike-rs/bike-core/src/entities/activities&#46;rs (36.8%): Missing lines 42-65
- bike-rs/bike-core/src/entities/activities/parsed&#46;rs (100%)
- bike-rs/bike-core/src/entities/activity_import_artifacts&#46;rs (100%)
- bike-rs/bike-core/src/entities/activity_imports&#46;rs (98.4%): Missing lines 192
- bike-rs/bike-core/src/entities/integration_events&#46;rs (100%)
- bike-rs/bike-core/src/entities/strava_activity_states&#46;rs (100%)
- bike-rs/bike-core/src/entities/strava_connections&#46;rs (75.6%): Missing lines 7-17,101,103-110
- bike-rs/bike-core/src/entities/strava_events&#46;rs (75.1%): Missing lines 110,141-154,156,162-170,172-178,180-190,210-212
- bike-rs/bike-core/src/heatmaps/projection&#46;rs (0.0%): Missing lines 129-131,138,145
- bike-rs/bike-core/src/jobs/adapter&#46;rs (100%)
- bike-rs/bike-core/src/jobs/job_type&#46;rs (100%)
- bike-rs/bike-core/src/strava/archive_json&#46;rs (100%)
- bike-rs/bike-core/src/strava/client&#46;rs (50.0%): Missing lines 237
- bike-rs/bike-core/src/strava/connection&#46;rs (4.5%): Missing lines 99-107,109-110,180,196,272-276,334,431,435
- bike-rs/bike-core/src/strava/credentials&#46;rs (80.0%): Missing lines 14
- bike-rs/bike-core/src/strava/events&#46;rs (100%)
- bike-rs/bike-core/src/strava/events/intake&#46;rs (45.5%): Missing lines 28,30,112-126,131-136,138-155,157-164,166-181,183-188,192-193
- bike-rs/bike-core/src/strava/events/intake/strava_webhook_unit&#46;rs (100%)
- bike-rs/bike-core/src/strava/logging&#46;rs (76.8%): Missing lines 37-38,52-56,58,67,92,100-101,127
- bike-rs/bike-core/src/strava/native/dependencies&#46;rs (32.8%): Missing lines 33,35,37,39,41,47,64-71,73-74,76,78-83,85-91,96,104,106,114,116,124-125,127,132
- bike-rs/bike-core/src/strava/native/mod&#46;rs (92.4%): Missing lines 151,220-227,250-251,253-258
- bike-rs/bike-core/src/strava/native/persistence&#46;rs (60.0%): Missing lines 26-28,52-55,71-73,78,122-141,152-155,192,211,213,258-264,276,280-292,294-311,313-322,324-330,332-339,341-352,354-361,363-374
- bike-rs/bike-core/src/strava/native/persistence/strava_native_unit_deletion&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/scan&#46;rs (0.0%): Missing lines 4-21,23-26,28-32
- bike-rs/bike-core/src/strava/native/strava_native_unit/collections&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/strava_native_unit/fixture&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/strava_native_unit/handlers&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/strava_native_unit/handlers_fixture&#46;rs (97.2%): Missing lines 28,85-86
- bike-rs/bike-core/src/strava/native/strava_native_unit/imports&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/strava_native_unit/mod&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/strava_native_unit/persistence&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/strava_native_unit/upsert&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/types&#46;rs (100%)
- bike-rs/bike-core/src/strava/oauth_state&#46;rs (93.1%): Missing lines 36-39,57,59,70,74,167
- bike-rs/bike-core/src/strava/oauth_state/strava_oauth_unit&#46;rs (100%)
- bike-rs/bike-core/src/strava/paths&#46;rs (100%)
- bike-rs/bike-core/src/strava/transport&#46;rs (0.0%): Missing lines 86
- bike-rs/bike-core/src/workflow_error&#46;rs (100%)

## Summary

- **Total**: 3625 lines
- **Missing**: 457 lines
- **Coverage**: 87.39%



## bike-rs/bike-core/src/activity_lifecycle&#46;rs

```
  182 
  183     Ok(affected_segment_ids)
  184 }
  185 
! 186 pub async fn delete_activity_with_derived_state<C: ConnectionTrait + TransactionTrait>(
! 187     db: &C,
  188     uploads_dir: &str,
  189     user_id: i32,
  190     activity: activities::Model,
  191 ) -> Result<Vec<i32>, AppError> {
```


---


```
  189     user_id: i32,
  190     activity: activities::Model,
  191 ) -> Result<Vec<i32>, AppError> {
  192     let txn = db.begin().await?;
! 193     let deleted = delete_activity_records(&txn, user_id, activity).await?;
! 194     txn.commit().await?;
! 195     if let Some(source) = deleted.source {
! 196         remove_upload_file(Path::new(uploads_dir).join(source)).await;
! 197     }
! 198     Ok(deleted.segment_ids)
! 199 }
  200 
  201 pub struct DeletedActivity {
  202     pub segment_ids: Vec<i32>,
  203     pub source: Option<String>,
```


---


```
  232     if let Some(activity_import_id) = activity.activity_import_id {
  233         let remaining_references = activities::Entity::find()
  234             .filter(activities::Column::UserId.eq(user_id))
  235             .filter(activities::Column::ActivityImportId.eq(Some(activity_import_id)))
! 236             .count(db)
  237             .await?;
  238 
  239         if remaining_references == 0 {
  240             let linked_import = activity_imports::Entity::find()
```


---


```
  239         if remaining_references == 0 {
  240             let linked_import = activity_imports::Entity::find()
  241                 .filter(activity_imports::Column::Id.eq(activity_import_id))
  242                 .filter(activity_imports::Column::UserId.eq(user_id))
! 243                 .one(db)
  244                 .await?;
  245 
  246             if let Some(linked_import) = linked_import {
! 247                 source = Some(linked_import.storage_path);
  248 
  249                 activity_imports::Entity::delete_many()
  250                     .filter(activity_imports::Column::Id.eq(activity_import_id))
  251                     .filter(activity_imports::Column::UserId.eq(user_id))
! 252                     .exec(db)
  253                     .await?;
  254             }
  255         }
  256     }
```


---


```
  777 async fn load_activity_for_reprocessing(
  778     db: &DatabaseConnection,
  779     activity_id: i32,
  780 ) -> Result<activities::Model, AppError> {
! 781     let activity = activities::Entity::find_by_id(activity_id)
  782         .one(db)
  783         .await?
! 784         .ok_or_else(|| AppError::not_found(format!("Activity {activity_id} was not found")))?;
! 785     if activity.source == "strava_sync" {
! 786         return Err(AppError::bad_request(
! 787             "Use Strava sync or backfill for Strava imports",
! 788         ));
! 789     }
! 790     Ok(activity)
  791 }
  792 
  793 fn activity_import_id_for_reprocessing(activity: &activities::Model) -> Result<i32, AppError> {
  794     activity.activity_import_id.ok_or_else(|| {
```


---



## bike-rs/bike-core/src/bin/recover-generated-imports&#46;rs

```
  137             }
  138         }
  139     }
  140     if run.refresh_recent && recent_gaps {
! 141         strava::events::request_sync(&db, run.user_id).await?;
  142     }
  143     println!(
  144         "{}",
  145         serde_json::json!({"next_after_id": last_id, "applied": run.apply, "failed": failed, "recent_refresh_requested": run.refresh_recent && recent_gaps})
```


---



## bike-rs/bike-core/src/entities/activities&#46;rs

```
  38     pub activity_import_id: Option<i32>,
  39 }
  40 
  41 impl Model {
! 42     pub async fn native_strava_ids(
! 43         db: &impl ConnectionTrait,
! 44         user_id: i32,
! 45     ) -> Result<Vec<i64>, DbErr> {
! 46         let values = Entity::find()
! 47             .select_only()
! 48             .column(Column::SourceCorrelationId)
! 49             .filter(Column::UserId.eq(user_id))
! 50             .filter(Column::Source.eq("strava_sync"))
! 51             .order_by_asc(Column::Id)
! 52             .limit(50)
! 53             .into_tuple::<Option<String>>()
! 54             .all(db)
! 55             .await?;
! 56         values
! 57             .into_iter()
! 58             .flatten()
! 59             .map(|value| {
! 60                 value
! 61                     .parse::<i64>()
! 62                     .map_err(|error| DbErr::Custom(error.to_string()))
! 63             })
! 64             .collect()
! 65     }
  66     pub async fn admin_page(
  67         db: &impl ConnectionTrait,
  68         filters: &AdminListFilters,
  69         page: u64,
```


---



## bike-rs/bike-core/src/entities/activity_imports&#46;rs

```
  188     ) -> Result<Vec<Model>, DbErr> {
  189         Self::find()
  190             .filter(Column::UserId.eq(user_id))
  191             .filter(Column::Id.gt(after_id))
! 192             .filter(Column::Source.eq(STRAVA_SOURCE))
  193             .filter(Column::Format.eq("tcx"))
  194             .order_by_asc(Column::Id)
  195             .limit(limit.clamp(1, 32))
  196             .all(db)
```


---



## bike-rs/bike-core/src/entities/strava_connections&#46;rs

```
   3 use sea_orm::entity::prelude::*;
   4 use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, QueryFilter, Set};
   5 
   6 impl Model {
!  7     pub async fn begin_disconnect(
!  8         db: &impl ConnectionTrait,
!  9         event: &crate::strava::events::StravaEvent,
! 10     ) -> Result<(), DbErr> {
! 11         let connection = Self::lock_event(db, event).await?;
! 12         let requested = connection.disconnect_requested_at.unwrap_or_else(Utc::now);
! 13         let mut active: ActiveModel = connection.into();
! 14         active.disconnect_requested_at = Set(Some(requested));
! 15         active.update(db).await?;
! 16         Ok(())
! 17     }
  18     pub async fn complete_native_sync(
  19         db: &impl ConnectionTrait,
  20         result: &crate::strava::native::CollectionResult,
  21     ) -> Result<(), DbErr> {
```


---


```
   97     }
   98     pub async fn all_connected(db: &impl ConnectionTrait) -> Result<Vec<Self>, DbErr> {
   99         Entity::find().all(db).await
  100     }
! 101     pub async fn scan_page(db: &impl ConnectionTrait, after_id: i32) -> Result<Vec<Self>, DbErr> {
  102         use sea_orm::{QueryOrder, QuerySelect};
! 103         Entity::find()
! 104             .filter(Column::Id.gt(after_id))
! 105             .filter(Column::DisconnectRequestedAt.is_null())
! 106             .order_by_asc(Column::Id)
! 107             .limit(20)
! 108             .all(db)
! 109             .await
! 110     }
  111 }
  112 
  113 #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
  114 #[sea_orm(table_name = "strava_connections")]
```


---



## bike-rs/bike-core/src/entities/strava_events&#46;rs

```
  106             event
  107                 .webhook_key()
  108                 .map_err(|error| DbErr::Custom(error.to_string()))?
  109         } else {
! 110             id.clone()
  111         };
  112         let inserted = Entity::insert(ActiveModel {
  113             id: Set(id),
  114             event_key: Set(key.clone()),
```


---


```
  137             .one(&txn)
  138             .await?
  139             .ok_or(DbErr::RecordNotFound("Strava event was not stored".into()))?;
  140         if inserted > 0 {
! 141             super::integration_events::Entity::record(&txn, super::integration_events::NewEvent {
! 142                 user_id: Some(event.bike_user_id),
! 143                 provider: "strava".into(),
! 144                 event_type: "strava.event.accepted".into(),
! 145                 level: "info".into(),
! 146                 message: format!("Strava {:?} requested.", event.operation),
! 147                 connection_id: Some(event.connection_id),
! 148                 payload: Some(serde_json::json!({
! 149                     "operation":event.operation, "strava_event_id":row.id,
! 150                     "correlation_id":row.correlation_id, "parent_event_id":row.parent_event_id,
! 151                     "bike_customer_id":row.customer_id,"bike_user_id":row.user_id,
! 152                     "strava_athlete_id":row.athlete_id,"strava_activity_id":row.activity_id,
! 153                     "temporal_namespace":std::env::var("TEMPORAL_NAMESPACE").unwrap_or_else(|_| "bike".into()),
! 154                     "temporal_workflow_id":row.workflow_id,
  155                 })),
! 156             }).await?;
  157         }
  158         txn.commit().await?;
  159         Ok(row)
  160     }
```


---


```
  158         txn.commit().await?;
  159         Ok(row)
  160     }
  161 
! 162     pub async fn pending(db: &impl ConnectionTrait) -> Result<Vec<Self>, DbErr> {
! 163         Entity::find()
! 164             .filter(Column::SubmittedAt.is_null())
! 165             .filter(Column::NextAttemptAt.lte(chrono::Utc::now()))
! 166             .order_by_asc(Column::NextAttemptAt)
! 167             .limit(100)
! 168             .all(db)
! 169             .await
! 170     }
  171 
! 172     pub async fn submitted(self, db: &impl ConnectionTrait) -> Result<(), DbErr> {
! 173         let mut active: ActiveModel = self.into();
! 174         active.submitted_at = Set(Some(chrono::Utc::now()));
! 175         active.handoff_error = Set(None);
! 176         active.update(db).await?;
! 177         Ok(())
! 178     }
  179 
! 180     pub async fn retry_handoff(
! 181         self,
! 182         db: &impl ConnectionTrait,
! 183         error: String,
! 184     ) -> Result<(), DbErr> {
! 185         let mut active: ActiveModel = self.into();
! 186         active.next_attempt_at = Set(chrono::Utc::now() + chrono::Duration::seconds(30));
! 187         active.handoff_error = Set(Some(error));
! 188         active.update(db).await?;
! 189         Ok(())
! 190     }
  191 
  192     pub async fn stored_mutation(
  193         db: &impl ConnectionTrait,
  194         event: &crate::strava::events::StravaEvent,
```


---


```
  206         }
  207         let stored: crate::strava::events::StravaEvent = serde_json::from_value(row.event.clone())
  208             .map_err(|error| DbErr::Custom(error.to_string()))?;
  209         if !stored.has_same_request(event) {
! 210             return Err(DbErr::Custom(
! 211                 "Strava event contract does not match stored acceptance".into(),
! 212             ));
  213         }
  214         row.mutation
  215             .map(|value| {
  216                 serde_json::from_value::<ActivityMutation>(value)
```


---



## bike-rs/bike-core/src/heatmaps/projection&#46;rs

```
  125         // then uses the shared queue API in this same transaction, including lineage.
  126         let leased = PendingProjection::find_by_statement(Statement::from_sql_and_values(DbBackend::Postgres, r#"
  127             WITH candidates AS (
  128                 SELECT p.activity_id FROM heatmap_projections p
! 129                 WHERE p.status='pending' AND p.projection_version=$1 AND (p.queued_at IS NULL OR p.queued_at < now() - make_interval(secs => $2::double precision))
! 130                 AND EXISTS (SELECT 1 FROM activities a WHERE a.id=p.activity_id AND a.source <> 'strava_sync')
! 131                 AND NOT EXISTS (SELECT 1 FROM background_tasks t WHERE t.task_type=$3
  132                     AND t.status IN ('pending','processing') AND (t.status='processing' OR t.attempts<t.max_attempts)
  133                     AND (t.payload->'data'->'activities') @> jsonb_build_array(jsonb_build_object('activity_id',p.activity_id,'generation',p.generation)))
  134                 ORDER BY p.activity_id LIMIT 16 FOR UPDATE SKIP LOCKED
  135             )
```


---


```
  134                 ORDER BY p.activity_id LIMIT 16 FOR UPDATE SKIP LOCKED
  135             )
  136             UPDATE heatmap_projections p SET queued_at=now() FROM candidates c WHERE p.activity_id=c.activity_id
  137             RETURNING p.activity_id, p.generation
! 138         "#,[PROJECTION_VERSION.into(), crate::config::Config::get().worker_recovery_timeout_seconds.into(), crate::jobs::JobType::PrepareHeatmap.as_str().into()])).all(&txn).await?;
  139         if leased.is_empty() {
  140             return Ok(0);
  141         }
  142         let activities: Vec<_> = leased.iter().map(|pending|serde_json::json!({"activity_id":pending.activity_id,"generation":pending.generation})).collect();
```


---


```
  141         }
  142         let activities: Vec<_> = leased.iter().map(|pending|serde_json::json!({"activity_id":pending.activity_id,"generation":pending.generation})).collect();
  143         let task = crate::background_jobs::background_tasks::Model::enqueue(
  144             &txn,
! 145             crate::jobs::JobType::PrepareHeatmap.as_str().into(),
  146             serde_json::json!({"type":"PrepareHeatmap","data":{"activities":activities}}),
  147             None,
  148             3,
  149         )
```


---



## bike-rs/bike-core/src/strava/client&#46;rs

```
  233         R: DeserializeOwned,
  234     {
  235         if let Some(archive) = RequestArchive::current() {
  236             if let Some(saved) = archive.store.response(&archive.key).await? {
! 237                 tracing::info!(operation, object_path = %saved.body.path, response_status = super::logging::ProviderResponseStatus::Reused.as_str(), "Reusing saved Strava response");
  238                 let bytes = archive.store.response_bytes(&saved).await?;
  239                 // Replays honor the generated response model as live calls do.
  240                 let _: R = decode_response(&bytes, action)?;
  241                 return decode_response(&bytes, action);
```


---



## bike-rs/bike-core/src/strava/connection&#46;rs

```
   95     pub url: Url,
   96     pub browser_session: String,
   97 }
   98 
!  99 pub async fn create_authorization_url_for_user(
! 100     db: &DatabaseConnection,
! 101     config: &Config,
! 102     user_id: i32,
! 103 ) -> Result<StravaAuthorization, AppError> {
! 104     ensure_strava_configured(config)?;
! 105     let state = super::oauth_state::OAuthState::new(&config.jwt_secret)
! 106         .issue(db, user_id, Utc::now())
! 107         .await?;
  108     Ok(StravaAuthorization {
! 109         url: build_authorization_url(config, &state.token).map_err(AppError::internal)?,
! 110         browser_session: state.browser_session,
  111     })
  112 }
  113 
  114 pub fn scopes_allow_activity_import(scopes: &str) -> bool {
```


---


```
  176 
  177 pub async fn exchange_code_for_connection(
  178     db: &DatabaseConnection,
  179     code: &str,
! 180     verified_state: super::oauth_state::VerifiedStravaState,
  181 ) -> Result<strava_connections::Model, AppError> {
  182     let config = Config::get();
  183     ensure_strava_configured(config)?;
```


---


```
  192 
  193         let connection =
  194             upsert_connection_from_token(db, verified_state.user_id, token_response).await?;
  195 
! 196         crate::strava::events::request_sync(db, connection.user_id).await?;
  197         load_connection(db, connection.user_id)
  198             .await?
  199             .ok_or_else(|| AppError::not_found("Strava connection was removed"))
  200     }
```


---


```
  268 ) -> Result<(), AppError> {
  269     let Some(connection) = load_connection(db, user_id).await? else {
  270         return Ok(());
  271     };
! 272     strava_connections::Entity::delete_many()
! 273         .filter(strava_connections::Column::Id.eq(connection.id))
! 274         .exec(db)
! 275         .await?;
! 276     Ok(())
  277 }
  278 
  279 fn ensure_strava_configured(config: &Config) -> Result<(), AppError> {
  280     if config.strava_enabled() {
```


---


```
  330         active_model.refresh_token = Set(crate::strava::credentials::seal(
  331             token_response.refresh_token.as_bytes(),
  332         )?);
  333         active_model.created_at = Set(Utc::now());
! 334         active_model.disconnect_requested_at = Set(None);
  335         active_model.expires_at = Set(expires_at);
  336         active_model.update(db).await.map_err(AppError::from)
  337     } else {
  338         strava_connections::ActiveModel {
```


---


```
  427     })
  428 }
  429 
  430 pub async fn resolve_connection_sync_state(
! 431     _db: &DatabaseConnection,
  432     connection: &strava_connections::Model,
  433 ) -> Result<ResolvedStravaConnectionSyncState, AppError> {
  434     Ok(ResolvedStravaConnectionSyncState {
! 435         connection: connection.clone(),
  436         active_sync_status: None,
  437     })
  438 }
```


---



## bike-rs/bike-core/src/strava/credentials&#46;rs

```
  10     let key = match std::env::var("STRAVA_TOKEN_ENCRYPTION_KEY")
  11         .ok()
  12         .filter(|key| !key.is_empty())
  13     {
! 14         Some(key) => STANDARD
  15             .decode(key)
  16             .map_err(|_| WorkflowError::internal("Invalid Strava credential encryption key"))?,
  17         None if Config::get().environment != "production" => {
  18             use sha2::Digest;
```


---



## bike-rs/bike-core/src/strava/events/intake&#46;rs

```
  24     async fn customer(&self, user_id: i32) -> Result<users::Model, WorkflowError> {
  25         Ok(users::Model::find_by_id(self, user_id).await?)
  26     }
  27 
! 28     async fn accept(&self, event: StravaEvent) -> Result<strava_events::Model, WorkflowError> {
  29         accept(self, event).await
! 30     }
  31 }
  32 
  33 struct WebhookIntake {
  34     subscription_id: Option<i64>,
```


---


```
  108     operation == StravaOperation::Disconnect
  109         && webhook.event_time < connection.created_at.timestamp()
  110 }
  111 
! 112 pub async fn accept_webhook(
! 113     db: &DatabaseConnection,
! 114     webhook: &StravaWebhookEvent,
! 115     request_id: Option<String>,
! 116 ) -> Result<Option<strava_events::Model>, WorkflowError> {
! 117     let subscription_id = std::env::var("STRAVA_SUBSCRIPTION_ID")
! 118         .ok()
! 119         .filter(|value| !value.is_empty())
! 120         .map(|value| value.parse::<i64>())
! 121         .transpose()
! 122         .map_err(|_| WorkflowError::internal("Invalid Strava subscription ID"))?;
! 123     WebhookIntake { subscription_id }
! 124         .accept(db, webhook, request_id)
! 125         .await
! 126 }
  127 
  128 #[cfg(test)]
  129 mod strava_webhook_unit;
  130 
! 131 pub async fn request_sync(
! 132     db: &DatabaseConnection,
! 133     user_id: i32,
! 134 ) -> Result<strava_events::Model, WorkflowError> {
! 135     accept(db, user_event(db, user_id, StravaOperation::Sync).await?).await
! 136 }
  137 
! 138 pub async fn request_backfill(
! 139     db: &DatabaseConnection,
! 140     user_id: i32,
! 141     after: DateTime<Utc>,
! 142     before: DateTime<Utc>,
! 143 ) -> Result<strava_events::Model, WorkflowError> {
! 144     if before <= after {
! 145         return Err(WorkflowError::bad_request(
! 146             "Backfill must end after it starts",
! 147         ));
! 148     }
! 149     let mut event = user_event(db, user_id, StravaOperation::Backfill).await?;
! 150     event.window = Some(super::super::native::ActivityWindow {
! 151         after: after.timestamp() - 1,
! 152         before: Some(before.timestamp()),
! 153     });
! 154     accept(db, event).await
! 155 }
  156 
! 157 pub async fn request_disconnect(
! 158     db: &DatabaseConnection,
! 159     user_id: i32,
! 160 ) -> Result<strava_events::Model, WorkflowError> {
! 161     let mut event = user_event(db, user_id, StravaOperation::Disconnect).await?;
! 162     event.revoke_remote = true;
! 163     accept(db, event).await
! 164 }
  165 
! 166 async fn user_event(
! 167     db: &DatabaseConnection,
! 168     user_id: i32,
! 169     operation: StravaOperation,
! 170 ) -> Result<StravaEvent, WorkflowError> {
! 171     let connection = super::super::load_connection(db, user_id)
! 172         .await?
! 173         .ok_or_else(|| WorkflowError::not_found("Connect Strava first"))?;
! 174     if matches!(operation, StravaOperation::Sync | StravaOperation::Backfill) {
! 175         super::super::ensure_connection_scopes_allow_activity_import(&connection)?;
! 176     }
! 177     let user = users::Model::find_by_id(db, user_id).await?;
! 178     let event = StravaEvent::new(&user, &connection, operation);
! 179     event.validate_connection(&connection)?;
! 180     Ok(event)
! 181 }
  182 
! 183 async fn accept(
! 184     db: &DatabaseConnection,
! 185     event: StravaEvent,
! 186 ) -> Result<strava_events::Model, WorkflowError> {
! 187     let row = strava_events::Model::accept(db, &event).await?;
! 188     tracing::info!(correlation_id = row.correlation_id, strava_event_id = row.id,
  189         bike_customer_id = %row.customer_id, bike_user_id = row.user_id,
  190         strava_athlete_id = row.athlete_id, strava_activity_id = row.activity_id,
  191         temporal_workflow_id = row.workflow_id, "Accepted durable Strava event");
! 192     Ok(row)
! 193 }
```


---



## bike-rs/bike-core/src/strava/logging&#46;rs

```
  33     pub const fn as_str(self) -> &'static str {
  34         match self {
  35             Self::Started => "started",
  36             Self::Completed => "completed",
! 37             Self::Failed => "failed",
! 38             Self::Cancelled => "cancelled",
  39         }
  40     }
  41 }
```


---


```
  48     TransportError,
  49 }
  50 
  51 impl ProviderResponseStatus {
! 52     pub const fn as_str(self) -> &'static str {
! 53         match self {
! 54             Self::Reused => "reused",
! 55             Self::Fetched => "fetched",
! 56             Self::TransportError => "transport_error",
  57         }
! 58     }
  59 }
  60 
  61 impl ExchangeContext {
  62     pub fn attempt(&self, processor: &str, status: AttemptStatus) {
```


---


```
  63         let event = &self.event;
  64         let temporal = &self.temporal;
  65         tracing::info!(processor, status = status.as_str(), correlation_id = %event.correlation_id,
  66             parent_event_id = event.parent_event_id.map(|id| id.to_string()).as_deref(),
! 67             request_id = event.request_id.as_deref(), trace_id = event.trace_id().as_deref(),
  68             strava_event_id = %event.strava_event_id, bike_customer_id = %event.bike_customer_id,
  69             bike_user_id = event.bike_user_id, bike_activity_id = event.bike_activity_id,
  70             strava_athlete_id = event.strava_athlete_id, strava_activity_id = event.strava_activity_id,
  71             temporal_namespace = temporal.temporal_namespace, temporal_workflow_id = temporal.temporal_workflow_id,
```


---


```
  88             bike_customer_id = %event.bike_customer_id,
  89             bike_user_id = event.bike_user_id,
  90             bike_activity_id = event.bike_activity_id,
  91             strava_athlete_id = %event.strava_athlete_id,
! 92             strava_activity_id = activity_id.as_deref(),
  93             connection_id = event.connection_id,
  94             strava_event_time = event.event_time,
  95             temporal_namespace = temporal.temporal_namespace,
  96             temporal_workflow_id = temporal.temporal_workflow_id,
```


---


```
   96             temporal_workflow_id = temporal.temporal_workflow_id,
   97             temporal_run_id = temporal.temporal_run_id,
   98             temporal_activity_id = temporal.temporal_activity_id,
   99             temporal_attempt = temporal.temporal_attempt,
! 100             request_id = event.request_id.as_deref(),
! 101             trace_id = event.trace_id().as_deref(), span_id = tracing::field::Empty);
  102         if let Some(carrier) = &event.trace_context {
  103             crate::observability::set_span_parent_from_carrier(&span, Some(carrier));
  104         }
  105         crate::observability::record_span_trace_context(&span);
```


---


```
  123         let temporal = &self.temporal;
  124         tracing::info!(
  125             correlation_id = %event.correlation_id,
  126             parent_event_id = event.parent_event_id.map(|id| id.to_string()).as_deref(),
! 127             request_id = event.request_id.as_deref(), trace_id = event.trace_id().as_deref(),
  128             strava_event_id = %event.strava_event_id,
  129             bike_customer_id = %event.bike_customer_id,
  130             bike_user_id = event.bike_user_id,
  131             bike_activity_id = event.bike_activity_id,
```


---



## bike-rs/bike-core/src/strava/native/dependencies&#46;rs

```
  29 }
  30 
  31 #[async_trait::async_trait]
  32 impl ActivityStorage for ObjectStore {
! 33     async fn read(&self, object: &ObjectRef) -> Result<Vec<u8>, WorkflowError> {
  34         ObjectStore::read(self, object).await
! 35     }
  36 
! 37     async fn write(&self, key: &str, bytes: &[u8]) -> Result<ObjectRef, WorkflowError> {
  38         ObjectStore::write(self, key, bytes).await
! 39     }
  40 
! 41     async fn remove(&self, path: &str) -> Result<(), WorkflowError> {
  42         match tokio::fs::remove_file(self.path(path)?).await {
  43             Ok(()) => Ok(()),
  44             Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
  45             Err(error) => Err(WorkflowError::internal(error.to_string())),
```


---


```
  43             Ok(()) => Ok(()),
  44             Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
  45             Err(error) => Err(WorkflowError::internal(error.to_string())),
  46         }
! 47     }
  48 }
  49 
  50 #[async_trait::async_trait]
  51 pub trait StravaProvider: Send + Sync {
```


---


```
   60     pub store: ObjectStore,
   61 }
   62 
   63 impl ProviderClient {
!  64     async fn access(
!  65         &self,
!  66         event: &StravaEvent,
!  67     ) -> Result<(StravaApiClient, String), WorkflowError> {
!  68         let connection = strava_connections::Model::for_event(&self.db, event).await?;
!  69         let client = StravaApiClient::new(&self.db, Config::get())?;
!  70         let connection =
!  71             super::super::ensure_fresh_access_token(&self.db, &client, connection).await?;
   72         Ok((
!  73             client,
!  74             super::super::credentials::open(&connection.access_token)?,
   75         ))
!  76     }
   77 
!  78     fn archive(&self, event: &StravaEvent, resource: &str) -> RequestArchive {
!  79         RequestArchive {
!  80             store: self.store.clone(),
!  81             key: object_key(event, resource),
!  82         }
!  83     }
   84 
!  85     async fn response(&self, archive: &RequestArchive) -> Result<ObjectRef, WorkflowError> {
!  86         self.store
!  87             .response(&archive.key)
!  88             .await?
!  89             .map(|response| response.body)
!  90             .ok_or_else(|| WorkflowError::internal("Strava response was not retained"))
!  91     }
   92 }
   93 
   94 #[async_trait::async_trait]
   95 impl StravaProvider for ProviderClient {
!  96     async fn activity(&self, event: &StravaEvent) -> Result<ObjectRef, WorkflowError> {
   97         let (client, token) = self.access(event).await?;
   98         let archive = self.archive(event, "activity");
   99         archive
  100             .clone()
```


---


```
  100             .clone()
  101             .scope(client.get_activity(&token, event.activity_id()?))
  102             .await?;
  103         self.response(&archive).await
! 104     }
  105 
! 106     async fn streams(&self, event: &StravaEvent) -> Result<ObjectRef, WorkflowError> {
  107         let (client, token) = self.access(event).await?;
  108         let archive = self.archive(event, "streams");
  109         archive
  110             .clone()
```


---


```
  110             .clone()
  111             .scope(client.get_activity_streams(&token, event.activity_id()?))
  112             .await?;
  113         self.response(&archive).await
! 114     }
  115 
! 116     async fn page(&self, input: &PageInput) -> Result<Vec<StravaActivitySummary>, WorkflowError> {
  117         let (client, token) = self.access(&input.collection.event).await?;
  118         client
  119             .list_activity_page(&token, Some(input.collection.window.after), input.page)
  120             .await?
```


---


```
  120             .await?
  121             .into_iter()
  122             .map(serde_json::from_value)
  123             .collect::<Result<Vec<_>, _>>()
! 124             .map_err(|error| WorkflowError::bad_request(error.to_string()))
! 125     }
  126 
! 127     async fn revoke(&self, event: &StravaEvent) -> Result<(), WorkflowError> {
  128         let (client, token) = self.access(event).await?;
  129         self.archive(event, "deauthorize")
  130             .scope(client.deauthorize(&token))
  131             .await
! 132     }
  133 }
  134 
  135 pub(super) fn object_key(event: &StravaEvent, resource: &str) -> String {
  136     format!("strava/events/{}/{resource}", event.strava_event_id)
```


---



## bike-rs/bike-core/src/strava/native/mod&#46;rs

```
  147                 artifact_kind: "provider_payload",
  148                 source_quality: "strava_streams",
  149                 bytes: &upload.bytes,
  150             },
! 151         )?;
  152         let source = self
  153             .store
  154             .write(&self.key(&input.event, "normalized-source"), &upload.bytes)
  155             .await?;
```


---


```
  216         }
  217         Ok(CollectionInput { event, window })
  218     }
  219 
! 220     pub async fn accept_child(&self, event: StravaEvent) -> Result<StravaEvent, WorkflowError> {
! 221         self.connection(&event).await?;
! 222         let stored = crate::entities::strava_events::Model::accept(&self.db, &event).await?;
! 223         let event = serde_json::from_value(stored.event.clone())
! 224             .map_err(|error| WorkflowError::internal(error.to_string()))?;
! 225         stored.submitted(&self.db).await?;
! 226         Ok(event)
! 227     }
  228 
  229     pub async fn complete_sync(
  230         &self,
  231         mut result: CollectionResult,
```


---


```
  246     ) -> Result<CollectionResult, WorkflowError> {
  247         self.complete_sync(result).await
  248     }
  249 
! 250     pub async fn begin_disconnect(&self, event: StravaEvent) -> Result<Vec<i64>, WorkflowError> {
! 251         self.connection(&event).await?;
  252         use sea_orm::TransactionTrait;
! 253         let txn = self.db.begin().await?;
! 254         strava_connections::Model::begin_disconnect(&txn, &event).await?;
! 255         let ids = activities::Model::native_strava_ids(&txn, event.bike_user_id).await?;
! 256         txn.commit().await?;
! 257         Ok(ids)
! 258     }
  259 
  260     pub async fn revoke_connection(
  261         &self,
  262         event: StravaEvent,
```


---



## bike-rs/bike-core/src/strava/native/persistence&#46;rs

```
  22         let summary: super::super::provider_payload::StoredStravaProviderPayload =
  23             serde_json::from_slice(&bytes)
  24                 .map_err(|error| WorkflowError::bad_request(error.to_string()))?;
  25         if !super::super::provider_payload::strava_activity_is_bike(&summary.activity) {
! 26             return self
! 27                 .delete_activity(self.capture_deletion(input.event).await?)
! 28                 .await;
  29         }
  30         let parsed_ref = input
  31             .parsed
  32             .as_ref()
```


---


```
  48     ) -> Result<ActivityChange, WorkflowError> {
  49         let txn = self.db.begin().await?;
  50         strava_connections::Model::lock_event(&txn, &input.event).await?;
  51         if strava_activity_states::Model::supersedes(&txn, &input.event, false).await? {
! 52             let change = ActivityChange::unchanged(input.event, ActivityStatus::Superseded);
! 53             strava_events::Model::record_mutation(&txn, &change).await?;
! 54             txn.commit().await?;
! 55             return Ok(change);
  56         }
  57         let existing = activities::Model::strava_activity(
  58             &txn,
  59             input.event.bike_user_id,
```


---


```
  67                 &input.source.sha256,
  68             )
  69             .await?
  70             {
! 71                 let change = Self::persist_unchanged(&txn, &input, activity).await?;
! 72                 txn.commit().await?;
! 73                 return Ok(change);
  74             }
  75         }
  76         let upload = super::super::build_activity_upload(&summary.activity, &summary.streams)?;
  77         if let Some(change) = strava_events::Model::stored_mutation(&txn, &input.event).await? {
! 78             return Ok(change);
  79         }
  80         let import = activity_imports::Entity::native_strava_source(
  81             &txn,
  82             &input.event,
```


---


```
  118         txn.commit().await?;
  119         Ok(change)
  120     }
  121 
! 122     async fn persist_unchanged(
! 123         db: &impl sea_orm::ConnectionTrait,
! 124         input: &NormalizedActivity,
! 125         activity: &activities::Model,
! 126     ) -> Result<ActivityChange, WorkflowError> {
! 127         let mut change = ActivityChange::unchanged(input.event.clone(), ActivityStatus::Unchanged);
! 128         change.event.bike_activity_id = Some(activity.id);
! 129         change.bike_activity_id = Some(activity.id);
! 130         change.import_id = activity.activity_import_id;
! 131         change.dirty_from = Some(activity.started_at.date_naive());
! 132         strava_activity_states::Model::apply_event(
! 133             db,
! 134             &input.event,
! 135             Some(input.source.sha256.clone()),
! 136             false,
! 137         )
! 138         .await?;
! 139         strava_events::Model::record_mutation(db, &change).await?;
! 140         Ok(change)
! 141     }
  142 
  143     pub async fn capture_deletion(
  144         &self,
  145         event: StravaEvent,
```


---


```
  148         let Some(activity) =
  149             activities::Model::strava_activity(&self.db, event.bike_user_id, event.activity_id()?)
  150                 .await?
  151         else {
! 152             return Ok(ActivityChange::unchanged(
! 153                 event,
! 154                 ActivityStatus::AlreadyAbsent,
! 155             ));
  156         };
  157         let segment_ids =
  158             crate::activity_lifecycle::load_segment_ids_for_activity(&self.db, activity.id).await?;
  159         let mut event = event;
```


---


```
  188         mut change: ActivityChange,
  189     ) -> Result<ActivityChange, WorkflowError> {
  190         let connection = strava_connections::Model::lock_event(txn, &change.event).await?;
  191         if let Some(saved) = strava_events::Model::stored_mutation(txn, &change.event).await? {
! 192             return Ok(saved);
  193         }
  194         if Self::deletion_superseded(txn, &change.event, &connection).await? {
  195             let skipped = ActivityChange::unchanged(change.event, ActivityStatus::Superseded);
  196             strava_events::Model::record_mutation(txn, &skipped).await?;
```


---


```
  207         connection: &strava_connections::Model,
  208     ) -> Result<bool, WorkflowError> {
  209         let disconnect_cleanup = event.parent_operation
  210             == Some(super::super::events::StravaOperation::Disconnect)
! 211             && connection.disconnect_requested_at.is_some();
  212         if disconnect_cleanup {
! 213             return Ok(false);
  214         }
  215         Ok(strava_activity_states::Model::supersedes(db, event, true).await?)
  216     }
```


---


```
  254         change: &ActivityChange,
  255     ) -> Result<(), WorkflowError> {
  256         strava_activity_states::Model::apply_event(db, &change.event, None, true).await?;
  257         if let Some(day) = change.dirty_from {
! 258             crate::analytics::mark_user_fitness_dirty(
! 259                 db,
! 260                 change.event.bike_user_id,
! 261                 day,
! 262                 chrono::Utc::now(),
! 263             )
! 264             .await?;
  265         }
  266         strava_events::Model::record_mutation(db, change).await?;
  267         Ok(())
  268     }
```


---


```
  272         change: ActivityChange,
  273     ) -> Result<ActivityChange, WorkflowError> {
  274         if let Some(source) = &change.removed_source {
  275             self.store.remove(source).await?;
! 276         }
  277         Ok(change)
  278     }
  279 
! 280     pub async fn rebuild_activity(
! 281         &self,
! 282         mut change: ActivityChange,
! 283     ) -> Result<ActivityChange, WorkflowError> {
! 284         let txn = self.db.begin().await?;
! 285         strava_connections::Model::lock_event(&txn, &change.event).await?;
! 286         if strava_activity_states::Model::supersedes(&txn, &change.event, false).await? {
! 287             return Ok(change);
! 288         }
! 289         if let Some(id) = change.bike_activity_id {
! 290             if let Some(activity) = activities::Entity::find_by_id(id).one(&txn).await? {
! 291                 let derived = crate::activity_data::deserialize_derived_activity_data(
! 292                     activity.derived_data_json.as_ref(),
  293                 );
! 294                 change.segment_ids = crate::activity_lifecycle::refresh_activity_derived_state_without_cache_rebuilds(&txn, change.event.bike_user_id, id, &derived.route_points).await?;
! 295                 crate::analytics::mark_segment_activity_changes(
! 296                     &txn,
! 297                     &change.segment_ids,
! 298                     chrono::Utc::now(),
! 299                 )
! 300                 .await?;
! 301                 crate::analytics::rebuild_activity_analytics_cache(&txn, &[id]).await?;
! 302                 crate::activity_training_analysis::rebuild_activity_training_analysis_cache(
! 303                     &txn,
! 304                     &[id],
! 305                 )
! 306                 .await?;
! 307             }
! 308         }
! 309         txn.commit().await?;
! 310         Ok(change)
! 311     }
  312 
! 313     pub async fn rebuild_fitness(
! 314         &self,
! 315         change: ActivityChange,
! 316     ) -> Result<ActivityChange, WorkflowError> {
! 317         if change.dirty_from.is_some() {
! 318             crate::analytics::rebuild_fitness_freshness_cache(&self.db, change.event.bike_user_id)
! 319                 .await?;
! 320         }
! 321         Ok(change)
! 322     }
  323 
! 324     pub async fn rebuild_segments(
! 325         &self,
! 326         change: ActivityChange,
! 327     ) -> Result<ActivityChange, WorkflowError> {
! 328         crate::analytics::rebuild_segment_analytics_cache(&self.db, &change.segment_ids).await?;
! 329         Ok(change)
! 330     }
  331 
! 332     pub async fn publish_heatmap(
! 333         &self,
! 334         change: ActivityChange,
! 335     ) -> Result<ActivityChange, WorkflowError> {
! 336         if let Some(id) = change.bike_activity_id {
! 337             if let Some(projection) = crate::entities::heatmap_projections::Entity::find_by_id(id)
! 338                 .one(&self.db)
! 339                 .await?
  340             {
! 341                 crate::heatmaps::preparation::prepare_activity(
! 342                     &self.db,
! 343                     crate::heatmaps::projection::PendingProjection {
! 344                         activity_id: id,
! 345                         generation: projection.generation,
! 346                     },
! 347                 )
! 348                 .await?;
! 349             }
! 350         }
! 351         Ok(change)
! 352     }
  353 
! 354     pub async fn complete_activity(
! 355         &self,
! 356         change: ActivityChange,
! 357     ) -> Result<ActivityChange, WorkflowError> {
! 358         if let Some(id) = change.import_id {
! 359             if let Some(import) =
! 360                 activity_imports::Entity::find_owned(&self.db, change.event.bike_user_id, id)
! 361                     .await?
  362             {
! 363                 let mut active: activity_imports::ActiveModel = import.into();
! 364                 active.status =
! 365                     Set(crate::activity_import_lifecycle::ACTIVITY_IMPORT_STATUS_PROCESSED.into());
! 366                 active.processing_stage =
! 367                     Set(crate::activity_import_lifecycle::ACTIVITY_IMPORT_STAGE_COMPLETE.into());
! 368                 active.processed_at = Set(Some(chrono::Utc::now()));
! 369                 active.processing_error = Set(None);
! 370                 active.update(&self.db).await?;
! 371             }
! 372         }
! 373         Ok(change)
! 374     }
  375 }
```


---



## bike-rs/bike-core/src/strava/native/scan&#46;rs

```
   1 use super::*;
   2 
   3 impl StravaOperations {
!  4     pub async fn scan_connections(&self, request: ScanRequest) -> Result<ScanPage, WorkflowError> {
!  5         let connections =
!  6             strava_connections::Model::scan_page(&self.db, request.after_connection_id).await?;
!  7         let next_connection_id = (connections.len() == 20)
!  8             .then(|| connections.last().map(|c| c.id))
!  9             .flatten();
! 10         let mut events = Vec::new();
! 11         for connection in connections {
! 12             if !super::super::scopes_allow_activity_import(&connection.scopes) {
! 13                 continue;
! 14             }
! 15             let user =
! 16                 crate::auth::entities::users::Model::find_by_id(&self.db, connection.user_id)
! 17                     .await?;
! 18             let mut event = StravaEvent::new(
! 19                 &user,
! 20                 &connection,
! 21                 super::super::events::StravaOperation::Sync,
  22             );
! 23             event.strava_event_id =
! 24                 uuid::Uuid::new_v5(&request.correlation_id, &connection.id.to_be_bytes());
! 25             event.correlation_id = request.correlation_id;
! 26             events.push(self.accept_child(event).await?);
  27         }
! 28         Ok(ScanPage {
! 29             events,
! 30             next_connection_id,
! 31         })
! 32     }
  33 }
```


---



## bike-rs/bike-core/src/strava/native/strava_native_unit/handlers_fixture&#46;rs

```
  24             .lock()
  25             .unwrap()
  26             .get(&object.path)
  27             .cloned()
! 28             .ok_or_else(|| WorkflowError::internal("Missing fixture object"))
  29     }
  30 
  31     async fn write(&self, key: &str, bytes: &[u8]) -> Result<ObjectRef, WorkflowError> {
  32         self.check()?;
```


---


```
  81     fn resource(&self) -> &'static str {
  82         match self {
  83             Self::Activity => "activity",
  84             Self::Streams => "streams",
! 85             Self::Page { .. } => "page",
! 86             Self::Revoke => "deauthorize",
  87         }
  88     }
  89 }
```


---



## bike-rs/bike-core/src/strava/oauth_state&#46;rs

```
  32     pub error: Option<String>,
  33 }
  34 
  35 impl AuthorizationCallback {
! 36     pub fn is_incomplete(&self) -> bool {
! 37         let missing_fields = self.code.is_none() || self.state.is_none();
! 38         self.error.is_some() || missing_fields
! 39     }
  40 }
  41 
  42 #[async_trait::async_trait]
  43 pub trait AuthorizationExchange: Send + Sync {
```


---


```
  53     async fn exchange(
  54         &self,
  55         code: &str,
  56         state: VerifiedStravaState,
! 57     ) -> Result<strava_connections::Model, WorkflowError> {
  58         super::exchange_code_for_connection(self, code, state).await
! 59     }
  60 }
  61 
  62 #[async_trait::async_trait]
  63 pub trait OAuthStateStore: Send + Sync {
```


---


```
  66 }
  67 
  68 #[async_trait::async_trait]
  69 impl OAuthStateStore for DatabaseConnection {
! 70     async fn save(&self, state: &AuthorizationState) -> Result<(), WorkflowError> {
  71         oauth_states::Model::save(self, PROVIDER, &state.token, &state.browser_session)
  72             .await
  73             .map_err(Into::into)
! 74     }
  75 
  76     async fn consume(&self, token: &str) -> Result<(), WorkflowError> {
  77         oauth_states::Model::consume(self, PROVIDER, token)
  78             .await
```


---


```
  163         if issued_at > now || now >= issued_at + Duration::seconds(STATE_TTL_SECONDS) {
  164             return Err(invalid_state());
  165         }
  166         if user_id <= 0 || Uuid::parse_str(nonce).is_err() {
! 167             return Err(invalid_state());
  168         }
  169         Ok(VerifiedStravaState {
  170             user_id,
  171             issued_at,
```


---



## bike-rs/bike-core/src/strava/transport&#46;rs

```
  82         if let Some(context) = &context {
  83             let request_id = raw
  84                 .headers
  85                 .get("x-request-id")
! 86                 .and_then(|value| value.to_str().ok());
  87             context.exchange(
  88                 &id.to_string(),
  89                 &method,
  90                 &path,
```


---


