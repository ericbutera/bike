# Diff Coverage
## Diff: e401699acdc0b5280a64d13d689fc3efd11cdd43...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-core/src/activity_import_lifecycle&#46;rs (0.0%): Missing lines 55
- bike-rs/bike-core/src/activity_import_pipeline&#46;rs (100%)
- bike-rs/bike-core/src/activity_reprocessing&#46;rs (90.0%): Missing lines 92,96-98
- bike-rs/bike-core/src/activity_source_recovery&#46;rs (0.0%): Missing lines 477
- bike-rs/bike-core/src/analytics&#46;rs (92.9%): Missing lines 361
- bike-rs/bike-core/src/archive_import/native&#46;rs (77.7%): Missing lines 15-16,88-110,113-136
- bike-rs/bike-core/src/bike_tasks&#46;rs (100%)
- bike-rs/bike-core/src/entities/activities/parsed&#46;rs (100%)
- bike-rs/bike-core/src/entities/activities/reprocessing&#46;rs (95.9%): Missing lines 91,93-94
- bike-rs/bike-core/src/entities/activity_event_history&#46;rs (100%)
- bike-rs/bike-core/src/entities/activity_imports&#46;rs (93.8%): Missing lines 78
- bike-rs/bike-core/src/entities/heatmap_projections/lifecycle&#46;rs (95.8%): Missing lines 28
- bike-rs/bike-core/src/entities/segment_user_summaries&#46;rs (100%)
- bike-rs/bike-core/src/jobs/adapter&#46;rs (94.0%): Missing lines 61-62,85,112,147
- bike-rs/bike-core/src/jobs/native&#46;rs (53.8%): Missing lines 11-22
- bike-rs/bike-core/src/object_store&#46;rs (100%)
- bike-rs/bike-core/src/segment_regeneration&#46;rs (95.7%): Missing lines 79
- bike-rs/bike-core/src/strava/events/intake&#46;rs (100%)
- bike-rs/bike-core/src/strava/events/intake/strava_webhook_unit&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/persistence&#46;rs (63.6%): Missing lines 56,114-115,255-263
- bike-rs/bike-core/src/strava/native/persistence/strava_native_unit_history&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/persistence/strava_native_unit_retry&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/strava_native_unit/upsert&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/strava_native_unit/workflow_spans&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/types&#46;rs (100%)
- bike-rs/bike-core/src/test_support/analytics&#46;rs (100%)
- bike-rs/bike-core/src/test_support/tasks&#46;rs (100%)
- bike-rs/bike-core/src/test_support/temporal_client&#46;rs (98.5%): Missing lines 59
- bike-rs/bike-core/src/workflow_error&#46;rs (88.9%): Missing lines 129
- bike-rs/bike-core/src/xc_goal_backfill&#46;rs (100%)

## Summary

- **Total**: 859 lines
- **Missing**: 93 lines
- **Coverage**: 89.17%



## bike-rs/bike-core/src/activity_import_lifecycle&#46;rs

```
  51     import: &activity_imports::Model,
  52     stage: &str,
  53     error_message: &str,
  54 ) -> Result<(), ActivityImportLifecycleError> {
! 55     import.clone().mark_failed(db, stage, error_message).await?;
  56     Ok(())
  57 }
  58 pub async fn record_activity_processing_event(
  59     db: &impl sea_orm::ConnectionTrait,
```


---



## bike-rs/bike-core/src/activity_reprocessing&#46;rs

```
   88             activities::Model::find_owned(db, activity_id, source.user_id)
   89                 .await?
   90                 .ok_or_else(|| WorkflowError::not_found("Activity not found"))?,
   91         ),
!  92         ImportTarget::NewActivity => None,
   93     };
   94     let import = match &current {
   95         Some(activity) => owned_import(db, activity).await?,
!  96         None => activity_imports::Entity::find_owned(db, source.user_id, source.activity_import_id)
!  97             .await?
!  98             .ok_or_else(|| WorkflowError::not_found("Activity import not found"))?,
   99     };
  100     import.require_retained_file_processing()?;
  101     let artifact = load_best_activity_parsing_artifact(db, &import).await?;
  102     if import.id != source.activity_import_id || artifact.object_reference()? != source.object {
```


