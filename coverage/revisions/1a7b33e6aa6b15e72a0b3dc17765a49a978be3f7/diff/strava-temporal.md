# Diff Coverage
## Diff: 7d811e85cc1f1992a99002b0d609e7cbd9d9bdef...HEAD, staged, unstaged and untracked changes

- bike-rs/strava-temporal/src/activities&#46;rs (26.1%): Missing lines 27-31,42-48,53-59,61-67,69-75,77-88,90-101,103-109,111-117,119-125,127-133,135-146,161-172,174-180,182-193,195-206,208-219,221-232,234-245,247-258,260-271,273-284,301-303,347,358-362
- bike-rs/strava-temporal/src/client&#46;rs (15.8%): Missing lines 23,25-29,31,34-43,45,50-51,53-54,57-60,62,64-69,71,73-92,115-120,122-156,158-161,163,165,167-171,173-174,176,178,180-190,192-195,197-199,202-206
- bike-rs/strava-temporal/src/metrics&#46;rs (0.0%): Missing lines 11-33
- bike-rs/strava-temporal/src/workflows/activity_create&#46;rs (100%)
- bike-rs/strava-temporal/src/workflows/activity_delete&#46;rs (100%)
- bike-rs/strava-temporal/src/workflows/activity_update&#46;rs (100%)
- bike-rs/strava-temporal/src/workflows/activity_upsert&#46;rs (100%)
- bike-rs/strava-temporal/src/workflows/backfill&#46;rs (83.3%): Missing lines 32-35
- bike-rs/strava-temporal/src/workflows/collection&#46;rs (100%)
- bike-rs/strava-temporal/src/workflows/disconnect&#46;rs (92.7%): Missing lines 52-54
- bike-rs/strava-temporal/src/workflows/logging&#46;rs (99.4%): Missing lines 185
- bike-rs/strava-temporal/src/workflows/options&#46;rs (98.9%): Missing lines 34
- bike-rs/strava-temporal/src/workflows/scheduled_sync&#46;rs (100%)
- bike-rs/strava-temporal/src/workflows/sync&#46;rs (100%)

## Summary

- **Total**: 1074 lines
- **Missing**: 386 lines
- **Coverage**: 64.06%



## bike-rs/strava-temporal/src/activities&#46;rs

```
  23 pub struct StravaActivities {
  24     operations: StravaOperations,
  25 }
  26 impl StravaActivities {
! 27     pub fn new(db: sea_orm::DatabaseConnection) -> Self {
! 28         Self {
! 29             operations: StravaOperations::new(db),
! 30         }
! 31     }
  32 
  33     #[cfg(test)]
  34     pub(crate) fn with_operations(operations: StravaOperations) -> Self {
  35         Self { operations }
```


---


