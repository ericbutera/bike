# Diff Coverage
## Diff: 7e43ab6d2d5b70d305a52e5f12bf704593c5fad7...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-core/src/activity_import_pipeline&#46;rs (61.5%): Missing lines 356-357,713-727,729-731
- bike-rs/bike-core/src/activity_import_pipeline/replay&#46;rs (0.0%): Missing lines 211-218
- bike-rs/bike-core/src/activity_import_pipeline/stage_execution&#46;rs (0.0%): Missing lines 11-12,14-15,163,165
- bike-rs/bike-core/src/activity_reprocessing&#46;rs (90.3%): Missing lines 36-38,107-109,128-130,142-144
- bike-rs/bike-core/src/bike_tasks&#46;rs (79.3%): Missing lines 79-81,101-103
- bike-rs/bike-core/src/bike_tasks/submission&#46;rs (100%)
- bike-rs/bike-core/src/bin/recover-generated-imports&#46;rs (100%)
- bike-rs/bike-core/src/entities/activities&#46;rs (100%)
- bike-rs/bike-core/src/entities/activities/history&#46;rs (100%)
- bike-rs/bike-core/src/entities/activities/parsed&#46;rs (100%)
- bike-rs/bike-core/src/entities/activities/reprocessing&#46;rs (93.1%): Missing lines 60,78-79,109,117-119
- bike-rs/bike-core/src/entities/activity_event_history&#46;rs (97.4%): Missing lines 137,141-143
- bike-rs/bike-core/src/entities/activity_imports&#46;rs (88.0%): Missing lines 64-66
- bike-rs/bike-core/src/entities/heatmap_projections/lifecycle&#46;rs (0.0%): Missing lines 132-135
- bike-rs/bike-core/src/entities/strava_activity_states&#46;rs (100%)
- bike-rs/bike-core/src/entities/strava_events&#46;rs (100%)
- bike-rs/bike-core/src/object_store&#46;rs (100%)
- bike-rs/bike-core/src/platform/temporal&#46;rs (100%)
- bike-rs/bike-core/src/services/training_goals&#46;rs (100%)
- bike-rs/bike-core/src/strava/client&#46;rs (0.0%): Missing lines 205-213,423-425
- bike-rs/bike-core/src/strava/native/persistence&#46;rs (85.7%): Missing lines 338-344
- bike-rs/bike-core/src/strava/native/persistence/strava_native_unit_deletion&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/persistence/strava_native_unit_history&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/strava_native_unit/fixture&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/strava_native_unit/upsert&#46;rs (100%)
- bike-rs/bike-core/src/test_support/activity_history&#46;rs (100%)
- bike-rs/bike-core/src/test_support/imports&#46;rs (0.0%): Missing lines 3-35
- bike-rs/bike-core/src/test_support/reprocessing&#46;rs (100%)
- bike-rs/bike-core/src/test_support/tasks&#46;rs (100%)
- bike-rs/bike-core/src/xc_goal_backfill&#46;rs (0.0%): Missing lines 167-168

## Summary

- **Total**: 985 lines
- **Missing**: 124 lines
- **Coverage**: 87.41%



## bike-rs/bike-core/src/activity_import_pipeline&#46;rs

```
  352 
  353 impl ActivityProcessingArtifact {
  354     pub(crate) fn object_reference(&self) -> Result<ObjectRef, AppError> {
  355         let sha256 = self.checksum_sha256.clone().ok_or_else(|| {
! 356             AppError::bad_request("Retained source needs a checksum before native reprocessing")
! 357         })?;
  358         Ok(ObjectRef {
  359             path: self.storage_path.clone(),
  360             sha256,
  361             bytes: self
```


---


```
  709         artifact_kind: &state.parsing_artifact.artifact_kind,
  710         source_quality: &state.parsing_artifact.source_quality,
  711         bytes: &state.bytes,
  712     })?;
! 713     enrich_recording(
! 714         run.db,
! 715         &ObjectStore::filesystem(run.uploads_dir, ObjectOwner::ActivityImports),
! 716         &state.import_model,
! 717         &state.parsing_artifact,
! 718         state.activity_model.as_ref(),
! 719         &mut parsed,
! 720     )
! 721     .await?;
! 722     state.parsed_activity = Some(parsed);
! 723     state.import_model = mark_activity_import_processing_stage(
! 724         run.db,
! 725         &state.import_model,
! 726         stage,
! 727         state.activity_model.as_ref().map(|activity| activity.id),
  728     )
! 729     .await?;
! 730     Ok(())
! 731 }
  732 
  733 /// Preserve the same recording/privacy classification on legacy and native parses.
  734 pub(crate) async fn enrich_recording(
  735     db: &DatabaseConnection,
```


