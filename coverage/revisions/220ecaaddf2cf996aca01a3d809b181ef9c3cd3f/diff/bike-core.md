# Diff Coverage
## Diff: 87f8fa643022dbf795def001041f74ffd2c02e34...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-core/src/activity_import_execution&#46;rs (0.0%): Missing lines 183,197
- bike-rs/bike-core/src/activity_import_lifecycle&#46;rs (100%)
- bike-rs/bike-core/src/activity_import_lock&#46;rs (100%)
- bike-rs/bike-core/src/activity_import_pipeline&#46;rs (100%)
- bike-rs/bike-core/src/activity_import_pipeline/replay&#46;rs (100%)
- bike-rs/bike-core/src/activity_import_pipeline/stage_execution&#46;rs (100%)
- bike-rs/bike-core/src/activity_import_recovery&#46;rs (66.7%): Missing lines 47
- bike-rs/bike-core/src/activity_lifecycle&#46;rs (8.3%): Missing lines 246,257-265,541-549,558,735,759
- bike-rs/bike-core/src/activity_training_analysis&#46;rs (90.9%): Missing lines 358
- bike-rs/bike-core/src/analytics&#46;rs (92.9%): Missing lines 212
- bike-rs/bike-core/src/archive_import&#46;rs (50.0%): Missing lines 152
- bike-rs/bike-core/src/archive_import/batch&#46;rs (100%)
- bike-rs/bike-core/src/auth/controllers/auth&#46;rs (100%)
- bike-rs/bike-core/src/auth/entities/refresh_tokens&#46;rs (100%)
- bike-rs/bike-core/src/auth/entities/users&#46;rs (100%)
- bike-rs/bike-core/src/auth/extractors/auth_info&#46;rs (100%)
- bike-rs/bike-core/src/auth/services/oauth&#46;rs (76.9%): Missing lines 219-220,227
- bike-rs/bike-core/src/background_jobs/admin&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/batches&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/batches/reprocessing&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/batches/segments&#46;rs (30.0%): Missing lines 78,102-107
- bike-rs/bike-core/src/background_jobs/diagnostics&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/diagnostics/graph&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/diagnostics/health&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/diagnostics/statistics&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/entities/background_tasks&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/entities/background_tasks/admin&#46;rs (77.5%): Missing lines 39,42,45,48-50,53-55
- bike-rs/bike-core/src/background_jobs/entities/background_tasks/diagnostics&#46;rs (99.2%): Missing lines 78
- bike-rs/bike-core/src/background_jobs/entities/batch_tasks&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/entities/pipeline_outputs&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/entities/pipeline_runs&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/entities/pipeline_runs/diagnostics&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/entities/pipeline_subjects&#46;rs (96.4%): Missing lines 108
- bike-rs/bike-core/src/background_jobs/entities/pipeline_tasks&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/entities/processor_registry&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/entities/task_anomalies&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/entities/task_attempts&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/entities/task_attempts/statistics&#46;rs (21.4%): Missing lines 20-39,41-53
- bike-rs/bike-core/src/background_jobs/entities/work_units&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/entities/worker_batches&#46;rs (100%)
- bike-rs/bike-core/src/entities/activities&#46;rs (78.6%): Missing lines 782-787,799-809,836-860
- bike-rs/bike-core/src/entities/activities/reports&#46;rs (100%)
- bike-rs/bike-core/src/entities/activities/training&#46;rs (100%)
- bike-rs/bike-core/src/entities/activity_archive_import_jobs&#46;rs (100%)
- bike-rs/bike-core/src/entities/activity_import_artifacts&#46;rs (96.0%): Missing lines 195
- bike-rs/bike-core/src/entities/activity_import_attempts&#46;rs (100%)
- bike-rs/bike-core/src/entities/activity_import_locks&#46;rs (100%)
- bike-rs/bike-core/src/entities/activity_imports&#46;rs (79.5%): Missing lines 354-368,380
- bike-rs/bike-core/src/entities/activity_training_analyses&#46;rs (52.8%): Missing lines 84,86-95,97-102
- bike-rs/bike-core/src/entities/analytics_user_states&#46;rs (100%)
- bike-rs/bike-core/src/entities/fitness_freshness_daily&#46;rs (55.6%): Missing lines 99-110
- bike-rs/bike-core/src/entities/heatmap_chunks&#46;rs (100%)
- bike-rs/bike-core/src/entities/heatmap_projections&#46;rs (100%)
- bike-rs/bike-core/src/entities/heatmap_projections/data&#46;rs (100%)
- bike-rs/bike-core/src/entities/heatmap_projections/lifecycle&#46;rs (74.5%): Missing lines 61,64,97-100,102,104-110,112-113,116-157,159-161
- bike-rs/bike-core/src/entities/heatmap_user_states&#46;rs (100%)
- bike-rs/bike-core/src/entities/segment_efforts&#46;rs (100%)
- bike-rs/bike-core/src/entities/segment_summaries&#46;rs (100%)
- bike-rs/bike-core/src/entities/segment_user_summaries&#46;rs (100%)
- bike-rs/bike-core/src/entities/segments&#46;rs (98.6%): Missing lines 99
- bike-rs/bike-core/src/entities/segments/access&#46;rs (93.9%): Missing lines 36,53-54,68
- bike-rs/bike-core/src/entities/strava_connections&#46;rs (56.2%): Missing lines 173-179
- bike-rs/bike-core/src/entities/user_preferences&#46;rs (100%)
- bike-rs/bike-core/src/platform/feature_flags/entities&#46;rs (100%)
- bike-rs/bike-core/src/platform/feature_flags/service&#46;rs (100%)
- bike-rs/bike-core/src/segment_regeneration&#46;rs (75.0%): Missing lines 63
- bike-rs/bike-core/src/segment_support&#46;rs (25.0%): Missing lines 201,228,298
- bike-rs/bike-core/src/services/reports&#46;rs (0.0%): Missing lines 880-886,967-972,986,1071-1077,1088-1095,1127-1133,1151-1159
- bike-rs/bike-core/src/services/training_goals&#46;rs (0.0%): Missing lines 477,482-483,486,501,510,516-517,670,711,721-722
- bike-rs/bike-core/src/strava/connection&#46;rs (40.0%): Missing lines 270,299-300
- bike-rs/bike-core/src/strava/native/persistence&#46;rs (0.0%): Missing lines 313,366-367
- bike-rs/bike-core/src/test_support/accounts&#46;rs (100%)
- bike-rs/bike-core/src/test_support/analytics&#46;rs (100%)
- bike-rs/bike-core/src/test_support/heatmaps&#46;rs (100%)
- bike-rs/bike-core/src/test_support/imports&#46;rs (100%)
- bike-rs/bike-core/src/test_support/mod&#46;rs (100%)
- bike-rs/bike-core/src/test_support/segments&#46;rs (100%)
- bike-rs/bike-core/src/training_profile&#46;rs (100%)
- bike-rs/bike-core/src/xc_goal_backfill&#46;rs (100%)

