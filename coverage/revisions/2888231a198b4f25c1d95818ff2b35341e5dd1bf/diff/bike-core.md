# Diff Coverage
## Diff: 220ecaaddf2cf996aca01a3d809b181ef9c3cd3f...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-core/src/activity_import_execution&#46;rs (85.7%): Missing lines 158
- bike-rs/bike-core/src/activity_import_lifecycle&#46;rs (100%)
- bike-rs/bike-core/src/activity_import_lock&#46;rs (100%)
- bike-rs/bike-core/src/activity_import_pipeline&#46;rs (100%)
- bike-rs/bike-core/src/activity_import_pipeline/replay&#46;rs (100%)
- bike-rs/bike-core/src/activity_import_pipeline/stage_execution&#46;rs (100%)
- bike-rs/bike-core/src/activity_import_recovery&#46;rs (100%)
- bike-rs/bike-core/src/activity_source_recovery&#46;rs (66.7%): Missing lines 631-633,719
- bike-rs/bike-core/src/activity_training_analysis&#46;rs (100%)
- bike-rs/bike-core/src/analytics&#46;rs (50.0%): Missing lines 209,670,683-686
- bike-rs/bike-core/src/archive_import&#46;rs (57.1%): Missing lines 100-102
- bike-rs/bike-core/src/auth/entities/users&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/entities/background_tasks&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/entities/work_units&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/execution&#46;rs (100%)
- bike-rs/bike-core/src/entities/activities/parsed&#46;rs (96.5%): Missing lines 137,149
- bike-rs/bike-core/src/entities/activity_analytics&#46;rs (100%)
- bike-rs/bike-core/src/entities/activity_archive_import_jobs&#46;rs (98.0%): Missing lines 226,229
- bike-rs/bike-core/src/entities/activity_import_artifacts&#46;rs (100%)
- bike-rs/bike-core/src/entities/activity_import_attempts&#46;rs (100%)
- bike-rs/bike-core/src/entities/activity_import_attempts/mutations&#46;rs (90.2%): Missing lines 34-35,38,42,44,74-80
- bike-rs/bike-core/src/entities/activity_import_locks&#46;rs (57.3%): Missing lines 89-96,98,112,114,116,119-124,153-160,162-167
- bike-rs/bike-core/src/entities/activity_imports/mutations&#46;rs (100%)
- bike-rs/bike-core/src/entities/activity_training_analyses&#46;rs (100%)
- bike-rs/bike-core/src/entities/analytics_user_states&#46;rs (40.9%): Missing lines 96-104,106-112,115-116,124-131
- bike-rs/bike-core/src/entities/fitness_freshness_daily&#46;rs (100%)
- bike-rs/bike-core/src/entities/provider_rate_limit_buckets&#46;rs (74.6%): Missing lines 108-109,126,130,145-148,150-155,157-160
- bike-rs/bike-core/src/entities/segment_efforts&#46;rs (0.0%): Missing lines 271-281
- bike-rs/bike-core/src/entities/segment_summaries&#46;rs (100%)
- bike-rs/bike-core/src/entities/segment_user_summaries&#46;rs (0.0%): Missing lines 108-125
- bike-rs/bike-core/src/entities/segments&#46;rs (100%)
- bike-rs/bike-core/src/entities/segments/mutations&#46;rs (90.4%): Missing lines 39,62,105-108,112-113,116,118-119,122,125-128,132-135,205,222,282-285
- bike-rs/bike-core/src/entities/strava_connections/mutations&#46;rs (97.6%): Missing lines 48,70
- bike-rs/bike-core/src/entities/user_preferences/mutations&#46;rs (95.1%): Missing lines 68,266-269,278-280,292-295,309-311
- bike-rs/bike-core/src/provider_rate_limit&#46;rs (100%)
- bike-rs/bike-core/src/services/preferences&#46;rs (100%)
- bike-rs/bike-core/src/strava/connection&#46;rs (0.0%): Missing lines 303,323
- bike-rs/bike-core/src/strava/native/persistence&#46;rs (0.0%): Missing lines 391
- bike-rs/bike-core/src/test_support/schema&#46;rs (100%)
- bike-rs/bike-core/src/workflow_error&#46;rs (71.4%): Missing lines 169-170
- bike-rs/bike-core/src/xc_goal_backfill&#46;rs (100%)

## Summary

- **Total**: 1673 lines
- **Missing**: 183 lines
- **Coverage**: 89.06%



