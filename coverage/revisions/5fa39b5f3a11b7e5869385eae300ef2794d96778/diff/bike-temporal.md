# Diff Coverage
## Diff: e401699acdc0b5280a64d13d689fc3efd11cdd43...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-temporal/src/activities&#46;rs (97.4%): Missing lines 68
- bike-rs/bike-temporal/src/activities/batches&#46;rs (100%)
- bike-rs/bike-temporal/src/activities/derived&#46;rs (100%)
- bike-rs/bike-temporal/src/activities/imports&#46;rs (100%)
- bike-rs/bike-temporal/src/client&#46;rs (100%)
- bike-rs/bike-temporal/src/operations&#46;rs (95.0%): Missing lines 52
- bike-rs/bike-temporal/src/operations/batches&#46;rs (60.0%): Missing lines 38,41,49,51,53,55,57,59
- bike-rs/bike-temporal/src/runtime&#46;rs (33.3%): Missing lines 20-21,44,54-55,57-63,65,70-77,79
- bike-rs/bike-temporal/src/workflows/activity_archive_import&#46;rs (100%)
- bike-rs/bike-temporal/src/workflows/activity_pages&#46;rs (90.8%): Missing lines 65-67,91-92,95-97,154-156
- bike-rs/bike-temporal/src/workflows/children&#46;rs (93.3%): Missing lines 22
- bike-rs/bike-temporal/src/workflows/options&#46;rs (100%)
- bike-rs/bike-temporal/src/workflows/prepare_heatmap&#46;rs (100%)
- bike-rs/bike-temporal/src/workflows/process_activity_import&#46;rs (100%)
- bike-rs/bike-temporal/src/workflows/rebuild_fitness_freshness&#46;rs (100%)
- bike-rs/bike-temporal/src/workflows/rebuild_segment_analytics&#46;rs (100%)
- bike-rs/bike-temporal/src/workflows/reconcile_heatmaps&#46;rs (89.3%): Missing lines 37-39
- bike-rs/bike-temporal/src/workflows/regenerate_activity_segments&#46;rs (100%)
- bike-rs/bike-temporal/src/workflows/regenerate_segment_efforts&#46;rs (100%)
- bike-rs/bike-temporal/src/workflows/reprocess_activity_import&#46;rs (100%)
- bike-rs/bike-temporal/src/workflows/steps/derived&#46;rs (100%)
- bike-rs/bike-temporal/src/workflows/steps/imports&#46;rs (100%)

## Summary

- **Total**: 524 lines
- **Missing**: 47 lines
- **Coverage**: 91.03%



## bike-rs/bike-temporal/src/activities&#46;rs

```
  64             (WorkflowError::not_found("retained file missing"), true),
  65             (WorkflowError::internal("temporary storage failure"), false),
  66         ] {
  67             let ActivityError::Application(failure) = domain_error(error) else {
! 68                 panic!("Expected native application failure");
  69             };
  70             assert_eq!(failure.is_non_retryable(), terminal);
  71         }
  72     }
```


---



## bike-rs/bike-temporal/src/operations&#46;rs

```
  48 }
  49 
  50 #[async_trait::async_trait]
  51 impl DerivedOperations for BikeOperations {
! 52     async fn rebuild_activity_segments(&self, task: ActivityTask) -> Result<Vec<i32>, WorkflowError> {
  53         let activity = self.activity(&task).await?;
  54         bike_core::segment_regeneration::match_activity(&self.db, &activity).await
  55     }
```


---



## bike-rs/bike-temporal/src/operations/batches&#46;rs

```
  34         )
  35         .await
  36     }
  37 
! 38     async fn regenerate_segment_efforts(&self, segment_id: i32) -> Result<(), WorkflowError> {
  39         bike_core::segment_regeneration::regenerate_segment_efforts(&self.db, segment_id).await?;
  40         Ok(())
! 41     }
  42 
  43     async fn finish_training_backfill(&self, user_id: i32) -> Result<(), WorkflowError> {
  44         bike_core::xc_goal_backfill::mark_user_xc_goal_backfill_completed(&self.db, user_id, chrono::Utc::now())
  45             .await?;
```


---


```
  45             .await?;
  46         Ok(())
  47     }
  48 
! 49     async fn prepare_archive(&self, job_id: i32) -> Result<ArchiveCursor, WorkflowError> {
  50         bike_core::archive_import::native::prepare(&self.db, &self.uploads_dir, job_id).await
! 51     }
  52 
! 53     async fn archive_page(&self, cursor: ArchiveCursor) -> Result<ArchivePage, WorkflowError> {
  54         bike_core::archive_import::native::page(&self.db, &self.uploads_dir, cursor).await
! 55     }
  56 
! 57     async fn finish_archive(&self, job_id: i32) -> Result<(), WorkflowError> {
  58         bike_core::archive_import::native::finish(&self.db, job_id).await
! 59     }
  60 
  61     async fn pending_heatmaps(&self, after_id: i32) -> Result<Vec<ActivityTask>, WorkflowError> {
  62         bike_core::heatmaps::projection::Projection::pending_tasks(&self.db, after_id)
  63             .await
```


---



## bike-rs/bike-temporal/src/runtime&#46;rs

