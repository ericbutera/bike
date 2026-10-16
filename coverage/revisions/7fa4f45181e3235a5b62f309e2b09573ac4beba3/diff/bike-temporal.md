# Diff Coverage
## Diff: 7e43ab6d2d5b70d305a52e5f12bf704593c5fad7...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-temporal/src/activities&#46;rs (100%)
- bike-rs/bike-temporal/src/activities/imports&#46;rs (100%)
- bike-rs/bike-temporal/src/activities/imports/execution&#46;rs (87.5%): Missing lines 14-15
- bike-rs/bike-temporal/src/client&#46;rs (100%)
- bike-rs/bike-temporal/src/operations&#46;rs (0.0%): Missing lines 20-22,31,33,42,47,53,59,71,77,105,112
- bike-rs/bike-temporal/src/runtime&#46;rs (0.0%): Missing lines 17-29,31-48,50-53
- bike-rs/bike-temporal/src/submission&#46;rs (94.6%): Missing lines 73,75
- bike-rs/bike-temporal/src/workflows/children&#46;rs (100%)
- bike-rs/bike-temporal/src/workflows/options&#46;rs (100%)
- bike-rs/bike-temporal/src/workflows/process_activity_import&#46;rs (100%)
- bike-rs/bike-temporal/src/workflows/reprocess_activity_import&#46;rs (100%)
- bike-rs/bike-temporal/src/workflows/steps/derived&#46;rs (100%)
- bike-rs/bike-temporal/src/workflows/steps/imports&#46;rs (100%)

## Summary

- **Total**: 252 lines
- **Missing**: 52 lines
- **Coverage**: 79.37%



## bike-rs/bike-temporal/src/activities/imports/execution&#46;rs

```
  10 ) -> Result<ImportExecution, WorkflowError> {
  11     let info = ctx.info();
  12     let required = |value: &Option<String>| {
  13         value.clone().ok_or_else(|| {
! 14             WorkflowError::bad_request("Import requires a Temporal workflow execution")
! 15         })
  16     };
  17     Ok(ImportExecution {
  18         command,
  19         workflow: JobReference {
```


---



## bike-rs/bike-temporal/src/operations&#46;rs

```
  16     objects: ObjectStore,
  17 }
  18 
  19 impl BikeOperations {
! 20     pub fn new(db: DatabaseConnection, objects: ObjectStore) -> Self {
! 21         Self { db, objects }
! 22     }
  23 }
  24 
  25 #[async_trait::async_trait]
  26 impl ImportOperations for BikeOperations {
```


---


```
  27     async fn activity_import(
  28         &self,
  29         execution: ImportExecution,
  30         _context: ActivityContext,
! 31     ) -> Result<ActivityImportResult, WorkflowError> {
  32         bike_core::activity_reprocessing::reprocess(&self.db, &self.objects, execution).await
! 33     }
  34 }
  35 
  36 #[async_trait::async_trait]
  37 impl DerivedOperations for BikeOperations {
```


---


```
  38     async fn rebuild_fitness_freshness(
  39         &self,
  40         change: FitnessChange,
  41         _context: ActivityContext,
! 42     ) -> Result<Publication, WorkflowError> {
  43         bike_core::analytics::rebuild_fitness_freshness_cache(&self.db, change.user_id).await?;
  44         let state =
  45             bike_core::entities::analytics_user_states::Model::for_user(&self.db, change.user_id)
  46                 .await?;
! 47         if state.is_some_and(|state| state.fitness_dirty_from_day.is_some()) {
  48             return Err(WorkflowError::internal(
  49                 "Fitness changed during rebuild; retry current source",
  50             ));
  51         }
```


---


```
  49                 "Fitness changed during rebuild; retry current source",
  50             ));
  51         }
  52         Ok(Publication::Published)
! 53     }
  54 
  55     async fn rebuild_segment_analytics(
  56         &self,
  57         change: SegmentChange,
```


---