---



## bike-rs/bike-core/src/activity_source_recovery&#46;rs

```
  473     let Some(source) = plan.source.take() else {
  474         return Ok(false);
  475     };
  476     if plan.reprocessing_required {
! 477         plan.import.require_retained_file_processing()?;
  478     }
  479     let mut artifact = source.artifact;
  480     let mut copied_path = None;
  481     if artifact.activity_import_id != plan.import_id {
```


---



## bike-rs/bike-core/src/analytics&#46;rs

```
  357     let current = segments::Model::lock_for_analytics(&txn, id).await?;
  358     if current.is_none_or(|current| {
  359         current.last_activity_change_at != segment.last_activity_change_at || current.updated_at != segment.updated_at
  360     }) {
! 361         return Ok(());
  362     }
  363     persist_segment_analytics_rebuild(&txn, &[id], rebuild).await?;
  364     txn.commit().await?;
  365     // Different segments share activity summaries. Release segment/rank
```


---



## bike-rs/bike-core/src/archive_import/native&#46;rs

```
  11         tokio::fs::create_dir_all(archive_path.parent().expect("archive directory")).await?;
  12         if Path::new(&job.archive_url).is_absolute() {
  13             tokio::fs::copy(&job.archive_url, &archive_path).await?;
  14         } else {
! 15             let downloaded = download_archive_from_url(uploads_dir, &job.archive_url).await?;
! 16             tokio::fs::rename(downloaded.archive_path, &archive_path).await?;
  17         }
  18     }
  19     mark_activity_archive_import_job_running(db, &job).await?;
  20     Ok(ArchiveCursor {
```


---


```
   84         .map_err(|error| AppError::bad_request(format!("{}: {}", entry.entry_name, error.message)))?;
   85     let bytes = maybe_decode_archive_entry(&entry.activity_entry, bytes).map_err(AppError::bad_request)?;
   86     if let Some(import) = crate::activity_import_pipeline::find_stored_activity_import(db, job.user_id, &bytes).await? {
   87         return Ok(import);
!  88     }
!  89     store_activity_upload_import_with_artifacts(
!  90         db,
!  91         StoreActivityUploadImportRequest {
!  92             uploads_dir,
!  93             user_storage_key: &job.user_storage_key,
!  94             user_id: job.user_id,
!  95             upload: ActivityUploadPayload {
!  96                 original_filename: entry.activity_entry.original_filename.clone(),
!  97                 format: entry.activity_entry.format.clone(),
!  98                 mime_type: None,
!  99                 source_correlation_id: None,
! 100                 bytes,
! 101             },
! 102             primary_artifact_kind: "original",
! 103             primary_source_quality: crate::activity_import_pipeline::original_source_quality_for_format(
! 104                 &entry.activity_entry.format,
! 105             ),
! 106             additional_artifacts: Vec::new(),
! 107             source: "archive_url_import",
! 108         },
! 109     )
! 110     .await
  111 }
  112 
! 113 pub async fn finish(db: &DatabaseConnection, job_id: i32) -> Result<(), AppError> {
! 114     let job = activity_archive_import_jobs::Entity::find_for_batch(db, job_id)
! 115         .await?
! 116         .ok_or_else(|| AppError::not_found("Archive job not found"))?;
! 117     let statuses = crate::entities::activity_imports::Entity::archive_statuses(db, job_id).await?;
! 118     let imported = statuses.iter().filter(|status| status.as_str() == "processed").count() as i32;
! 119     let duplicate = statuses.iter().filter(|status| status.as_str() == "duplicate").count() as i32;
! 120     mark_activity_archive_import_job_finished(
! 121         db,
! 122         &job,
! 123         &ActivityArchiveImportResponse {
! 124             source: job.archive_url.clone(),
! 125             total_entries: job.total_entries,
! 126             supported_entry_count: job.supported_entry_count,
! 127             skipped_unsupported_count: job.skipped_unsupported_count,
! 128             imported_count: imported,
! 129             duplicate_count: duplicate,
! 130             failed_count: 0,
! 131             error_samples: Vec::new(),
! 132         },
! 133     )
! 134     .await?;
! 135     Ok(())
! 136 }
  137 
  138 #[cfg(test)]
  139 mod tests {
  140     use super::*;
```