## Summary

- **Total**: 2494 lines
- **Missing**: 310 lines
- **Coverage**: 87.57%



## bike-rs/bike-core/src/activity_import_execution&#46;rs

```
  179         return Ok(());
  180     };
  181     pipeline_runs::Model::record_origin(db, &context).await?;
  182     pipeline_subjects::Model::attach(db, &context.run_id, "activity", activity_id).await?;
! 183     if let Some(activity) = crate::entities::activities::Model::find_by_id(db, activity_id).await? {
  184         let revision = activity.updated_at.timestamp_micros().to_string();
  185         pipeline_outputs::Model::require(db, "activity", activity_id, revision.clone()).await?;
  186         pipeline_outputs::Model::publish(
  187             db,
```


---


```
  193         .await?;
  194     }
  195     if crate::heatmaps::projection::Projection::enabled(db).await? {
  196         if let Some(projection) =
! 197             crate::entities::heatmap_projections::Model::for_activity(db, activity_id).await?
  198         {
  199             let revision = crate::heatmaps::projection::Projection::revision(projection.generation);
  200             pipeline_outputs::Model::require(db, "heatmap", activity_id, revision.clone()).await?;
  201             if ["ready", "skipped"].contains(&projection.status.as_str()) {
```


---



## bike-rs/bike-core/src/activity_import_recovery&#46;rs

```
  43     tasks: &JobQueue,
  44     now: DateTime<Utc>,
  45 ) -> Result<usize, ActivityImportRecoveryError> {
  46     let stale_before = now - ChronoDuration::seconds(ACTIVITY_IMPORT_STALE_PROCESSING_SECONDS);
! 47     let user_ids = activity_imports::Entity::stale_manual_owner_ids(db, stale_before).await?;
  48 
  49     let mut recovered_count = 0usize;
  50     for user_id in user_ids {
  51         recovered_count +=
```


---



## bike-rs/bike-core/src/activity_lifecycle&#46;rs

```
  242     uploads_dir: &str,
  243     tasks: &TaskQueue,
  244     user_id: i32,
  245 ) -> Result<DuplicateActivityCleanupSummary, AppError> {
! 246     let activities = activities::Model::for_user(db, user_id).await?;
  247     let cleanup_plan = plan_duplicate_activity_cleanup(activities);
  248 
  249     if cleanup_plan.duplicate_activity_ids.is_empty() {
  250         return Ok(DuplicateActivityCleanupSummary {
```


---


```
  253             retained_activity_count: 0,
  254         });
  255     }
  256 
! 257     let activities_by_id = activities::Model::list_owned_by_ids(
! 258         db,
! 259         user_id,
! 260         cleanup_plan.duplicate_activity_ids.iter().copied(),
! 261     )
! 262     .await?
! 263     .into_iter()
! 264     .map(|activity| (activity.id, activity))
! 265     .collect::<HashMap<_, _>>();
  266     let mut affected_segment_ids = Vec::new();
  267     let mut fitness_dirty_from_day: Option<chrono::NaiveDate> = None;
  268 
  269     for activity_id in &cleanup_plan.duplicate_activity_ids {
```


---


```
  537     user_id: i32,
  538     activity_import_ids: &[i32],
  539 ) -> Result<HashMap<i32, activities::Model>, AppError> {
  540     Ok(
! 541         activities::Model::linked_by_import_ids(db, user_id, activity_import_ids)
! 542             .await?
! 543             .into_iter()
! 544             .filter_map(|activity| {
! 545                 activity
! 546                     .activity_import_id
! 547                     .map(|import_id| (import_id, activity))
! 548             })
! 549             .collect(),
  550     )
  551 }
  552 
  553 async fn load_activities_by_id(
```


---


```
  554     db: &DatabaseConnection,
  555     user_id: i32,
  556     activity_ids: &[i32],
  557 ) -> Result<Vec<activities::Model>, AppError> {
! 558     activities::Model::list_owned_by_ids(db, user_id, activity_ids.iter().copied())
  559         .await
  560         .map_err(AppError::from)
  561 }
```