```
  16 use temporalio_sdk::runtime::worker_tuner::{FixedSizeSlotSupplier, TunerHolder};
  17 use temporalio_sdk::{Runtime, Worker, WorkerOptions};
  18 
  19 pub async fn connect() -> Result<Client, crate::client::Error> {
! 20     bike_core::jobs::native::connect().await
! 21 }
  22 
  23 pub async fn ensure_schedule(client: &Client) -> Result<(), crate::client::Error> {
  24     use temporalio_client::schedules::{
  25         CreateScheduleOptions, ScheduleAction, ScheduleError, ScheduleIntervalSpec, ScheduleOverlapPolicy, ScheduleSpec,
```


---


```
  40         .build();
  41     match client.create_schedule("bike-heatmap-reconciliation", options).await {
  42         Ok(_) => Ok(()),
  43         Err(ScheduleError::Rpc(status)) if status.code() == tonic::Code::AlreadyExists => Ok(()),
! 44         Err(error) => Err(error.into()),
  45     }
  46 }
  47 
  48 pub fn worker(
```


---


```
  50     client: Client,
  51     db: sea_orm::DatabaseConnection,
  52     uploads_dir: String,
  53 ) -> Result<Worker, crate::client::Error> {
! 54     let objects = ObjectStore::filesystem(&uploads_dir, ObjectOwner::ActivityImports);
! 55     let operations = Arc::new(BikeOperations::new(db, objects, uploads_dir));
  56     let options = WorkerOptions::new(TASK_QUEUE)
! 57         .tuner(
! 58             TunerHolder::builder()
! 59                 .workflow_task_slot_supplier(FixedSizeSlotSupplier::new(16))
! 60                 .activity_task_slot_supplier(FixedSizeSlotSupplier::new(2))
! 61                 .local_activity_task_slot_supplier(FixedSizeSlotSupplier::new(1))
! 62                 .nexus_task_slot_supplier(FixedSizeSlotSupplier::new(1))
! 63                 .build(),
  64         )
! 65         .register_workflow::<ProcessActivityImport>()?
  66         .register_workflow::<ReprocessActivityImport>()?
  67         .register_workflow::<RebuildFitnessFreshness>()?
  68         .register_workflow::<RebuildSegmentAnalytics>()?
  69         .register_workflow::<PrepareHeatmap>()?
! 70         .register_workflow::<RegenerateActivitySegments>()?
! 71         .register_workflow::<RegenerateSegmentEfforts>()?
! 72         .register_workflow::<RegenerateUserSegments>()?
! 73         .register_workflow::<ReprocessUserActivityImports>()?
! 74         .register_workflow::<ReprocessArchiveFitActivityImports>()?
! 75         .register_workflow::<BackfillUserXcTraining>()?
! 76         .register_workflow::<ActivityArchiveImport>()?
! 77         .register_workflow::<ReconcileHeatmaps>()?
  78         .register_activities(ImportActivities::new(operations.clone()))
! 79         .register_activities(BatchActivities::new(operations.clone()))
  80         .register_activities(DerivedActivities::new(operations))
  81         .graceful_shutdown_period(Duration::from_secs(10))
  82         .build();
  83     Ok(Worker::new(runtime, client, options)?)
```


---



## bike-rs/bike-temporal/src/workflows/activity_pages&#46;rs

```
  61             input.after_id = task.activity_id;
  62             reprocess(ctx, task).await?;
  63         }
  64         if full {
! 65             return ctx
! 66                 .continue_as_new(input, Default::default())
! 67                 .map(|never| match never {});
  68         }
  69         Ok(())
  70     }
  71 }
```


---


```
   87         )
   88         .await?;
   89         let full = tasks.len() == 16;
   90         for task in tasks {
!  91             input.after_id = task.activity_id;
!  92             reprocess(ctx, task).await?;
   93         }
   94         if full {
!  95             return ctx
!  96                 .continue_as_new(input, Default::default())
!  97                 .map(|never| match never {});
   98         }
   99         Ok(())
  100     }
  101 }
```


---


```
  150             )
  151             .await?;
  152         }
  153         if full {
! 154             return ctx
! 155                 .continue_as_new(input, Default::default())
! 156                 .map(|never| match never {});
  157         }
  158         options::run(
  159             ctx,
  160             BatchActivities::finish_training_backfill,
```


---



## bike-rs/bike-temporal/src/workflows/children&#46;rs

```
  18     D::Input: Serialize,
  19     D::Output: TemporalDeserializable,
  20 {
  21     if ctx.cancellation_token().is_cancelled() {
! 22         return Err(temporalio_sdk::WorkflowTermination::cancelled());
  23     }
  24     let child_options = ChildWorkflowOptions::builder()
  25         .task_queue(crate::TASK_QUEUE.to_owned())
  26         .priority(options::priority(priority, resource))
```


---



## bike-rs/bike-temporal/src/workflows/reconcile_heatmaps&#46;rs

```
  33             )
  34             .await?;
  35         }
  36         if full {
! 37             return ctx
! 38                 .continue_as_new(next, Default::default())
! 39                 .map(|never| match never {});
  40         }
  41         Ok(())
  42     }
  43 }
```


---