## bike-rs/bike-core/src/activity_import_execution&#46;rs

```
  154 ) -> Result<(), WorkflowError> {
  155     attempts::Model::finish_active(db, import, outcome, error).await
  156 }
  157 
! 158 pub(crate) fn json_error(_: serde_json::Error) -> WorkflowError {
  159     WorkflowError::internal("Could not serialize ingestion execution state")
  160 }
  161 
  162 pub async fn record_batch_stage(
```


---



## bike-rs/bike-core/src/activity_source_recovery&#46;rs

```
  627             "Recovery activity changed; plan it again",
  628         ));
  629     }
  630     if artifact.activity_import_id != plan.import_id {
! 631         artifact = artifact
! 632             .copy_to_import(&transaction, plan.import_id)
! 633             .await?;
  634     }
  635     current
  636         .adopt_source_metadata(&transaction, &artifact)
  637         .await?;
```


---


```
  715         Ok(()) => (),
  716         Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
  717         Err(error) => return Err(error.into()),
  718     }
! 719     artifact.delete(&transaction).await?;
  720     transaction.commit().await?;
  721     Ok(true)
  722 }
```


---



## bike-rs/bike-core/src/analytics&#46;rs

```
  205 ) -> Result<(), sea_orm::DbErr>
  206 where
  207     C: ConnectionTrait,
  208 {
! 209     analytics_user_states::Model::mark_changed(db, user_id, changed_at).await
  210 }
  211 
  212 pub async fn mark_user_fitness_dirty<C>(
  213     db: &C,
```


---


```
  666         segment_summaries::Model::store_summary(db, segment_id, summary).await?;
  667     }
  668 
  669     for ((segment_id, user_id), summary) in rebuild.segment_user_summary_by_key {
! 670         segment_user_summaries::Model::store_summary(db, segment_id, user_id, summary).await?;
  671     }
  672 
  673     Ok(written)
  674 }
```


---


```
  679 ) -> Result<(), sea_orm::DbErr>
  680 where
  681     C: ConnectionTrait,
  682 {
! 683     update
! 684         .effort
! 685         .set_ranks(db, update.overall_rank, update.user_rank)
! 686         .await?;
  687     Ok(())
  688 }
  689 
  690 #[tracing::instrument(name = "activity.analytics.rebuild", skip_all, fields(activity_count = activity_ids.len()))]
```


---



## bike-rs/bike-core/src/archive_import&#46;rs

```
   96         ACTIVITY_IMPORT_LOCK_STAGE_QUEUED,
   97     )
   98     .await?;
   99 
! 100     let job =
! 101         activity_archive_import_jobs::Model::queue(db, user_id, user_storage_key, archive_url)
! 102             .await;
  103 
  104     let job = match job {
  105         Ok(job) => job,
  106         Err(error) => {
```


---



## bike-rs/bike-core/src/entities/activities/parsed&#46;rs

```
  133         save_data: ActivitySaveData,
  134     ) -> Result<activities::Model, AppError> {
  135         let activity_type = self.activity_type.clone();
  136         let archive_title = (self.source == "strava_sync"
! 137             && parsing_artifact.artifact_kind == ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL)
  138             .then(|| self.title.clone());
  139         let mut active_model: activities::ActiveModel = self.into();
  140         apply_fields(
  141             &mut active_model,
```


---


```
  145             save_data,
  146         );
  147         active_model.activity_type = Set(activity_type);
  148         if let Some(title) = archive_title {
! 149             active_model.title = Set(title);
  150         }
  151         active_model.update(db).await.map_err(AppError::from)
  152     }
```


---



## bike-rs/bike-core/src/entities/activity_archive_import_jobs&#46;rs

```
  222             status: Set(archive_outcome(summary).into()),
  223             failure_message: Set(None),
  224             error_samples_json: Set(Some(
  225                 serde_json::to_string(&summary.error_samples).map_err(|error| {
! 226                     AppError::internal(format!(
  227                         "Failed to serialize archive import errors: {error}"
  228                     ))
! 229                 })?,
  230             )),
  231             finished_at: Set(Some(Utc::now())),
  232             ..Default::default()
  233         }
```


---



## bike-rs/bike-core/src/entities/activity_import_attempts/mutations&#46;rs