---


```
  731 async fn load_activity_for_reprocessing(
  732     db: &DatabaseConnection,
  733     activity_id: i32,
  734 ) -> Result<activities::Model, AppError> {
! 735     let activity = activities::Model::find_by_id(db, activity_id)
  736         .await?
  737         .ok_or_else(|| AppError::not_found(format!("Activity {activity_id} was not found")))?;
  738     if activity.source == "strava_sync" {
  739         return Err(AppError::bad_request(
```


---


```
  755 async fn load_activity_import_for_reprocessing(
  756     db: &DatabaseConnection,
  757     activity_import_id: i32,
  758 ) -> Result<activity_imports::Model, AppError> {
! 759     activity_imports::Entity::find_for_administration(db, activity_import_id)
  760         .await?
  761         .ok_or_else(|| {
  762             AppError::not_found(format!(
  763                 "Activity import {activity_import_id} was not found"
```


---



## bike-rs/bike-core/src/activity_training_analysis&#46;rs

```
  354         .collect::<Vec<_>>();
  355     segment_ids.sort_unstable();
  356     segment_ids.dedup();
  357 
! 358     let dh_segment_ids = segments::Model::downhill_by_ids(db, &segment_ids)
  359         .await?
  360         .into_iter()
  361         .map(|segment| segment.id)
  362         .collect::<HashSet<_>>();
```


---



## bike-rs/bike-core/src/analytics&#46;rs

```
  208 ) -> Result<(), sea_orm::DbErr>
  209 where
  210     C: ConnectionTrait,
  211 {
! 212     if let Some(model) = analytics_user_states::Model::for_user(db, user_id).await? {
  213         let mut active_model: analytics_user_states::ActiveModel = model.into();
  214         active_model.last_activity_change_at = Set(changed_at);
  215         active_model.update(db).await?;
  216     } else {
```


---



## bike-rs/bike-core/src/archive_import&#46;rs

```
  148     db: &DatabaseConnection,
  149     uploads_dir: &str,
  150     job_id: i32,
  151 ) -> Result<(), AppError> {
! 152     let job = activity_archive_import_jobs::Entity::find_for_batch(db, job_id)
  153         .await?
  154         .ok_or_else(|| AppError::internal(format!("Archive import job {job_id} was not found")))?;
  155 
  156     mark_user_activity_import_lock_stage(
```


---



## bike-rs/bike-core/src/auth/services/oauth&#46;rs

```
  215                 user = user
  216                     .link_oauth_identity(db, provider, provider_user.subject)
  217                     .await
  218                     .map_err(|e| {
! 219                         AuthError::internal_error(format!("Failed to update user: {}", e))
! 220                     })?;
  221 
  222                 span.record("auth.user_resolution", "linked_existing_email");
  223                 return Ok(user);
  224             }
```


---


```
  223                 return Ok(user);
  224             }
  225 
  226             // Create new OAuth-only user.
! 227             let user = users::Model::create_oauth_user(db, provider, provider_user)
  228                 .await
  229                 .map_err(|e| AuthError::internal_error(format!("Failed to create user: {}", e)))?;
  230 
  231             span.record("auth.user_resolution", "created_user");
```


---



## bike-rs/bike-core/src/background_jobs/batches/segments&#46;rs

```
  74         return Ok(false);
  75     }
  76     let ids: Vec<i32> = serde_json::from_value(batch.source["segment_ids"].clone())
  77         .map_err(|error| DbErr::Custom(error.to_string()))?;
! 78     let parent = super::background_tasks::Model::find_by_id(db, batch.id)
  79         .await?
  80         .ok_or_else(|| DbErr::RecordNotFound("Batch producer disappeared".into()))?;
  81     let origin = crate::background_jobs::pipeline::PipelineContext::from_payload(&parent.payload)
  82         .map_err(|error| DbErr::Custom(error.to_string()))?
```


---


```
   98                 .await?;
   99             }
  100             let mut source = batch.source.clone();
  101             source["caches_scheduled"] = true.into();
! 102             worker_batches::Model {
! 103                 source,
! 104                 ..batch.clone()
! 105             }
! 106             .store_source(db)
! 107             .await?;
  108             Ok(!ids.is_empty())
  109         }),
  110     )
  111     .await
```


---



## bike-rs/bike-core/src/background_jobs/entities/background_tasks/admin&#46;rs

```
  35             query = Model::with_correlation(query, id);
  36         }
  37 
  38         if let Some(ref t) = params.task_type {
! 39             query = query.filter(Column::TaskType.eq(t.clone()));
  40         }
  41         if let Some(ref s) = params.status {
! 42             query = query.filter(Column::Status.eq(s.clone()));
  43         }
  44         if let Some(ref e) = params.error {
! 45             query = query.filter(Column::Error.contains(e.as_str()));
  46         }
  47         if let Some(ref d) = params.from_date {
! 48             if let Ok(parsed) = DateTime::parse_from_rfc3339(d) {
! 49                 query = query.filter(Column::CreatedAt.gte(parsed.with_timezone(&Utc)));
! 50             }
  51         }
  52         if let Some(ref d) = params.to_date {
! 53             if let Ok(parsed) = DateTime::parse_from_rfc3339(d) {
! 54                 query = query.filter(Column::CreatedAt.lte(parsed.with_timezone(&Utc)));
! 55             }
  56         }
  57 
  58         let paginator = query.paginate(db, per_page);
  59         let total = paginator.num_items().await? as i64;
```