```
   38 
   39 #[activities]
   40 impl StravaActivities {
   41     #[activity(name = "strava_scan_connections")]
!  42     pub async fn scan_connections(
!  43         self: Arc<Self>,
!  44         ctx: ActivityContext,
!  45         request: ScanRequest,
!  46     ) -> Result<ScanPage, ActivityError> {
!  47         let info = ctx.info();
!  48         let span = tracing::info_span!("strava.scan", correlation_id = %request.correlation_id,
   49             temporal_namespace = info.namespace,
   50             temporal_workflow_id = info.workflow_id, temporal_run_id = info.workflow_run_id,
   51             temporal_activity_id = info.activity_id, temporal_attempt = info.attempt,
   52             trace_id = tracing::field::Empty, span_id = tracing::field::Empty);
!  53         bike_core::observability::record_span_trace_context(&span);
!  54         self.operations
!  55             .scan_connections(request)
!  56             .instrument(span)
!  57             .await
!  58             .map_err(activity_error)
!  59     }
   60     #[activity(name = "strava_resolve_collection_window")]
!  61     pub async fn collection_window(
!  62         self: Arc<Self>,
!  63         ctx: ActivityContext,
!  64         input: StravaEvent,
!  65     ) -> Result<CollectionInput, ActivityError> {
!  66         run(ctx, input.clone(), self.operations.collection_window(input)).await
!  67     }
   68     #[activity(name = "strava_accept_child_event")]
!  69     pub async fn accept_child(
!  70         self: Arc<Self>,
!  71         ctx: ActivityContext,
!  72         input: ChildEvent,
!  73     ) -> Result<StravaEvent, ActivityError> {
!  74         run(ctx, input.parent, self.operations.accept_child(input.child)).await
!  75     }
   76     #[activity(name = "strava_complete_sync")]
!  77     pub async fn complete_sync(
!  78         self: Arc<Self>,
!  79         ctx: ActivityContext,
!  80         input: CollectionResult,
!  81     ) -> Result<CollectionResult, ActivityError> {
!  82         run(
!  83             ctx,
!  84             input.event.clone(),
!  85             self.operations.complete_sync(input),
!  86         )
!  87         .await
!  88     }
   89     #[activity(name = "strava_complete_backfill")]
!  90     pub async fn complete_backfill(
!  91         self: Arc<Self>,
!  92         ctx: ActivityContext,
!  93         input: CollectionResult,
!  94     ) -> Result<CollectionResult, ActivityError> {
!  95         run(
!  96             ctx,
!  97             input.event.clone(),
!  98             self.operations.complete_backfill(input),
!  99         )
! 100         .await
! 101     }
  102     #[activity(name = "strava_begin_disconnect")]
! 103     pub async fn begin_disconnect(
! 104         self: Arc<Self>,
! 105         ctx: ActivityContext,
! 106         input: StravaEvent,
! 107     ) -> Result<Vec<i64>, ActivityError> {
! 108         run(ctx, input.clone(), self.operations.begin_disconnect(input)).await
! 109     }
  110     #[activity(name = "strava_revoke_connection")]
! 111     pub async fn revoke_connection(
! 112         self: Arc<Self>,
! 113         ctx: ActivityContext,
! 114         input: StravaEvent,
! 115     ) -> Result<StravaEvent, ActivityError> {
! 116         run(ctx, input.clone(), self.operations.revoke_connection(input)).await
! 117     }
  118     #[activity(name = "strava_remove_connection")]
! 119     pub async fn remove_connection(
! 120         self: Arc<Self>,
! 121         ctx: ActivityContext,
! 122         input: StravaEvent,
! 123     ) -> Result<StravaEvent, ActivityError> {
! 124         run(ctx, input.clone(), self.operations.remove_connection(input)).await
! 125     }
  126     #[activity(name = "strava_fetch_activity")]
! 127     pub async fn fetch_activity(
! 128         self: Arc<Self>,
! 129         ctx: ActivityContext,
! 130         input: StravaEvent,
! 131     ) -> Result<ActivitySource, ActivityError> {
! 132         run(ctx, input.clone(), self.operations.fetch_activity(input)).await
! 133     }
  134     #[activity(name = "strava_fetch_streams")]
! 135     pub async fn fetch_streams(
! 136         self: Arc<Self>,
! 137         ctx: ActivityContext,
! 138         input: ActivitySource,
! 139     ) -> Result<ActivitySources, ActivityError> {
! 140         run(
! 141             ctx,
! 142             input.event.clone(),
! 143             self.operations.fetch_streams(input),
! 144         )
! 145         .await
! 146     }
  147     #[activity(name = "strava_normalize_activity")]
  148     pub async fn normalize_activity(
  149         self: Arc<Self>,
  150         ctx: ActivityContext,
```


---