```
  30             checkpoint_json: Set(previous.and_then(|attempt| attempt.checkpoint_json.clone())),
  31             ..Default::default()
  32         };
  33         let attempt = attempt.insert(db).await.map_err(|error| {
! 34             if matches!(
! 35                 error.sql_err(),
  36                 Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
  37             ) {
! 38                 WorkflowError::conflict(
  39                     "This import or its activity already has a queued or running attempt",
  40                 )
  41             } else {
! 42                 WorkflowError::from(error)
  43             }
! 44         })?;
  45         crate::background_jobs::entities::pipeline_outputs::Model::require(
  46             db,
  47             "import",
  48             import.id,
```


---


```
  70         active.current_stage = Set(record.stage);
  71         active.stages_json = Set(serde_json::to_value(records).map_err(json_error)?);
  72         active.updated_at = Set(Utc::now());
  73         if let Some(context) = crate::background_jobs::execution::ExecutionContext::current() {
! 74             active.worker_task_id = Set(Some(context.task_id));
! 75             crate::background_jobs::entities::task_attempts::Model::progress(
! 76                 db,
! 77                 context.task_id,
! 78                 context.attempt,
! 79             )
! 80             .await?;
  81         }
  82         if let Some(id) = activity_id {
  83             active.activity_id = Set(Some(id));
  84             record_activity_lineage(db, id).await?;
```


---



## bike-rs/bike-core/src/entities/activity_import_locks&#46;rs

```
   85         .await;
   86 
   87         match lock {
   88             Ok(lock) => Ok(lock),
!  89             Err(error) => {
!  90                 if let Some(existing) = Self::for_user(db, user_id).await? {
!  91                     return Err(ActivityImportLockError::conflict(format!(
!  92                         "Another activity processing operation is already {} for this user ({})",
!  93                         describe_stage(&existing.stage),
!  94                         describe_source(&existing.source),
!  95                     )));
!  96                 }
   97 
!  98                 Err(ActivityImportLockError::from(error))
   99             }
  100         }
  101     }
  102 }
```


---


```
  108         source: &str,
  109         stage: &str,
  110     ) -> Result<activity_import_locks::Model, ActivityImportLockError> {
  111         let lock = Self::for_user(db, user_id).await?.ok_or_else(|| {
! 112             ActivityImportLockError::internal(format!(
  113                 "Activity import lock for user {user_id} was missing while starting {}",
! 114                 describe_source(source),
  115             ))
! 116         })?;
  117 
  118         if lock.source != source {
! 119             return Err(ActivityImportLockError::internal(format!(
! 120                 "Activity import lock for user {} is owned by {} instead of {}",
! 121                 user_id,
! 122                 describe_source(&lock.source),
! 123                 describe_source(source),
! 124             )));
  125         }
  126 
  127         let mut active_model: activity_import_locks::ActiveModel = lock.into();
  128         active_model.stage = Set(stage.to_string());
```


---


```
  149             );
  150             return Self::acquire(db, user_id, source, stage).await;
  151         };
  152 
! 153         if lock.source != source {
! 154             return Err(ActivityImportLockError::internal(format!(
! 155                 "Activity import lock for user {} is owned by {} instead of {}",
! 156                 user_id,
! 157                 describe_source(&lock.source),
! 158                 describe_source(source),
! 159             )));
! 160         }
  161 
! 162         let mut active_model: activity_import_locks::ActiveModel = lock.into();
! 163         active_model.stage = Set(stage.to_string());
! 164         active_model
! 165             .update(db)
! 166             .await
! 167             .map_err(ActivityImportLockError::from)
  168     }
  169 }
```


---



## bike-rs/bike-core/src/entities/analytics_user_states&#46;rs

```
   92     }
   93 }
   94 
   95 impl Model {
!  96     pub async fn mark_changed(
!  97         db: &impl ConnectionTrait,
!  98         user_id: i32,
!  99         changed_at: DateTime<Utc>,
! 100     ) -> Result<(), sea_orm::DbErr> {
! 101         if let Some(model) = analytics_user_states::Model::for_user(db, user_id).await? {
! 102             let mut active_model: analytics_user_states::ActiveModel = model.into();
! 103             active_model.last_activity_change_at = Set(changed_at);
! 104             active_model.update(db).await?;
  105         } else {
! 106             analytics_user_states::ActiveModel {
! 107                 user_id: Set(user_id),
! 108                 last_activity_change_at: Set(changed_at),
! 109                 ..Default::default()
! 110             }
! 111             .insert(db)
! 112             .await?;
  113         }
  114 
! 115         Ok(())
! 116     }
  117     pub async fn mark_dirty(
  118         db: &impl ConnectionTrait,
  119         user_id: i32,
  120         dirty_from_day: NaiveDate,
```