---



## bike-rs/bike-core/src/background_jobs/entities/background_tasks/diagnostics&#46;rs

```
  74             .into_tuple::<i32>()
  75             .one(db)
  76             .await?;
  77         if active.is_some() {
! 78             return Ok(None);
  79         }
  80         let end = Entity::find()
  81             .filter(Column::Id.in_subquery(ids))
  82             .select_only()
```


---



## bike-rs/bike-core/src/background_jobs/entities/pipeline_subjects&#46;rs

```
  104         let query = Entity::find()
  105             .filter(Column::Kind.eq("activity"))
  106             .filter(Column::SubjectId.eq(activity_id));
  107         let query = if let Some(after) = after {
! 108             query.filter(Column::RunId.gt(after))
  109         } else {
  110             query
  111         };
  112         let run_ids = query
```


---



## bike-rs/bike-core/src/background_jobs/entities/task_attempts/statistics&#46;rs

```
  16         signal: &str,
  17     ) -> Result<Vec<ProcessorDistribution>, DbErr> {
  18         if db.get_database_backend() != DbBackend::Postgres {
  19             return Ok(Vec::new());
! 20         }
! 21         let (expression, table, time, condition) = match signal {
! 22             "logical" => (
! 23                 "EXTRACT(EPOCH FROM (t.completed_at-t.created_at))",
! 24                 "background_tasks t",
! 25                 "t.completed_at",
! 26                 "t.status='completed'",
! 27             ),
! 28             "wait" => (
! 29                 "EXTRACT(EPOCH FROM (a.started_at-a.eligible_at))",
! 30                 "background_task_attempts a JOIN background_tasks t ON t.id=a.task_id",
! 31                 "a.finished_at",
! 32                 "a.eligible_at IS NOT NULL AND a.outcome=$2",
! 33             ),
! 34             _ => (
! 35                 "EXTRACT(EPOCH FROM (a.finished_at-a.started_at))",
! 36                 "background_task_attempts a JOIN background_tasks t ON t.id=a.task_id",
! 37                 "a.finished_at",
! 38                 "a.outcome=$2",
! 39             ),
  40         };
! 41         let sql = format!("SELECT task_type, COUNT(*)::bigint samples, percentile_cont(0.5) WITHIN GROUP (ORDER BY duration) p50_seconds, percentile_cont(0.9) WITHIN GROUP (ORDER BY duration) p90_seconds FROM (SELECT t.task_type, {expression} duration FROM {table} WHERE {time}>=$1 AND {time}<=CURRENT_TIMESTAMP AND {condition}) durations WHERE duration>=0 GROUP BY task_type");
! 42         let mut values = vec![(Utc::now() - chrono::Duration::hours(hours)).into()];
! 43         if signal != "logical" {
! 44             values.push(outcome.into());
! 45         }
! 46         let rows = ProcessorDistribution::find_by_statement(Statement::from_sql_and_values(
! 47             DbBackend::Postgres,
! 48             sql,
! 49             values,
! 50         ))
! 51         .all(db)
! 52         .await?;
! 53         Ok(rows)
  54     }
  55 }
```


---



## bike-rs/bike-core/src/entities/activities&#46;rs

```
  778 mod training;
  779 pub use training::ActivitySummaryRow;
  780 
  781 impl Model {
! 782     pub async fn for_user(db: &impl ConnectionTrait, user_id: i32) -> Result<Vec<Self>, DbErr> {
! 783         Entity::find()
! 784             .filter(Column::UserId.eq(user_id))
! 785             .all(db)
! 786             .await
! 787     }
  788     pub async fn owned_import_activity(
  789         db: &impl ConnectionTrait,
  790         user_id: i32,
  791         import_id: i32,
```


---


```
  795             .filter(Column::ActivityImportId.eq(Some(import_id)))
  796             .one(db)
  797             .await
  798     }
! 799     pub async fn linked_by_import_ids(
! 800         db: &impl ConnectionTrait,
! 801         user_id: i32,
! 802         import_ids: &[i32],
! 803     ) -> Result<Vec<Self>, DbErr> {
! 804         Entity::find()
! 805             .filter(Column::UserId.eq(user_id))
! 806             .filter(Column::ActivityImportId.is_in(import_ids.iter().copied()))
! 807             .all(db)
! 808             .await
! 809     }
  810     pub async fn at_start_for_sports(
  811         db: &impl ConnectionTrait,
  812         user_id: i32,
  813         started_at: DateTime<Utc>,
```


---


```
  832             .filter(Column::SourceCorrelationId.eq(correlation_id))
  833             .one(db)
  834             .await
  835     }
! 836     pub async fn owner_ids(db: &impl ConnectionTrait) -> Result<Vec<i32>, DbErr> {
! 837         let mut ids = Entity::find()
! 838             .select_only()
! 839             .column(Column::UserId)
! 840             .filter(Column::UserId.is_not_null())
! 841             .distinct()
! 842             .into_tuple::<i32>()
! 843             .all(db)
! 844             .await?;
! 845         ids.sort_unstable();
! 846         ids.dedup();
! 847         Ok(ids)
! 848     }
! 849     pub async fn for_import_inspection(
! 850         db: &impl ConnectionTrait,
! 851         import: &super::activity_imports::Model,
! 852     ) -> Result<Option<Self>, DbErr> {
! 853         if let Some(id) = import.activity_id {
! 854             return Self::find_by_id(db, id).await;
! 855         }
! 856         Entity::find()
! 857             .filter(Column::ActivityImportId.eq(Some(import.id)))
! 858             .one(db)
! 859             .await
! 860     }
  861 }
  862 
  863 #[cfg(test)]
  864 mod admin_list_tests {
```