```
  157         )
  158         .await
  159     }
  160     #[activity(name = "strava_persist_activity")]
! 161     pub async fn persist_activity(
! 162         self: Arc<Self>,
! 163         ctx: ActivityContext,
! 164         input: NormalizedActivity,
! 165     ) -> Result<ActivityChange, ActivityError> {
! 166         run(
! 167             ctx,
! 168             input.event.clone(),
! 169             self.operations.persist_activity(input),
! 170         )
! 171         .await
! 172     }
  173     #[activity(name = "strava_capture_deletion")]
! 174     pub async fn capture_deletion(
! 175         self: Arc<Self>,
! 176         ctx: ActivityContext,
! 177         input: StravaEvent,
! 178     ) -> Result<ActivityChange, ActivityError> {
! 179         run(ctx, input.clone(), self.operations.capture_deletion(input)).await
! 180     }
  181     #[activity(name = "strava_delete_activity")]
! 182     pub async fn delete_activity(
! 183         self: Arc<Self>,
! 184         ctx: ActivityContext,
! 185         input: ActivityChange,
! 186     ) -> Result<ActivityChange, ActivityError> {
! 187         run(
! 188             ctx,
! 189             input.event.clone(),
! 190             self.operations.delete_activity(input),
! 191         )
! 192         .await
! 193     }
  194     #[activity(name = "strava_delete_source")]
! 195     pub async fn delete_source(
! 196         self: Arc<Self>,
! 197         ctx: ActivityContext,
! 198         input: ActivityChange,
! 199     ) -> Result<ActivityChange, ActivityError> {
! 200         run(
! 201             ctx,
! 202             input.event.clone(),
! 203             self.operations.delete_source(input),
! 204         )
! 205         .await
! 206     }
  207     #[activity(name = "strava_rebuild_activity")]
! 208     pub async fn rebuild_activity(
! 209         self: Arc<Self>,
! 210         ctx: ActivityContext,
! 211         input: ActivityChange,
! 212     ) -> Result<ActivityChange, ActivityError> {
! 213         run(
! 214             ctx,
! 215             input.event.clone(),
! 216             self.operations.rebuild_activity(input),
! 217         )
! 218         .await
! 219     }
  220     #[activity(name = "strava_rebuild_fitness")]
! 221     pub async fn rebuild_fitness(
! 222         self: Arc<Self>,
! 223         ctx: ActivityContext,
! 224         input: ActivityChange,
! 225     ) -> Result<ActivityChange, ActivityError> {
! 226         run(
! 227             ctx,
! 228             input.event.clone(),
! 229             self.operations.rebuild_fitness(input),
! 230         )
! 231         .await
! 232     }
  233     #[activity(name = "strava_rebuild_segments")]
! 234     pub async fn rebuild_segments(
! 235         self: Arc<Self>,
! 236         ctx: ActivityContext,
! 237         input: ActivityChange,
! 238     ) -> Result<ActivityChange, ActivityError> {
! 239         run(
! 240             ctx,
! 241             input.event.clone(),
! 242             self.operations.rebuild_segments(input),
! 243         )
! 244         .await
! 245     }
  246     #[activity(name = "strava_publish_heatmap")]
! 247     pub async fn publish_heatmap(
! 248         self: Arc<Self>,
! 249         ctx: ActivityContext,
! 250         input: ActivityChange,
! 251     ) -> Result<ActivityChange, ActivityError> {
! 252         run(
! 253             ctx,
! 254             input.event.clone(),
! 255             self.operations.publish_heatmap(input),
! 256         )
! 257         .await
! 258     }
  259     #[activity(name = "strava_complete_activity")]
! 260     pub async fn complete_activity(
! 261         self: Arc<Self>,
! 262         ctx: ActivityContext,
! 263         input: ActivityChange,
! 264     ) -> Result<ActivityChange, ActivityError> {
! 265         run(
! 266             ctx,
! 267             input.event.clone(),
! 268             self.operations.complete_activity(input),
! 269         )
! 270         .await
! 271     }
  272     #[activity(name = "strava_list_activity_page")]
! 273     pub async fn list_activity_page(
! 274         self: Arc<Self>,
! 275         ctx: ActivityContext,
! 276         input: PageInput,
! 277     ) -> Result<ActivityPage, ActivityError> {
! 278         run(
! 279             ctx,
! 280             input.collection.event.clone(),
! 281             self.operations.list_page(input),
! 282         )
! 283         .await
! 284     }
  285 }
  286 
  287 async fn run<T>(
  288     ctx: ActivityContext,
```


---


```
  297         StravaOperation::Create | StravaOperation::Update | StravaOperation::Upsert => {
  298             UpsertActivity::run.name()
  299         }
  300         StravaOperation::Delete => DeleteActivity::run.name(),
! 301         StravaOperation::Sync => SyncActivities::run.name(),
! 302         StravaOperation::Backfill => BackfillActivities::run.name(),
! 303         StravaOperation::Disconnect => DisconnectStrava::run.name(),
  304     };
  305     if info.workflow_type.as_deref() != Some(expected_workflow) {
  306         return Err(ApplicationFailure::non_retryable(std::io::Error::other(
  307             "Strava event operation does not match its Temporal workflow",
```