---


```
  120         dirty_from_day: NaiveDate,
  121         changed_at: DateTime<Utc>,
  122     ) -> Result<(), DbErr> {
  123         if let Some(model) = analytics_user_states::Model::for_user(db, user_id).await? {
! 124             let next_dirty_from_day = model
! 125                 .fitness_dirty_from_day
! 126                 .map(|existing| existing.min(dirty_from_day))
! 127                 .unwrap_or(dirty_from_day);
! 128             let mut active_model: analytics_user_states::ActiveModel = model.into();
! 129             active_model.last_activity_change_at = Set(changed_at);
! 130             active_model.fitness_dirty_from_day = Set(Some(next_dirty_from_day));
! 131             active_model.update(db).await?;
  132         } else {
  133             analytics_user_states::ActiveModel {
  134                 user_id: Set(user_id),
  135                 last_activity_change_at: Set(changed_at),
```


---



## bike-rs/bike-core/src/entities/provider_rate_limit_buckets&#46;rs

```
  104         now: DateTime<Utc>,
  105     ) -> Result<provider_rate_limit_buckets::Model, AppError> {
  106         let mut active_model: provider_rate_limit_buckets::ActiveModel = self.into();
  107         if active_model.reset_at.as_ref() <= &now {
! 108             active_model.used_count = Set(spec.units);
! 109             active_model.reset_at = Set(now + spec.window);
  110         } else {
  111             let used_count = *active_model.used_count.as_ref() + spec.units;
  112             active_model.used_count = Set(used_count);
  113         }
```


---


```
  122         used_count: i32,
  123         reset_at: DateTime<Utc>,
  124     ) -> Result<Option<Self>, AppError> {
  125         if limit_count <= 0 || used_count < 0 {
! 126             return Ok(None);
  127         }
  128 
  129         let now = Utc::now();
! 130         let Some(row) =
  131             provider_rate_limit_buckets::Model::find_bucket(db, provider, bucket).await?
  132         else {
  133             let row = provider_rate_limit_buckets::ActiveModel {
  134                 provider: Set(provider.to_string()),
```


---


```
  141             let row = row.insert(db).await?;
  142             return Ok(Some(row));
  143         };
  144 
! 145         let should_reconcile = row.needs_reconciliation(now, limit_count, used_count);
! 146         if !should_reconcile {
! 147             return Ok(Some(row));
! 148         }
  149 
! 150         let mut active_model: provider_rate_limit_buckets::ActiveModel = row.into();
! 151         active_model.limit_count = Set(limit_count);
! 152         active_model.used_count = Set(used_count);
! 153         active_model.reset_at = Set(reset_at);
! 154         let updated = active_model.update(db).await?;
! 155         Ok(Some(updated))
  156     }
! 157     fn needs_reconciliation(&self, now: DateTime<Utc>, limit_count: i32, used_count: i32) -> bool {
! 158         let usage_changed = used_count > self.used_count || limit_count != self.limit_count;
! 159         self.reset_at <= now || usage_changed
! 160     }
  161     pub async fn initialize(
  162         db: &impl ConnectionTrait,
  163         spec: &ProviderQuotaBucketSpec,
  164         now: DateTime<Utc>,
```


---



## bike-rs/bike-core/src/entities/segment_efforts&#46;rs

```
  267     }
  268 }
  269 
  270 impl Model {
! 271     pub async fn set_ranks(
! 272         self,
! 273         db: &impl ConnectionTrait,
! 274         overall_rank: i32,
! 275         user_rank: i32,
! 276     ) -> Result<Self, DbErr> {
! 277         let mut active_model: ActiveModel = self.into();
! 278         active_model.overall_rank = Set(Some(overall_rank));
! 279         active_model.user_rank = Set(Some(user_rank));
! 280         active_model.update(db).await
! 281     }
  282 }
```


---



## bike-rs/bike-core/src/entities/segment_user_summaries&#46;rs