---



## bike-rs/bike-core/src/entities/activity_import_artifacts&#46;rs

```
  191             _ => match self.format.as_str() {
  192                 "fit" => 490,
  193                 "tcx" => 390,
  194                 "gpx" => 290,
! 195                 _ => 0,
  196             },
  197         }
  198     }
  199 }
```


---



## bike-rs/bike-core/src/entities/activity_imports&#46;rs

```
  350             .order_by_asc(Column::CreatedAt)
  351             .all(db)
  352             .await
  353     }
! 354     pub async fn stale_manual_owner_ids(
! 355         db: &impl ConnectionTrait,
! 356         stale_before: DateTime<Utc>,
! 357     ) -> Result<Vec<i32>, DbErr> {
! 358         Self::find()
! 359             .select_only()
! 360             .column(Column::UserId)
! 361             .distinct()
! 362             .filter(Column::Source.eq("manual_upload"))
! 363             .filter(Column::Status.eq("processing"))
! 364             .filter(Column::LastProcessingEventAt.lte(stale_before))
! 365             .into_tuple()
! 366             .all(db)
! 367             .await
! 368     }
  369     pub async fn processing_owner_ids(
  370         db: &impl ConnectionTrait,
  371         source: Option<&str>,
  372     ) -> Result<Vec<i32>, DbErr> {
```


---


```
  376             .distinct()
  377             .filter(Column::Status.eq("processing"))
  378             .filter(Column::Source.ne(STRAVA_SOURCE));
  379         if let Some(source) = source {
! 380             query = query.filter(Column::Source.eq(source));
  381         }
  382         query.into_tuple().all(db).await
  383     }
  384     pub async fn archive_statuses(
```


---



## bike-rs/bike-core/src/entities/activity_training_analyses&#46;rs

```
   80             .filter(Column::ActivityId.is_in(activity_ids.iter().copied()))
   81             .all(db)
   82             .await
   83     }
!  84     pub async fn for_user(db: &impl ConnectionTrait, user_id: i32) -> Result<Vec<Self>, DbErr> {
   85         use sea_orm::QueryFilter;
!  86         Entity::find()
!  87             .filter(Column::UserId.eq(user_id))
!  88             .all(db)
!  89             .await
!  90     }
!  91     pub async fn for_owned_activity_ids(
!  92         db: &impl ConnectionTrait,
!  93         user_id: i32,
!  94         activity_ids: &[i32],
!  95     ) -> Result<Vec<Self>, DbErr> {
   96         use sea_orm::QueryFilter;
!  97         Entity::find()
!  98             .filter(Column::UserId.eq(user_id))
!  99             .filter(Column::ActivityId.is_in(activity_ids.iter().copied()))
! 100             .all(db)
! 101             .await
! 102     }
  103 }
  104 
  105 impl Model {
  106     pub async fn clear_activity(db: &impl ConnectionTrait, id: i32) -> Result<(), DbErr> {
```


---



## bike-rs/bike-core/src/entities/fitness_freshness_daily&#46;rs

```
   95             query = query.filter(Column::Day.gte(start));
   96         }
   97         query.all(db).await
   98     }
!  99     pub async fn latest_at_or_before(
! 100         db: &impl ConnectionTrait,
! 101         user_id: i32,
! 102         end: NaiveDate,
! 103     ) -> Result<Option<Self>, DbErr> {
! 104         Entity::find()
! 105             .filter(Column::UserId.eq(user_id))
! 106             .filter(Column::Day.lte(end))
! 107             .order_by_desc(Column::Day)
! 108             .one(db)
! 109             .await
! 110     }
  111 }
```


---



## bike-rs/bike-core/src/entities/heatmap_projections/lifecycle&#46;rs

```
  57                     activity_id,
  58                     &format!("{}:{PROJECTION_VERSION}", projection.generation),
  59                 )
  60                 .await?;
! 61             }
  62             chunks_entity::Model::clear_activity(db, activity_id).await?;
  63             user_states::Model::bump_revision(db, user_id).await?;
! 64         }
  65         Ok(())
  66     }
  67 
  68     pub async fn backfill_page(
```


---


