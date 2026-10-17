# Diff Coverage
## Diff: 6718a8f493d55e7c624283bedcba8a97808385c6...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-core/src/activity_import_execution&#46;rs (100%)
- bike-rs/bike-core/src/activity_import_pipeline&#46;rs (64.8%): Missing lines 304,309,319,325,328,332-334,467,469-470,472,477-478,480,483,486,1094,1202
- bike-rs/bike-core/src/activity_import_pipeline/replay&#46;rs (100%)
- bike-rs/bike-core/src/activity_import_pipeline/stage_execution&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/admin&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/history&#46;rs (100%)

## Summary

- **Total**: 135 lines
- **Missing**: 19 lines
- **Coverage**: 85.93%



## bike-rs/bike-core/src/activity_import_pipeline&#46;rs

```
  300     crate::activity_import_lifecycle::record_activity_processing_event(db, import, event_type, level, message, payload)
  301         .await;
  302 }
  303 
! 304 async fn run_activity_processing_stages(
  305     run: ActivityProcessingRun<'_>,
  306 ) -> Result<PersistActivityUploadOutcome, AppError> {
  307     run.import.require_sql_processing()?;
  308     let attempt = begin_processing_attempt(&run).await?;
! 309     let result = execute_activity_processing_stages(&run, attempt.clone()).await;
  310     if let Err(error) = &result {
  311         let own_attempt = activity_import_attempts::Entity::find_for_execution(run.db, attempt.id).await?;
  312         if let Some(own_attempt) = own_attempt.filter(|attempt| attempt.status == "running") {
  313             mark_activity_import_failed(run.db, &run.import, &own_attempt.current_stage, error).await?;
```


---


```
  315     }
  316     result
  317 }
  318 
! 319 async fn execute_activity_processing_stages(
  320     run: &ActivityProcessingRun<'_>,
  321     attempt: activity_import_attempts::Model,
  322 ) -> Result<PersistActivityUploadOutcome, AppError> {
  323     let mut state = load_activity_processing_state(run, attempt).await?;
```


---


```
  321     attempt: activity_import_attempts::Model,
  322 ) -> Result<PersistActivityUploadOutcome, AppError> {
  323     let mut state = load_activity_processing_state(run, attempt).await?;
  324 
! 325     for stage in activity_processing_stages() {
  326         if execution::stages(&state.attempt)?
  327             .iter()
! 328             .any(|record| record.stage == stage.id() && record.status == "reused")
  329         {
  330             continue;
  331         }
! 332         start_processing_node(run, &mut state, stage.id()).await?;
! 333         let outcome = run_activity_processing_node(run, &mut state, *stage).await?;
! 334         complete_processing_node(run, &mut state, *stage).await?;
  335         if let Some(outcome) = outcome {
  336             execution::finish_active(run.db, &state.import_model, "duplicate", None).await?;
  337             return Ok(outcome);
  338         }
```


---


```
  463 
  464 async fn run_activity_processing_node(
  465     run: &ActivityProcessingRun<'_>,
  466     state: &mut ActivityProcessingState,
! 467     stage: ActivityProcessingStage,
  468 ) -> Result<Option<PersistActivityUploadOutcome>, AppError> {
! 469     match stage {
! 470         ActivityProcessingStage::RawStored => mark_raw_stored_node(run, state).await?,
  471         ActivityProcessingStage::ActivityParsed => {
! 472             parse_activity_node(run, state, stage.id()).await?;
  473             if let Some(outcome) = prevent_duplicate_activity(run, state).await? {
  474                 return Ok(Some(outcome));
  475             }
  476         }
! 477         ActivityProcessingStage::ActivitySaved => save_activity_node(run, state, stage.id()).await?,
! 478         ActivityProcessingStage::SegmentsBuilt => build_segments_node(run, state, stage.id()).await?,
  479         ActivityProcessingStage::SegmentAnalyticsBuilt => {
! 480             build_segment_analytics_node(run, state, stage.id()).await?;
  481         }
  482         ActivityProcessingStage::ActivityAnalyticsBuilt => {
! 483             build_activity_analytics_node(run, state, stage.id()).await?;
  484         }
  485         ActivityProcessingStage::TrainingAnalysisBuilt => {
! 486             build_training_analysis_node(run, state, stage.id()).await?;
  487         }
  488     }
  489 
  490     Ok(None)
```


---


```
  1090     activity: activities::Model,
  1091     activity_import: activity_imports::Model,
  1092     training_profile: Option<&TrainingProfile>,
  1093 ) -> Result<ReprocessedActivityImport, AppError> {
! 1094     let outcome = run_activity_processing_stages(ActivityProcessingRun {
  1095         db,
  1096         uploads_dir,
  1097         user_id,
  1098         import: activity_import,
```


---


```
  1198         let expected_import = state.import_model.clone();
  1199         let expected_segments = state.affected_segment_ids.clone();
  1200         let expected_dirty_day = state.parsed_activity.as_ref().unwrap().draft.started_at.date_naive();
  1201         let PersistActivityUploadOutcome::Imported(result) = activity_processing_result(state).unwrap() else {
! 1202             panic!("Completed source must return its imported activity");
  1203         };
  1204         assert_eq!(result.activity.id, expected_activity.id);
  1205         assert_eq!(result.import.id, expected_import.id);
  1206         assert_eq!(result.affected_segment_ids, expected_segments);
```


---