---



## bike-rs/bike-core/src/activity_import_pipeline/replay&#46;rs

```
  207     .await?;
  208     import
  209         .mark_processing(&transaction, &plan.start_stage, None, Utc::now())
  210         .await?;
! 211     durable::Model::enqueue(
! 212         &transaction,
! 213         "process_activity_import".into(),
! 214         serde_json::json!({"data": {"user_id": import.user_id, "import_id": import.id, "attempt_id": attempt.id}}),
! 215         None,
! 216         1,
! 217     )
! 218     .await?;
  219     transaction.commit().await?;
  220     Ok((plan, attempt.id))
  221 }
```


---



## bike-rs/bike-core/src/activity_import_pipeline/stage_execution&#46;rs

```
   7         .await?
   8     {
   9         Some(attempt) => attempt,
  10         None => {
! 11             let source = load_best_activity_parsing_artifact(run.db, &run.import)
! 12                 .await
  13                 .map(|artifact| artifact_metadata(&artifact))
! 14                 .unwrap_or_else(
! 15                     |_| serde_json::json!({"filename": run.import.original_filename, "format": run.import.format}),
  16                 );
  17             execution::create_attempt(
  18                 run.db,
  19                 &run.import,
```


---


```
  159             .map_or_else(Vec::new, |activity| {
  160                 vec![format!("Saved activity #{}", activity.id)]
  161             }),
  162         ActivityProcessingNode::SegmentsBuilt => {
! 163             vec![format!(
  164                 "{} affected segments",
! 165                 state.affected_segment_ids.len()
  166             )]
  167         }
  168         ActivityProcessingNode::SegmentAnalyticsBuilt => vec![format!(
  169             "Rebuilt analytics for {} segments",
```


---



## bike-rs/bike-core/src/activity_reprocessing&#46;rs

```
  32         &self,
  33         command: &crate::bike_tasks::ActivityImport,
  34     ) -> Result<ActivityImportResult, WorkflowError> {
  35         let ImportTarget::ExistingActivity { activity_id, .. } = command.target else {
! 36             return Err(WorkflowError::bad_request(
! 37                 "This adapter reprocesses existing activities",
! 38             ));
  39         };
  40         let source = &command.source;
  41         Ok(ActivityImportResult {
  42             activity_id,
```


---


```
  103         .await?
  104         .ok_or_else(|| WorkflowError::not_found("Activity import not found"))?;
  105     import.require_sql_processing()?;
  106     if import.activity_id != Some(activity.id) {
! 107         return Err(WorkflowError::conflict(
! 108             "Import does not belong to this activity",
! 109         ));
  110     }
  111     Ok(import)
  112 }
```


---


```
  124     execution: ImportExecution,
  125 ) -> Result<ActivityImportResult, WorkflowError> {
  126     execution.command.validate()?;
  127     let ImportTarget::ExistingActivity { activity_id, .. } = execution.command.target else {
! 128         return Err(WorkflowError::bad_request(
! 129             "New imports have not been converted",
! 130         ));
  131     };
  132     if let Some(scope) = activity_event_history::Model::import_refresh(db, &execution).await? {
  133         return scope.result(&execution.command);
  134     }
```


---


```
  138         .ok_or_else(|| WorkflowError::not_found("Activity not found"))?;
  139     let import = owned_import(db, &activity).await?;
  140     let artifact = load_best_activity_parsing_artifact(db, &import).await?;
  141     if import.id != source.activity_import_id || artifact.object_reference()? != source.object {
! 142         return Err(WorkflowError::conflict(
! 143             "Retained source changed since submission",
! 144         ));
  145     }
  146     let bytes = objects.read(&source.object).await?;
  147     let mut parsed = parse_activity_artifact(ActivityParserArtifact {
  148         original_filename: &artifact.original_filename,
```


---



## bike-rs/bike-core/src/bike_tasks&#46;rs