---


```
  343             };
  344             match &result {
  345                 Ok(_) => logging.attempt(&info.activity_type, AttemptStatus::Completed),
  346                 Err(ActivityError::Cancelled { .. }) => {
! 347                     logging.attempt(&info.activity_type, AttemptStatus::Cancelled)
  348                 }
  349                 Err(_) => logging.attempt(&info.activity_type, AttemptStatus::Failed),
  350             }
  351             result
```


---


```
  354 }
  355 
  356 fn activity_error(error: WorkflowError) -> ActivityError {
  357     let delay = error.retry_at.map(|at| {
! 358         (at - chrono::Utc::now())
! 359             .to_std()
! 360             .unwrap_or(Duration::from_secs(1))
! 361             .max(Duration::from_secs(1))
! 362     });
  363     let retryable = error.retry_at.is_some() || error.status.as_u16() == 429;
  364     let terminal = error.status.is_client_error() && !retryable;
  365     ApplicationFailure::builder(error)
  366         .type_name("StravaOperationFailure".to_owned())
```


---



## bike-rs/strava-temporal/src/client&#46;rs

```
  19     pub namespace: String,
  20     pub task_queue: String,
  21 }
  22 impl TemporalConfig {
! 23     pub fn from_env() -> Self {
  24         Self {
! 25             address: std::env::var("TEMPORAL_ADDRESS")
! 26                 .unwrap_or_else(|_| "http://temporal:7233".into()),
! 27             namespace: std::env::var("TEMPORAL_NAMESPACE").unwrap_or_else(|_| "bike".into()),
! 28             task_queue: std::env::var("TEMPORAL_TASK_QUEUE")
! 29                 .unwrap_or_else(|_| "bike-strava".into()),
  30         }
! 31     }
  32 }
  33 
! 34 pub async fn connect(config: &TemporalConfig) -> Result<Client, Error> {
! 35     let options = ConnectionOptions::new(config.address.parse::<temporalio_client::Url>()?)
! 36         .identity("bike-strava-temporal")
! 37         .connect_timeout(Duration::from_secs(10))
! 38         .build();
! 39     Ok(Client::new(
! 40         Connection::connect(options).await?,
! 41         ClientOptions::new(config.namespace.clone()).build(),
! 42     )?)
! 43 }
  44 
! 45 pub async fn ensure_schedule(client: &Client, config: &TemporalConfig) -> Result<(), Error> {
  46     use temporalio_client::schedules::{
  47         CreateScheduleOptions, ScheduleAction, ScheduleError, ScheduleIntervalSpec,
  48         ScheduleOverlapPolicy, ScheduleSpec,
  49     };
! 50     let options = CreateScheduleOptions::builder()
! 51         .action(ScheduleAction::start_workflow(
  52             ScheduledSync::run,
! 53             None,
! 54             config.task_queue.clone(),
  55             "bike-strava-scan",
  56         ))
! 57         .spec(
! 58             ScheduleSpec::builder()
! 59                 .intervals(vec![ScheduleIntervalSpec::from(Duration::from_secs(
! 60                     6 * 3600,
  61                 ))])
! 62                 .build(),
  63         )
! 64         .overlap_policy(ScheduleOverlapPolicy::Skip)
! 65         .build();
! 66     match client.create_schedule("bike-strava-sync", options).await {
! 67         Ok(_) => Ok(()),
! 68         Err(ScheduleError::Rpc(status)) if status.code() == tonic::Code::AlreadyExists => Ok(()),
! 69         Err(error) => Err(error.into()),
  70     }
! 71 }
  72 
! 73 pub fn worker(
! 74     runtime: &Runtime,
! 75     client: Client,
! 76     config: &TemporalConfig,
! 77     db: DatabaseConnection,
! 78 ) -> Result<Worker, Error> {
! 79     let options = WorkerOptions::new(config.task_queue.clone())
! 80         .register_workflow::<CreateActivity>()?
! 81         .register_workflow::<UpdateActivity>()?
! 82         .register_workflow::<UpsertActivity>()?
! 83         .register_workflow::<DeleteActivity>()?
! 84         .register_workflow::<SyncActivities>()?
! 85         .register_workflow::<BackfillActivities>()?
! 86         .register_workflow::<DisconnectStrava>()?
! 87         .register_workflow::<ScheduledSync>()?
! 88         .register_activities(StravaActivities::new(db))
! 89         .graceful_shutdown_period(Duration::from_secs(10))
! 90         .build();
! 91     Ok(Worker::new(runtime, client, options)?)
! 92 }
  93 
  94 pub fn search_attributes(event: &StravaEvent) -> SearchAttributes {
  95     let mut values = vec![
  96         SearchAttributeKey::<String>::keyword("BikeCustomerId")
```