---



## bike-rs/bike-core/src/entities/activities/reprocessing&#46;rs

```
  87         let mut derived = crate::activity_data::deserialize_derived_activity_data(self.derived_data_json.as_ref());
  88         let previous = derived.recording.clone();
  89         derived.recording.merge(recording.clone());
  90         if derived.recording != previous
! 91             && !Self::store_recording(db, self.id, self.user_id, self.updated_at, &derived).await?
  92         {
! 93             return Err(WorkflowError::internal(
! 94                 "Activity changed while merging recording evidence; retry import",
  95             ));
  96         }
  97         Ok(())
  98     }
```


---



## bike-rs/bike-core/src/entities/activity_imports&#46;rs

```
  74 impl Entity {
  75     pub async fn recent_manual_uploads(db: &impl ConnectionTrait, user_id: i32) -> Result<Vec<Model>, DbErr> {
  76         Self::find()
  77             .filter(Column::UserId.eq(user_id))
! 78             .filter(Column::Source.eq("manual_upload"))
  79             .order_by_desc(Column::CreatedAt)
  80             .limit(25)
  81             .all(db)
  82             .await
```


---



## bike-rs/bike-core/src/entities/heatmap_projections/lifecycle&#46;rs

```
  24         db: &impl ConnectionTrait,
  25         after_id: i32,
  26     ) -> Result<Vec<crate::bike_tasks::ActivityTask>, DbErr> {
  27         if !Self::enabled(db).await? {
! 28             return Ok(Vec::new());
  29         }
  30         let rows = projections::Entity::find()
  31             .filter(projections::Column::ActivityId.gt(after_id))
  32             .filter(projections::Column::Status.is_in(["pending", "failed"]))
```


---



## bike-rs/bike-core/src/jobs/adapter&#46;rs

```
  57         .map(|_| ())
  58         .map_err(super::TaskError::Storage)
  59     }
  60 
! 61     pub async fn queue_fitness_freshness(&self, user_id: i32) -> Result<(), super::TaskError> {
! 62         self.rebuild_fitness_freshness(user_id).await
  63     }
  64 
  65     pub async fn rebuild_segment_analytics(&self, mut segment_ids: Vec<i32>) -> Result<(), super::TaskError> {
  66         segment_ids.retain(|id| *id > 0);
```


---


```
  81     }
  82 
  83     pub async fn regenerate_segment_efforts(&self, segment_id: i32) -> Result<QueuedJobReference, String> {
  84         if segment_id <= 0 {
! 85             return Err("Segment ID must be positive".into());
  86         }
  87         self.submit(
  88             "regenerate_segment_efforts",
  89             &SegmentChange {
```


---


```
  108     }
  109 
  110     async fn user_page(&self, name: &str, user_id: i32) -> Result<(), String> {
  111         if user_id <= 0 {
! 112             return Err("User ID must be positive".into());
  113         }
  114         self.submit(name, &UserPage { user_id, after_id: 0 }).await.map(|_| ())
  115     }
```


---


```
  143     }
  144 
  145     pub async fn archive_activity_import(&self, job_id: i32) -> Result<(), String> {
  146         if job_id <= 0 {
! 147             return Err("Archive ID must be positive".into());
  148         }
  149         self.submit("activity_archive_import", &ArchiveInput { job_id, cursor: None })
  150             .await
  151             .map(|_| ())
```


---



## bike-rs/bike-core/src/jobs/native&#46;rs