```
  75 
  76 impl ActivityImport {
  77     pub fn validate_result(&self, result: &ActivityImportResult) -> Result<(), WorkflowError> {
  78         if result.activity_import_id != self.source.activity_import_id {
! 79             return Err(WorkflowError::conflict(
! 80                 "Result does not belong to the requested import",
! 81             ));
  82         }
  83         if let ImportTarget::ExistingActivity { activity_id, .. } = self.target {
  84             if result.activity_id != activity_id {
  85                 return Err(WorkflowError::conflict(
```


---


```
   97         if self.source.command_id.trim().is_empty() {
   98             return Err(WorkflowError::bad_request("Import requires a command ID"));
   99         }
  100         if self.source.user_id <= 0 || self.source.activity_import_id <= 0 {
! 101             return Err(WorkflowError::bad_request(
! 102                 "Import requires an owner and import ID",
! 103             ));
  104         }
  105         if let Some(cause) = &self.source.cause {
  106             cause.validate(self.source.user_id)?;
  107         }
```


---



## bike-rs/bike-core/src/entities/activities/reprocessing&#46;rs

```
  56     ) -> Result<RefreshScope, WorkflowError> {
  57         let route = if self.is_bike_activity() {
  58             parsed.derived_data.route_points.as_slice()
  59         } else {
! 60             &[]
  61         };
  62         let segment_ids =
  63             crate::activity_lifecycle::refresh_activity_derived_state_without_cache_rebuilds(
  64                 txn,
```


---


```
  74                 &[self.id],
  75             )
  76             .await?;
  77         } else {
! 78             crate::entities::activity_training_analyses::Model::clear_activity(txn, self.id)
! 79                 .await?;
  80         }
  81         let dirty_from = before
  82             .started_at
  83             .date_naive()
```


---


```
  105             activity_id,
  106             expected_revision,
  107         } = execution.command.target
  108         else {
! 109             return Err(WorkflowError::bad_request("Expected a reprocess command"));
  110         };
  111         let target_matches =
  112             self.id == activity_id && self.user_id == execution.command.source.user_id;
  113         let source_matches = self.activity_import_id
```


---


```
  113         let source_matches = self.activity_import_id
  114             == Some(execution.command.source.activity_import_id)
  115             && self.source != "strava_sync";
  116         if !target_matches || !source_matches {
! 117             return Err(WorkflowError::conflict(
! 118                 "Reprocess target does not match its retained source",
! 119             ));
  120         }
  121         if revision(self.updated_at)? != expected_revision {
  122             return Err(WorkflowError::conflict(
  123                 "Activity changed since reprocessing was requested",
```


---



## bike-rs/bike-core/src/entities/activity_event_history&#46;rs

```
  133                     .and_then(|data| data.source_metadata()),
  134             )
  135             .map_err(json_error)?,
  136         );
! 137     }
  138     Ok(snapshot)
  139 }
  140 
! 141 fn json_error(error: serde_json::Error) -> DbErr {
! 142     DbErr::Custom(error.to_string())
! 143 }
  144 
  145 impl Model {
  146     /// The audit event is also the natural command key for a committed import.
  147     pub async fn import_refresh(
```


---



## bike-rs/bike-core/src/entities/activity_imports&#46;rs

```
  60             .filter(Column::UserId.eq(source.user_id))
  61             .exec(db)
  62             .await?;
  63         if changed.rows_affected != 1 {
! 64             return Err(crate::workflow_error::WorkflowError::conflict(
! 65                 "Reprocessed import disappeared",
! 66             ));
  67         }
  68         Ok(())
  69     }
  70 }
```


---



## bike-rs/bike-core/src/entities/heatmap_projections/lifecycle&#46;rs

```
  128         "#,[PROJECTION_VERSION.into(), crate::config::Config::get().worker_recovery_timeout_seconds.into(), crate::jobs::JobType::PrepareHeatmap.as_str().into()])).all(&txn).await?;
  129         if leased.is_empty() {
  130             return Ok(0);
  131         }
! 132         let activities: Vec<_> = leased
! 133             .iter()
! 134             .map(|pending| serde_json::json!({"activity_id":pending.activity_id,"generation":pending.generation}))
! 135             .collect();
  136         let task = crate::background_jobs::background_tasks::Model::enqueue(
  137             &txn,
  138             crate::jobs::JobType::PrepareHeatmap.as_str().into(),
  139             serde_json::json!({"type":"PrepareHeatmap","data":{"activities":activities}}),
```


---