---


```
  111     }
  112     SearchAttributes::new(values)
  113 }
  114 
! 115 pub fn start_options(config: &TemporalConfig, event: &StravaEvent) -> WorkflowStartOptions {
! 116     WorkflowStartOptions::new(config.task_queue.clone(), event.workflow_id())
! 117         .search_attributes(search_attributes(event))
! 118         .id_reuse_policy(WorkflowIdReusePolicy::RejectDuplicate)
! 119         .build()
! 120 }
  121 
! 122 pub async fn start_event(
! 123     client: &Client,
! 124     config: &TemporalConfig,
! 125     event: StravaEvent,
! 126 ) -> Result<Option<String>, Error> {
! 127     let options = start_options(config, &event);
! 128     let result = match event.operation {
! 129         StravaOperation::Create => client
! 130             .start_workflow(CreateActivity::run, event, options)
! 131             .await
! 132             .map(|handle| handle.run_id().map(str::to_owned)),
! 133         StravaOperation::Update => client
! 134             .start_workflow(UpdateActivity::run, event, options)
! 135             .await
! 136             .map(|handle| handle.run_id().map(str::to_owned)),
! 137         StravaOperation::Upsert => client
! 138             .start_workflow(UpsertActivity::run, event, options)
! 139             .await
! 140             .map(|handle| handle.run_id().map(str::to_owned)),
! 141         StravaOperation::Delete => client
! 142             .start_workflow(DeleteActivity::run, event, options)
! 143             .await
! 144             .map(|handle| handle.run_id().map(str::to_owned)),
! 145         StravaOperation::Sync => client
! 146             .start_workflow(SyncActivities::run, event, options)
! 147             .await
! 148             .map(|handle| handle.run_id().map(str::to_owned)),
! 149         StravaOperation::Backfill => client
! 150             .start_workflow(BackfillActivities::run, event, options)
! 151             .await
! 152             .map(|handle| handle.run_id().map(str::to_owned)),
! 153         StravaOperation::Disconnect => client
! 154             .start_workflow(DisconnectStrava::run, event, options)
! 155             .await
! 156             .map(|handle| handle.run_id().map(str::to_owned)),
  157     };
! 158     match result {
! 159         Ok(run_id)
! 160         | Err(temporalio_client::errors::WorkflowStartError::AlreadyStarted { run_id, .. }) => {
! 161             Ok(run_id)
  162         }
! 163         Err(error) => Err(error.into()),
  164     }
! 165 }
  166 
! 167 pub async fn dispatch_events(
! 168     client: Client,
! 169     config: TemporalConfig,
! 170     db: DatabaseConnection,
! 171 ) -> Result<(), Error> {
  172     loop {
! 173         for row in bike_core::entities::strava_events::Model::pending(&db).await? {
! 174             dispatch_event(&client, &config, &db, row).await?;
  175         }
! 176         tokio::time::sleep(Duration::from_secs(1)).await;
  177     }
! 178 }
  179 
! 180 async fn dispatch_event(
! 181     client: &Client,
! 182     config: &TemporalConfig,
! 183     db: &DatabaseConnection,
! 184     row: bike_core::entities::strava_events::Model,
! 185 ) -> Result<(), Error> {
! 186     let span = handoff_span(&row, config);
! 187     async {
! 188         let result = match serde_json::from_value(row.event.clone()) {
! 189             Ok(event) => start_event(client, config, event).await,
! 190             Err(error) => Err(error.into()),
  191         };
! 192         match result {
! 193             Ok(run_id) => {
! 194                 tracing::info!(temporal_run_id = run_id, "Strava event handed to Temporal");
! 195                 row.submitted(db).await?;
  196             }
! 197             Err(error) => {
! 198                 tracing::error!(error = %error, "Temporal handoff will retry");
! 199                 row.retry_handoff(db, error.to_string()).await?;
  200             }
  201         }
! 202         Ok(())
! 203     }
! 204     .instrument(span)
! 205     .await
! 206 }
  207 
  208 pub(crate) fn handoff_span(
  209     row: &bike_core::entities::strava_events::Model,
  210     config: &TemporalConfig,
```