```
  104     }
  105 }
  106 
  107 impl Model {
! 108     pub(crate) async fn store_summary(
! 109         db: &impl ConnectionTrait,
! 110         segment_id: i32,
! 111         user_id: i32,
! 112         summary: SegmentUserSummaryAccumulator,
! 113     ) -> Result<(), DbErr> {
! 114         segment_user_summaries::ActiveModel {
! 115             segment_id: Set(segment_id),
! 116             user_id: Set(user_id),
! 117             effort_count: Set(summary.effort_count),
! 118             personal_best_effort_id: Set(summary.personal_best_effort_id),
! 119             personal_best_duration_seconds: Set(summary.personal_best_duration_seconds),
! 120             ..Default::default()
! 121         }
! 122         .insert(db)
! 123         .await?;
! 124         Ok(())
! 125     }
  126 }
```


---



## bike-rs/bike-core/src/entities/segments/mutations&#46;rs

```
  35         self.distance_meters
  36     }
  37     fn require_owner(&self, user_id: i32) -> Result<(), AppError> {
  38         if self.activity.user_id != user_id {
! 39             return Err(AppError::not_found("Activity not found"));
  40         }
  41         Ok(())
  42     }
```


---


```
  58         let route_points = slice_builder_route_points(
  59             &activity_route_points,
  60             start_route_point_index,
  61             end_route_point_index,
! 62         )?;
  63         let distance_meters = route_points.last().and_then(|point| point.distance_meters);
  64 
  65         Ok(Self {
  66             activity,
```


---


```
  101     start_route_point_index: i32,
  102     end_route_point_index: i32,
  103 ) -> Result<Vec<ActivityRoutePoint>, AppError> {
  104     if route_points.len() < 2 {
! 105         return Err(AppError::validation_field(
! 106             "activity_id",
! 107             "Selected activity does not have enough route data to build a segment",
! 108         ));
  109     }
  110 
  111     let start_index = usize::try_from(start_route_point_index).map_err(|_| {
! 112         AppError::validation_field(
! 113             "start_route_point_index",
  114             "Segment start must be within the selected activity route",
  115         )
! 116     })?;
  117     let end_index = usize::try_from(end_route_point_index).map_err(|_| {
! 118         AppError::validation_field(
! 119             "end_route_point_index",
  120             "Segment end must be within the selected activity route",
  121         )
! 122     })?;
  123 
  124     if start_index >= route_points.len() {
! 125         return Err(AppError::validation_field(
! 126             "start_route_point_index",
! 127             "Segment start must be within the selected activity route",
! 128         ));
  129     }
  130 
  131     if end_index >= route_points.len() {
! 132         return Err(AppError::validation_field(
! 133             "end_route_point_index",
! 134             "Segment end must be within the selected activity route",
! 135         ));
  136     }
  137 
  138     if start_index >= end_index {
  139         return Err(AppError::validation_field(
```


---


```
  201         let should_update_title = self.title != source.title;
  202         let should_update_builder_source = !source.matches_builder(&self);
  203 
  204         if !should_update_title && !should_update_builder_source {
! 205             return Ok(self);
  206         }
  207 
  208         let activity_ids = if should_update_title {
  209             segment_efforts::Model::activity_ids_for_segments(db, &[self.id]).await?
```


---


```
  218 
  219         let updated_segment = active_segment.update(&txn).await?;
  220 
  221         if !activity_ids.is_empty() {
! 222             rebuild_activity_analytics_cache(&txn, &activity_ids).await?;
  223         }
  224 
  225         txn.commit().await?;
```


---


```
  278         let segment_summary = summarize_activity_upload(filename, format, bytes)?;
  279         let segment_detail = derive_activity_detail_data(filename, format, bytes)?;
  280 
  281         if segment_detail.route_points.len() < 2 {
! 282             return Err(AppError::validation_field(
! 283                 "file",
! 284                 "Segment imports require GPX or TCX route data with coordinates",
! 285             ));
  286         }
  287 
  288         if let Some(existing_segment) = segments::Model::find_duplicate(
  289             db,
```


---



## bike-rs/bike-core/src/entities/strava_connections/mutations&#46;rs

```
  44         .with_tokens(
  45             &token_response.access_token,
  46             &token_response.refresh_token,
  47             expires_at,
! 48         )?;
  49         if reconnect {
  50             active_model.update(db).await.map_err(AppError::from)
  51         } else {
  52             active_model.insert(db).await.map_err(AppError::from)
```


---