## bike-rs/bike-core/src/strava/client&#46;rs

```
  201         access_token: &str,
  202         activity_id: i64,
  203     ) -> Result<StravaActivityStreams, AppError> {
  204         self.send_json(
! 205             streams_api::get_activity_streams(
! 206                 &self.sdk(Some(access_token)),
! 207                 activity_id,
! 208                 "time,distance,latlng,altitude,velocity_smooth,heartrate,cadence,watts,temp,moving,grade_smooth"
! 209                     .split(',')
! 210                     .map(str::to_owned)
! 211                     .collect(),
! 212                 true,
! 213             ),
  214             StravaRequestClass::Read,
  215             "get_activity_streams",
  216             "fetch Strava activity streams",
  217         )
```


---


```
  419             ));
  420         }
  421 
  422         if !sdk_accepted {
! 423             return Err(AppError::internal(format!(
! 424                 "Generated Strava SDK rejected the response while trying to {action}; inspect the retained exchange"
! 425             )));
  426         }
  427         decode_response(&bytes, action)
  428     }
```


---



## bike-rs/bike-core/src/strava/native/persistence&#46;rs

```
  334             if let Some(activity) = activities::Model::find_by_id(&txn, id).await? {
  335                 let derived = crate::activity_data::deserialize_derived_activity_data(
  336                     activity.derived_data_json.as_ref(),
  337                 );
! 338                 change.segment_ids = crate::activity_lifecycle::refresh_activity_derived_state_without_cache_rebuilds(
! 339                     &txn,
! 340                     change.event.bike_user_id,
! 341                     id,
! 342                     &derived.route_points,
! 343                 )
! 344                 .await?;
  345                 crate::analytics::mark_segment_activity_changes(
  346                     &txn,
  347                     &change.segment_ids,
  348                     chrono::Utc::now(),
```


---



## bike-rs/bike-core/src/test_support/imports&#46;rs

```
   1 use crate::entities::{activity_archive_import_jobs, activity_import_artifacts};
   2 
!  3 pub fn activity_import_attempt() -> crate::entities::activity_import_attempts::Model {
!  4     let execution = super::import_execution();
!  5     let at = super::timestamp();
!  6     crate::entities::activity_import_attempts::Model {
!  7         id: 19,
!  8         user_id: execution.command.source.user_id,
!  9         activity_import_id: execution.command.source.activity_import_id,
! 10         activity_id: None,
! 11         worker_task_id: None,
! 12         command_id: Some(execution.command.source.command_id.clone()),
! 13         temporal_context: Some(serde_json::to_value(&execution).expect("valid fixture execution")),
! 14         status: "started".into(),
! 15         requested_stage: "raw_stored".into(),
! 16         start_stage: "raw_stored".into(),
! 17         current_stage: "raw_stored".into(),
! 18         reused_attempt_id: None,
! 19         source_json: serde_json::json!({
! 20             "filename": super::activity_import().original_filename,
! 21             "format": super::activity_import().format,
! 22         }),
! 23         stages_json: serde_json::to_value(
! 24             crate::activity_import_execution::initial_stages("raw_stored", None)
! 25                 .expect("valid fixture stages"),
! 26         )
! 27         .expect("valid fixture stage records"),
! 28         checkpoint_json: None,
! 29         error: None,
! 30         created_at: at,
! 31         started_at: Some(at),
! 32         finished_at: None,
! 33         updated_at: at,
! 34     }
! 35 }
  36 
  37 pub fn import_artifact() -> activity_import_artifacts::Model {
  38     let import = super::activity_import();
  39     activity_import_artifacts::Model {
```


---



## bike-rs/bike-core/src/xc_goal_backfill&#46;rs

```
  163         }
  164         XC_GOAL_BACKFILL_STATUS_WAITING => {
  165             "XC training backfill is waiting for the current activity processing job to finish."
  166         }
! 167         XC_GOAL_BACKFILL_STATUS_RUNNING => "XC training backfill is rebuilding historical training metrics.",
! 168         XC_GOAL_BACKFILL_STATUS_COMPLETED => "XC training backfill completed. Historical ride metrics are up to date.",
  169         XC_GOAL_BACKFILL_STATUS_FAILED => {
  170             "XC training backfill failed. Try saving again or queue a user-id backfill from admin analytics."
  171         }
  172         _ => "XC training backfill status is unknown.",
```


---