```
   93 
   94     /// Queue and lease rows in the same transaction. Worker crashes and enqueue
   95     /// failures cannot strand durable dirty state. Projection and heartbeat leases
   96     /// share WORKER_RECOVERY_TIMEOUT_SECONDS.
!  97     pub async fn enqueue_pending(db: &DatabaseConnection) -> Result<u64, DbErr> {
!  98         let context =
!  99             crate::background_jobs::pipeline::PipelineContext::current().unwrap_or_else(|| {
! 100                 crate::background_jobs::pipeline::PipelineContext::received(
  101                     "heatmap_reconciliation",
! 102                     None,
  103                 )
! 104             });
! 105         crate::background_jobs::pipeline::PipelineContext::scope(
! 106             Some(context),
! 107             Self::lease_and_enqueue(db),
! 108         )
! 109         .await
! 110     }
  111 
! 112     async fn lease_and_enqueue(db: &DatabaseConnection) -> Result<u64, DbErr> {
! 113         let txn = db.begin().await?;
  114         // Native locking is necessary for concurrent outbox consumers. Submission
  115         // then uses the shared queue API in this same transaction, including lineage.
! 116         let leased = PendingProjection::find_by_statement(Statement::from_sql_and_values(DbBackend::Postgres, r#"
! 117             WITH candidates AS (
! 118                 SELECT p.activity_id FROM heatmap_projections p
! 119                 WHERE p.status='pending' AND p.projection_version=$1 AND (p.queued_at IS NULL OR p.queued_at < now() - make_interval(secs => $2::double precision))
! 120                 AND EXISTS (SELECT 1 FROM activities a WHERE a.id=p.activity_id AND a.source <> 'strava_sync')
! 121                 AND NOT EXISTS (SELECT 1 FROM background_tasks t WHERE t.task_type=$3
! 122                     AND t.status IN ('pending','processing') AND (t.status='processing' OR t.attempts<t.max_attempts)
! 123                     AND (t.payload->'data'->'activities') @> jsonb_build_array(jsonb_build_object('activity_id',p.activity_id,'generation',p.generation)))
! 124                 ORDER BY p.activity_id LIMIT 16 FOR UPDATE SKIP LOCKED
! 125             )
! 126             UPDATE heatmap_projections p SET queued_at=now() FROM candidates c WHERE p.activity_id=c.activity_id
! 127             RETURNING p.activity_id, p.generation
! 128         "#,[PROJECTION_VERSION.into(), crate::config::Config::get().worker_recovery_timeout_seconds.into(), crate::jobs::JobType::PrepareHeatmap.as_str().into()])).all(&txn).await?;
! 129         if leased.is_empty() {
! 130             return Ok(0);
! 131         }
! 132         let activities: Vec<_> = leased.iter().map(|pending|serde_json::json!({"activity_id":pending.activity_id,"generation":pending.generation})).collect();
! 133         let task = crate::background_jobs::background_tasks::Model::enqueue(
! 134             &txn,
! 135             crate::jobs::JobType::PrepareHeatmap.as_str().into(),
! 136             serde_json::json!({"type":"PrepareHeatmap","data":{"activities":activities}}),
! 137             None,
! 138             3,
! 139         )
! 140         .await?;
! 141         for pending in leased {
! 142             let revision = Self::revision(pending.generation);
! 143             crate::background_jobs::entities::pipeline_outputs::Model::advance(
! 144                 &txn,
! 145                 "heatmap",
! 146                 pending.activity_id,
! 147                 &revision,
! 148             )
! 149             .await?;
! 150             crate::background_jobs::entities::pipeline_outputs::Model::attach_waiting_tasks(
! 151                 &txn,
! 152                 "heatmap",
! 153                 pending.activity_id,
! 154                 &revision,
! 155                 task.id,
! 156             )
! 157             .await?;
  158         }
! 159         txn.commit().await?;
! 160         Ok(1)
! 161     }
  162 
  163     pub fn revision(generation: i64) -> String {
  164         format!("{generation}:{PROJECTION_VERSION}")
  165     }
```


---



## bike-rs/bike-core/src/entities/segments&#46;rs

```
   95             .exec(&txn)
   96             .await?;
   97         if !activity_ids.is_empty() {
   98             crate::analytics::rebuild_activity_analytics_cache(&txn, &activity_ids).await?;
!  99         }
  100         txn.commit().await
  101     }
  102 
  103     pub async fn metrics_counts(
```


---



## bike-rs/bike-core/src/entities/segments/access&#46;rs

```
  32         distance_meters: Option<f64>,
  33         route_points: &[ActivityRoutePoint],
  34     ) -> Result<Option<Model>, DbErr> {
  35         let Some(target_key) = segment_dedupe_key(distance_meters, route_points) else {
! 36             return Ok(None);
  37         };
  38 
  39         let mut candidate_query = Entity::find()
  40             .select_only()
```


---


```
  49             candidate_query = candidate_query
  50                 .filter(Column::DistanceMeters.gte(minimum_distance))
  51                 .filter(Column::DistanceMeters.lt(maximum_distance));
  52         } else {
! 53             candidate_query = candidate_query.filter(Column::DistanceMeters.is_null());
! 54         }
  55 
  56         let candidates = candidate_query
  57             .into_model::<SegmentDedupeCandidateRow>()
  58             .all(db)
```


---


```
  64             if segment_dedupe_key(segment.distance_meters, &candidate_route_points).as_deref()
  65                 == Some(target_key.as_str())
  66             {
  67                 return Self::find_owned(db, segment.id, user_id).await;
! 68             }
  69         }
  70 
  71         Ok(None)
  72     }
```


---



## bike-rs/bike-core/src/entities/strava_connections&#46;rs

```
  169             .filter(Column::UserId.eq(user_id))
  170             .one(db)
  171             .await
  172     }
! 173     pub async fn remove(&self, db: &impl ConnectionTrait) -> Result<(), DbErr> {
! 174         Entity::delete_many()
! 175             .filter(Column::Id.eq(self.id))
! 176             .exec(db)
! 177             .await?;
! 178         Ok(())
! 179     }
  180 }
```


---



## bike-rs/bike-core/src/segment_regeneration&#46;rs

```
  59 pub async fn regenerate_segment_efforts(
  60     db: &DatabaseConnection,
  61     segment_id: i32,
  62 ) -> Result<(), SegmentRegenerationError> {
! 63     let segment = segments::Model::find_by_id(db, segment_id)
  64         .await?
  65         .ok_or_else(|| {
  66             SegmentRegenerationError::internal(format!("segment {segment_id} was not found"))
  67         })?;
```