```
  66         .with_tokens(
  67             &token_response.access_token,
  68             &token_response.refresh_token,
  69             expires_at,
! 70         )?;
  71         if let Some(athlete) = token_response.athlete {
  72             active_model = active_model.with_athlete(athlete);
  73         }
  74         active_model.update(db).await.map_err(AppError::from)
```


---



## bike-rs/bike-core/src/entities/user_preferences/mutations&#46;rs

```
  64             self.xc_goal_target_elevation_gain_meters,
  65             self.xc_goal_event_name.as_deref(),
  66             self.xc_goal_target_finish_time_seconds,
  67             self.xc_goal_event_profile.as_deref(),
! 68         )?;
  69         Ok(ValidatedPreferences {
  70             unit_system,
  71             estimated_ftp_watts,
  72             heart_rate_zone_bounds_json: serialize_heart_rate_zone_bounds(zones.as_deref())?,
```


---


```
  262         return Ok(None);
  263     };
  264 
  265     if value.chars().count() > XC_GOAL_EVENT_NAME_MAX_LENGTH {
! 266         return Err(WorkflowError::validation_field(
! 267             "xc_goal_event_name",
! 268             "XC goal event name must be 120 characters or fewer",
! 269         ));
  270     }
  271 
  272     Ok(Some(value.to_string()))
  273 }
```


---


```
  274 
  275 fn validate_target_finish_time_seconds(value: Option<i32>) -> Result<Option<i32>, WorkflowError> {
  276     match value {
  277         Some(seconds) if seconds <= 0 => Err(WorkflowError::validation_field(
! 278             "xc_goal_target_finish_time_seconds",
! 279             "Target finish time must be greater than zero",
! 280         )),
  281         _ => Ok(value),
  282     }
  283 }
```


---


```
  288     };
  289     let normalized = value.to_ascii_lowercase();
  290 
  291     if !XC_GOAL_EVENT_PROFILES.contains(&normalized.as_str()) {
! 292         return Err(WorkflowError::validation_field(
! 293             "xc_goal_event_profile",
! 294             "XC goal event profile is not supported",
! 295         ));
  296     }
  297 
  298     Ok(Some(normalized))
  299 }
```


---


```
  305     max_message: &str,
  306 ) -> Result<Option<f64>, WorkflowError> {
  307     match value {
  308         Some(next) if !next.is_finite() || next <= 0.0 => Err(WorkflowError::validation_field(
! 309             field,
! 310             "Value must be greater than zero",
! 311         )),
  312         Some(next) if next > max_value => Err(WorkflowError::validation_field(field, max_message)),
  313         _ => Ok(value),
  314     }
  315 }
```


---



## bike-rs/bike-core/src/strava/connection&#46;rs

```
  299     db: &DatabaseConnection,
  300     user_id: i32,
  301     token_response: StravaAuthorizationTokenResponse,
  302 ) -> Result<strava_connections::Model, AppError> {
! 303     strava_connections::Model::connect_from_token(db, user_id, token_response).await
  304 }
  305 
  306 pub(crate) async fn ensure_fresh_access_token(
  307     db: &DatabaseConnection,
```


---


```
  319     };
  320     let token_response = archive
  321         .scope(client.refresh_access_token(&refresh_token))
  322         .await?;
! 323     connection.refresh_credentials(db, token_response).await
  324 }
  325 
  326 pub(crate) fn build_activity_upload(
  327     activity: &StravaActivitySummary,
```


---



## bike-rs/bike-core/src/strava/native/persistence&#46;rs

```
  387             if let Some(import) =
  388                 activity_imports::Entity::find_owned(&self.db, change.event.bike_user_id, id)
  389                     .await?
  390             {
! 391                 import.mark_native_processed(&self.db).await?;
  392             }
  393         }
  394         Ok(change)
  395     }
```


---



## bike-rs/bike-core/src/workflow_error&#46;rs

```
  165 impl From<crate::xc_goal_backfill::XcGoalBackfillError> for WorkflowError {
  166     fn from(error: crate::xc_goal_backfill::XcGoalBackfillError) -> Self {
  167         use crate::xc_goal_backfill::XcGoalBackfillError;
  168         match error {
! 169             XcGoalBackfillError::ActivityImportLock(error) => error.into(),
! 170             XcGoalBackfillError::Database(error) => error.into(),
  171             XcGoalBackfillError::Queue(message) => {
  172                 Self::internal(format!("Failed to queue XC training backfill: {message}"))
  173             }
  174         }
```


---