---



## bike-rs/strava-temporal/src/metrics&#46;rs

```
   7     pub bound_addr: std::net::SocketAddr,
   8     pub abort_handle: tokio::task::AbortHandle,
   9 }
  10 
! 11 pub fn start(
! 12     address: std::net::SocketAddr,
! 13 ) -> Result<(RuntimeOptions, MetricsServer), crate::client::Error> {
! 14     let server = start_prometheus_metric_exporter(
! 15         PrometheusExporterOptions::builder()
! 16             .socket_addr(address)
! 17             .counters_total_suffix(true)
! 18             .use_seconds_for_durations(true)
! 19             .unit_suffix(true)
! 20             .build(),
! 21     )?;
! 22     let meter: std::sync::Arc<dyn temporalio_common::telemetry::metrics::CoreMeter> = server.meter;
! 23     let options = RuntimeOptions::builder()
! 24         .telemetry_options(TelemetryOptions::builder().metrics(meter).build())
! 25         .build()?;
! 26     Ok((
! 27         options,
! 28         MetricsServer {
! 29             bound_addr: server.bound_addr,
! 30             abort_handle: server.abort_handle,
! 31         },
! 32     ))
! 33 }
```


---



## bike-rs/strava-temporal/src/workflows/backfill&#46;rs

```
  28     ) -> WorkflowResult<CollectionResult> {
  29         let input = run(ctx, CollectionWindow, event).await?;
  30         let result = match collection::upsert_pages(ctx, input).await? {
  31             collection::CollectionStep::Completed(result) => result,
! 32             collection::CollectionStep::Continue(event) => {
! 33                 return ctx
! 34                     .continue_as_new(event, Default::default())
! 35                     .map(|never| match never {})
  36             }
  37         };
  38         run(ctx, CompleteBackfill, result).await
  39     }
```


---



## bike-rs/strava-temporal/src/workflows/disconnect&#46;rs

```
  48                 .build();
  49             options::execute_child_workflow(ctx, DeleteActivity::run, child, options).await?;
  50         }
  51         if more {
! 52             return ctx
! 53                 .continue_as_new(event, Default::default())
! 54                 .map(|never| match never {});
  55         }
  56         let event = run(ctx, RevokeConnection, event).await?;
  57         run(ctx, RemoveConnection, event).await
  58     }
```


---



## bike-rs/strava-temporal/src/workflows/logging&#46;rs

```
  181     let (status, error) = match result {
  182         Ok(_) => (WorkflowStatus::Completed, None),
  183         Err(WorkflowTermination::Cancelled { .. }) => (WorkflowStatus::Cancelled, None),
  184         Err(WorkflowTermination::ContinueAsNew(_)) => (WorkflowStatus::Continued, None),
! 185         Err(WorkflowTermination::Evicted) => return,
  186         Err(error) => (WorkflowStatus::Failed, Some(error.to_string())),
  187     };
  188     record(
  189         ctx,
```


---



## bike-rs/strava-temporal/src/workflows/options&#46;rs

```
  30         .start_child_workflow(definition, event, options)
  31         .await
  32         .map_err(WorkflowTermination::from);
  33     if result.is_err() {
! 34         log.finished(ctx, &result);
  35     }
  36     let child = result?;
  37     log.started(ctx, &child.run_id);
  38     Ok((child, log))
```


---