---



## bike-rs/bike-core/src/segment_support&#46;rs

```
  197     C: ConnectionTrait,
  198 {
  199     if segment_route_points.len() < 2 {
  200         let affected_activity_ids = load_activity_ids_for_segment(db, segment_id).await?;
! 201         segment_efforts::Entity::delete_for_segment(db, segment_id).await?;
  202         return Ok(affected_activity_ids);
  203     }
  204 
  205     let mut affected_activity_ids = load_activity_ids_for_segment(db, segment_id).await?;
```


---


```
  224             }
  225         }
  226     }
  227 
! 228     segment_efforts::Entity::delete_for_segment(db, segment_id).await?;
  229 
  230     for (user_id, activity_id, matches) in replacements {
  231         insert_matches(db, user_id, segment_id, activity_id, &matches).await?;
  232     }
```


---


```
  294 ) -> Result<(), SegmentSupportError>
  295 where
  296     C: ConnectionTrait,
  297 {
! 298     segment_efforts::Entity::store_matches(db, user_id, segment_id, activity_id, matches).await?;
  299     Ok(())
  300 }
  301 
  302 fn match_segment_efforts(
```


---



## bike-rs/bike-core/src/services/reports&#46;rs

```
  876         return get_aggregate_training_reports(context, query).await;
  877     }
  878 
  879     let envelope = TrainingReportEnvelope::from_context(&context);
! 880     let activity_models = activities::Model::report_range(
! 881         &context.db,
! 882         context.user_id,
! 883         context.range_start,
! 884         context.range_end,
! 885         query.min_duration_seconds,
! 886         query.min_distance_meters,
  887     )
  888     .await?;
  889 
  890     match report_id {
```


---


```
  963     let selected_ids = parse_activity_ids(query.activity_ids.as_deref())?;
  964     let selected_models = if selected_ids.is_empty() {
  965         Vec::new()
  966     } else {
! 967         activities::Model::report_selection(
! 968             &context.db,
! 969             context.user_id,
! 970             &selected_ids,
! 971             query.min_duration_seconds,
! 972             query.min_distance_meters,
  973         )
  974         .await?
  975     };
```


---


```
  982 async fn reassessment_response(
  983     context: &TrainingReportBuildContext,
  984     query: &TrainingReportsQuery,
  985 ) -> Result<TrainingReportsResponse, BikeCoreError> {
! 986     let preferences = user_preferences::Model::for_user(&context.db, context.user_id).await?;
  987     let range = reassessment_analysis_range(context, preferences.as_ref())?;
  988     let activities = load_reassessment_activities(context, query, &range).await?;
  989     let fitness_rows = load_reassessment_fitness_rows(context.user_id, &context.db, &range).await?;
  990     let mut response = TrainingReportEnvelope {
```


---


```
  1067     context: &TrainingReportBuildContext,
  1068     query: &TrainingReportsQuery,
  1069     range: &ReassessmentAnalysisRange,
  1070 ) -> Result<Vec<activities::Model>, BikeCoreError> {
! 1071     activities::Model::report_range(
! 1072         &context.db,
! 1073         context.user_id,
! 1074         range.analysis_start,
! 1075         range.reassessment_end,
! 1076         query.min_duration_seconds,
! 1077         query.min_distance_meters,
  1078     )
  1079     .await
  1080     .map_err(BikeCoreError::from)
  1081 }
```


---


```
  1084     user_id: i32,
  1085     db: &DatabaseConnection,
  1086     range: &ReassessmentAnalysisRange,
  1087 ) -> Result<Vec<fitness_freshness_daily::Model>, BikeCoreError> {
! 1088     fitness_freshness_daily::Model::between_days(
! 1089         db,
! 1090         user_id,
! 1091         Some(range.analysis_start.date_naive()),
! 1092         range.reassessment_end.date_naive(),
! 1093     )
! 1094     .await
! 1095     .map_err(BikeCoreError::from)
  1096 }
  1097 
  1098 async fn build_aggregate_training_reports(
  1099     context: TrainingReportBuildContext,
```


---


```
  1123 async fn load_report_activities(
  1124     context: &TrainingReportBuildContext,
  1125     query: &TrainingReportsQuery,
  1126 ) -> Result<Vec<activities::Model>, BikeCoreError> {
! 1127     activities::Model::report_range(
! 1128         &context.db,
! 1129         context.user_id,
! 1130         context.range_start,
! 1131         context.range_end,
! 1132         query.min_duration_seconds,
! 1133         query.min_distance_meters,
  1134     )
  1135     .await
  1136     .map_err(BikeCoreError::from)
  1137 }
```


---


```
  1147     if activity_ids.is_empty() {
  1148         return Ok(HashMap::new());
  1149     }
  1150 
! 1151     Ok(activity_training_analyses::Model::for_owned_activity_ids(
! 1152         &context.db,
! 1153         context.user_id,
! 1154         &activity_ids,
! 1155     )
! 1156     .await?
! 1157     .into_iter()
! 1158     .map(|analysis| (analysis.activity_id, analysis))
! 1159     .collect())
  1160 }
  1161 
  1162 fn aggregate_report_buckets(
  1163     context: &TrainingReportBuildContext,
```


---



## bike-rs/bike-core/src/services/training_goals&#46;rs