```
  55     async fn rebuild_segment_analytics(
  56         &self,
  57         change: SegmentChange,
  58         _context: ActivityContext,
! 59     ) -> Result<Publication, WorkflowError> {
  60         use bike_core::entities::segments;
  61         let before = segments::Model::find_by_id(&self.db, change.segment_id).await?;
  62         bike_core::analytics::rebuild_segment_analytics_cache(&self.db, &[change.segment_id])
  63             .await?;
```


---


```
  67                 "Segment changed during rebuild; retry current source",
  68             ));
  69         }
  70         Ok(Publication::Published)
! 71     }
  72 
  73     async fn prepare_heatmap(
  74         &self,
  75         change: HeatmapChange,
```


---


```
  73     async fn prepare_heatmap(
  74         &self,
  75         change: HeatmapChange,
  76         _context: ActivityContext,
! 77     ) -> Result<Publication, WorkflowError> {
  78         use bike_core::{
  79             entities::heatmap_projections,
  80             heatmaps::{
  81                 preparation,
```


---


```
  101         }
  102         let current =
  103             heatmap_projections::Model::for_activity(&self.db, change.activity_id).await?;
  104         if current
! 105             .is_some_and(|projection| matches!(projection.status.as_str(), "pending" | "failed"))
  106         {
  107             return Err(WorkflowError::internal(
  108                 "Heatmap changed during preparation; rebuild current source",
  109             ));
```


---


```
  108                 "Heatmap changed during preparation; rebuild current source",
  109             ));
  110         }
  111         Ok(Publication::Published)
! 112     }
  113 }
```


---



## bike-rs/bike-temporal/src/runtime&#46;rs

```
  13 use std::{sync::Arc, time::Duration};
  14 use temporalio_client::{Client, ClientOptions, Connection, ConnectionOptions};
  15 use temporalio_sdk::{Runtime, Worker, WorkerOptions};
  16 
! 17 pub async fn connect() -> Result<Client, crate::client::Error> {
! 18     let address =
! 19         std::env::var("TEMPORAL_ADDRESS").unwrap_or_else(|_| "http://temporal:7233".into());
! 20     let namespace = std::env::var("TEMPORAL_NAMESPACE").unwrap_or_else(|_| "bike".into());
! 21     let options = ConnectionOptions::new(address.parse::<temporalio_client::Url>()?)
! 22         .identity("bike-tasks")
! 23         .connect_timeout(Duration::from_secs(5))
! 24         .build();
! 25     Ok(Client::new(
! 26         Connection::connect(options).await?,
! 27         ClientOptions::new(namespace).build(),
! 28     )?)
! 29 }
  30 
! 31 pub fn worker(
! 32     runtime: &Runtime,
! 33     client: Client,
! 34     db: sea_orm::DatabaseConnection,
! 35     uploads_dir: String,
! 36 ) -> Result<Worker, crate::client::Error> {
! 37     let objects = ObjectStore::filesystem(uploads_dir, ObjectOwner::ActivityImports);
! 38     let operations = Arc::new(BikeOperations::new(db, objects));
! 39     let options = WorkerOptions::new(TASK_QUEUE)
! 40         .register_workflow::<ReprocessActivityImport>()?
! 41         .register_workflow::<RebuildFitnessFreshness>()?
! 42         .register_workflow::<RebuildSegmentAnalytics>()?
! 43         .register_workflow::<PrepareHeatmap>()?
! 44         .register_activities(ImportActivities::new(operations.clone()))
! 45         .register_activities(DerivedActivities::new(operations))
! 46         .register_activities(SchedulingActivities::new(
! 47             client.clone(),
! 48             DebouncePolicy::default(),
  49         ))
! 50         .graceful_shutdown_period(Duration::from_secs(10))
! 51         .build();
! 52     Ok(Worker::new(runtime, client, options)?)
! 53 }
```


---



## bike-rs/bike-temporal/src/submission&#46;rs

```
  69     async fn reprocess_activity_import(
  70         &self,
  71         input: ReprocessInput,
  72         cause: TaskCause,
! 73     ) -> Result<JobReference, Error> {
  74         TaskSubmissionService::reprocess_activity_import(self, input, cause).await
! 75     }
  76 }
```


---