```
   7 };
   8 
   9 pub type Error = Box<dyn std::error::Error + Send + Sync>;
  10 
! 11 pub async fn connect() -> Result<Client, Error> {
! 12     let address = std::env::var("TEMPORAL_ADDRESS").unwrap_or_else(|_| "http://temporal:7233".into());
! 13     let namespace = std::env::var("TEMPORAL_NAMESPACE").unwrap_or_else(|_| "bike".into());
! 14     let options = ConnectionOptions::new(address.parse::<temporalio_client::Url>()?)
! 15         .identity("bike-tasks")
! 16         .connect_timeout(Duration::from_secs(5))
! 17         .build();
! 18     Ok(Client::new(
! 19         Connection::connect(options).await?,
! 20         ClientOptions::new(namespace).build(),
! 21     )?)
! 22 }
  23 
  24 pub async fn submit(client: &Client, name: &str, input: serde_json::Value) -> Result<QueuedJobReference, Error> {
  25     let id = format!("bike-{name}-{}", uuid::Uuid::new_v4());
  26     client
```


---



## bike-rs/bike-core/src/segment_regeneration&#46;rs

```
  75         .ok_or_else(|| crate::workflow_error::WorkflowError::not_found("Activity was deleted"))?;
  76     let route = if current.is_bike_activity() {
  77         deserialize_derived_activity_data(current.derived_data_json.as_ref()).route_points
  78     } else {
! 79         Vec::new()
  80     };
  81     let mut ids = crate::entities::segment_user_summaries::Model::segment_ids_for_user(&txn, current.user_id).await?;
  82     ids.extend(
  83         crate::activity_lifecycle::refresh_activity_derived_state_without_cache_rebuilds(
```


---



## bike-rs/bike-core/src/strava/native/persistence&#46;rs

```
  52         }
  53         let existing =
  54             activities::Model::lock_strava_activity(&txn, input.event.bike_user_id, input.event.activity_id()?).await?;
  55         if let Some(change) = Self::persist_matching_content(&txn, &input, existing.as_ref()).await? {
! 56             txn.commit().await?;
  57             return Ok(change);
  58         }
  59         let upload = super::super::build_activity_upload(&summary.activity, &summary.streams)?;
  60         let previous_segment_ids = match &existing {
```


---


```
  110             return Ok(None);
  111         };
  112         if !strava_activity_states::Model::matches_content(db, &input.event, &input.source.sha256).await? {
  113             return Ok(None);
! 114         }
! 115         Self::persist_unchanged(db, input, activity).await.map(Some)
  116     }
  117 
  118     async fn persist_unchanged(
  119         db: &DatabaseTransaction,
```


---


```
  251         if let Some(id) = change.bike_activity_id {
  252             if let Some(activity) = activities::Model::lock_owned(&txn, id, change.event.bike_user_id).await? {
  253                 let derived =
  254                     crate::activity_data::deserialize_derived_activity_data(activity.derived_data_json.as_ref());
! 255                 let affected_segments =
! 256                     crate::activity_lifecycle::refresh_activity_derived_state_without_cache_rebuilds(
! 257                         &txn,
! 258                         change.event.bike_user_id,
! 259                         id,
! 260                         &derived.route_points,
! 261                     )
! 262                     .await?;
! 263                 change.include_segments(affected_segments);
  264                 crate::analytics::mark_segment_activity_changes(&txn, &change.segment_ids, chrono::Utc::now()).await?;
  265                 crate::analytics::rebuild_activity_analytics_cache(&txn, &[id]).await?;
  266                 crate::entities::activity_training_analyses::Model::clear_for_non_cycling(&txn, &activity).await?;
  267                 if activity.is_bike_activity() {
```


---



## bike-rs/bike-core/src/test_support/temporal_client&#46;rs

```
  55                             ..Default::default()
  56                         }
  57                         .encode_to_vec()
  58                     }
! 59                     other => panic!("Unexpected RPC {other}"),
  60                 };
  61                 Ok(GrpcSuccessResponse {
  62                     headers: Default::default(),
  63                     proto,
```


---



## bike-rs/bike-core/src/workflow_error&#46;rs

```
  125     }
  126 }
  127 
  128 impl From<crate::jobs::TaskError> for WorkflowError {
! 129     fn from(error: crate::jobs::TaskError) -> Self {
  130         Self::internal(error.to_string())
  131     }
  132 }
```


---