```
  473     ) -> Result<XcGoalProgressResponse, BikeCoreError> {
  474         let goal = load_xc_event_goal(db, user_id).await?;
  475         let freshness =
  476             load_latest_fitness_freshness_snapshot(db, user_id, now.date_naive()).await?;
! 477         let analysis_models = activity_training_analyses::Model::for_user(db, user_id).await?;
  478         let activity_ids = analysis_models
  479             .iter()
  480             .map(|analysis| analysis.activity_id)
  481             .collect::<Vec<_>>();
! 482         let activity_by_id =
! 483             activities::Model::training_summaries(db, user_id, &activity_ids).await?;
  484 
  485         let mut rides = xc_rides_from_analysis_models(analysis_models, &activity_by_id);
! 486         let race_activities = activities::Model::race_summaries(db, user_id).await?;
  487 
  488         append_missing_race_rides(&mut rides, race_activities);
  489         sort_xc_rides_recent_first(&mut rides);
```


---


```
  497         now: DateTime<Utc>,
  498     ) -> Result<DhGoalProgressResponse, BikeCoreError> {
  499         let freshness =
  500             load_latest_fitness_freshness_snapshot(db, user_id, now.date_naive()).await?;
! 501         let segment_models = segments::Model::downhill_for_user(db, user_id).await?;
  502 
  503         let segment_ids = segment_models
  504             .iter()
  505             .map(|segment| segment.id)
```


---


```
  506             .collect::<Vec<_>>();
  507         let effort_models = if segment_ids.is_empty() {
  508             Vec::new()
  509         } else {
! 510             segment_efforts::Model::for_owned_segments(db, user_id, &segment_ids).await?
  511         };
  512         let activity_ids = effort_models
  513             .iter()
  514             .map(|effort| effort.activity_id)
```


---


```
  512         let activity_ids = effort_models
  513             .iter()
  514             .map(|effort| effort.activity_id)
  515             .collect::<Vec<_>>();
! 516         let activity_by_id =
! 517             activities::Model::training_summaries(db, user_id, &activity_ids).await?;
  518         let efforts = dh_effort_sources_from_models(effort_models, &activity_by_id);
  519 
  520         Ok(build_dh_goal_progress_response(
  521             segment_models,
```


---


```
  666 async fn load_xc_event_goal(
  667     db: &sea_orm::DatabaseConnection,
  668     user_id: i32,
  669 ) -> Result<Option<XcEventGoal>, BikeCoreError> {
! 670     let preferences = user_preferences::Model::for_user(db, user_id).await?;
  671     let Some(preferences) = preferences else {
  672         return Ok(None);
  673     };
```


---


```
  707     db: &sea_orm::DatabaseConnection,
  708     user_id: i32,
  709     end_date: NaiveDate,
  710 ) -> Result<Option<FitnessFreshnessSnapshot>, BikeCoreError> {
! 711     let freshness_state = analytics_user_states::Model::for_user(db, user_id).await?;
  712 
  713     if freshness_state
  714         .as_ref()
  715         .and_then(|state| state.fitness_dirty_from_day)
```


---


```
  717     {
  718         return Ok(None);
  719     }
  720 
! 721     let latest_row =
! 722         fitness_freshness_daily::Model::latest_at_or_before(db, user_id, end_date).await?;
  723 
  724     Ok(latest_row.map(|row| {
  725         decay_fitness_freshness_snapshot_to_day(
  726             FitnessFreshnessSnapshot {
```


---



## bike-rs/bike-core/src/strava/connection&#46;rs

```
  266 ) -> Result<(), AppError> {
  267     let Some(connection) = load_connection(db, user_id).await? else {
  268         return Ok(());
  269     };
! 270     connection.remove(db).await?;
  271     Ok(())
  272 }
  273 
  274 fn ensure_strava_configured(config: &Config) -> Result<(), AppError> {
```


---


```
  295     db: &DatabaseConnection,
  296     user_id: i32,
  297     token_response: StravaAuthorizationTokenResponse,
  298 ) -> Result<strava_connections::Model, AppError> {
! 299     if let Some(existing_for_athlete) =
! 300         strava_connections::Model::for_athlete(db, token_response.athlete.id).await?
  301     {
  302         if existing_for_athlete.user_id != user_id {
  303             return Err(AppError::bad_request(
  304                 "That Strava athlete is already connected to another Bike account",
```


---



## bike-rs/bike-core/src/strava/native/persistence&#46;rs

```
  309         if strava_activity_states::Model::supersedes(&txn, &change.event, false).await? {
  310             return Ok(change);
  311         }
  312         if let Some(id) = change.bike_activity_id {
! 313             if let Some(activity) = activities::Model::find_by_id(&txn, id).await? {
  314                 let derived = crate::activity_data::deserialize_derived_activity_data(
  315                     activity.derived_data_json.as_ref(),
  316                 );
  317                 change.segment_ids = crate::activity_lifecycle::refresh_activity_derived_state_without_cache_rebuilds(&txn, change.event.bike_user_id, id, &derived.route_points).await?;
```


---


```
  362         &self,
  363         change: ActivityChange,
  364     ) -> Result<ActivityChange, WorkflowError> {
  365         if let Some(id) = change.bike_activity_id {
! 366             if let Some(projection) =
! 367                 crate::entities::heatmap_projections::Model::for_activity(&self.db, id).await?
  368             {
  369                 crate::heatmaps::preparation::prepare_activity(
  370                     &self.db,
  371                     crate::heatmaps::projection::PendingProjection {
```


---


