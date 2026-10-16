# Diff Coverage
## Diff: 7e43ab6d2d5b70d305a52e5f12bf704593c5fad7...HEAD, staged, unstaged and untracked changes

- bike-rs/api/src/activity_import_history&#46;rs (0.0%): Missing lines 66,69,74-76,134,138,146,154-155,157,168
- bike-rs/api/src/activity_reprocessing&#46;rs (95.5%): Missing lines 52-53
- bike-rs/api/src/controllers/activity_import_history&#46;rs (33.3%): Missing lines 22-29,62-63,75,77,90,93
- bike-rs/api/src/controllers/admin&#46;rs (84.7%): Missing lines 775,777,780-786
- bike-rs/api/src/controllers/segments&#46;rs (100%)
- bike-rs/api/src/storage&#46;rs (50.0%): Missing lines 66-68

## Summary

- **Total**: 143 lines
- **Missing**: 40 lines
- **Coverage**: 72.03%



## bike-rs/api/src/activity_import_history&#46;rs

```
  62 
  63 pub async fn history(
  64     db: &DatabaseConnection,
  65     user_id: i32,
! 66     filter: HistoryFilter,
  67 ) -> Result<ImportHistoryResponse, AppError> {
  68     recover_owned_imports(db, user_id).await?;
! 69     let page = filter.page.unwrap_or(1).max(1);
  70     let (items, total) = activity_imports::Entity::history(
  71         db,
  72         user_id,
  73         page,
! 74         filter.source.as_deref().filter(|value| !value.is_empty()),
! 75         filter.status.as_deref().filter(|value| !value.is_empty()),
! 76         filter.archive_job_id,
  77     )
  78     .await?;
  79     Ok(ImportHistoryResponse {
  80         items: items
```


---


```
  130 pub async fn admin_replay(
  131     db: &DatabaseConnection,
  132     uploads_dir: &str,
  133     import_id: i32,
! 134     command: ReplayCommand,
  135     queue: bool,
  136 ) -> Result<ReplayResponse, AppError> {
  137     let import = admin_import(db, import_id).await?;
! 138     replay(db, uploads_dir, import.user_id, import_id, command, queue).await
  139 }
  140 
  141 pub async fn replay(
  142     db: &DatabaseConnection,
```


---


```
  142     db: &DatabaseConnection,
  143     uploads_dir: &str,
  144     user_id: i32,
  145     import_id: i32,
! 146     command: ReplayCommand,
  147     queue: bool,
  148 ) -> Result<ReplayResponse, AppError> {
  149     let import = activity_imports::Entity::find_owned(db, user_id, import_id)
  150         .await?
```


---


```
  150         .await?
  151         .ok_or_else(|| AppError::not_found("Activity import not found"))?;
  152     let (plan, attempt_id) = if queue {
  153         let expected =
! 154             plan_activity_import_replay(db, uploads_dir, &import, &command.stage).await?;
! 155         if command.expected_start_stage.as_ref().is_some_and(|stage| {
  156             stage != &expected.start_stage
! 157                 || command.expected_reused_attempt_id != expected.reused_attempt_id
  158         }) {
  159             return Err(AppError::conflict(
  160                 "Replay prerequisites changed; review the updated start stage and try again",
  161             ));
```


---


```
  164             queue_expected_activity_import_replay(db, uploads_dir, &import, &expected).await?;
  165         (plan, Some(id))
  166     } else {
  167         (
! 168             plan_activity_import_replay(db, uploads_dir, &import, &command.stage).await?,
  169             None,
  170         )
  171     };
  172     Ok(ReplayResponse {
```


---



## bike-rs/api/src/activity_reprocessing&#46;rs

```
  48             .reprocess_activity_import(input, cause),
  49     )
  50     .await
  51     .map_err(|_| {
! 52         AppError::internal("Temporal acceptance timed out; retry with the same command_id")
! 53     })?
  54     .map_err(|error| {
  55         AppError::internal(format!(
  56             "Temporal did not acknowledge reprocessing: {error}"
  57         ))
```


---



## bike-rs/api/src/controllers/activity_import_history&#46;rs

```
  18     pub archive_job_id: Option<i32>,
  19 }
  20 
  21 impl From<HistoryQuery> for service::HistoryFilter {
! 22     fn from(query: HistoryQuery) -> Self {
! 23         Self {
! 24             page: query.page,
! 25             source: query.source,
! 26             status: query.status,
! 27             archive_job_id: query.archive_job_id,
! 28         }
! 29     }
  30 }
  31 
  32 #[derive(Debug, Deserialize, ToSchema, IntoParams)]
  33 #[into_params(parameter_in = Query)]
```


---


```
  58     let owner_id = query.user_id.unwrap_or(user.id);
  59     if owner_id <= 0 {
  60         return Err(AppError::bad_request("User ID must be positive"));
  61     }
! 62     let filter = service::HistoryFilter::from(query);
! 63     Ok(Json(service::history(&state.db, owner_id, filter).await?))
  64 }
  65 
  66 #[utoipa::path(get, path = "/admin/activity-imports/{id}/replay", params(("id" = i32, Path), ReplayRequest),
  67     responses((status = 200, body = ReplayResponse), (status = 400, body = ApiErrorResponse), (status = 401), (status = 403), (status = 404, body = ApiErrorResponse)),
```


---


```
  71     State(state): State<Arc<AppStorage>>,
  72     Path(id): Path<i32>,
  73     Query(request): Query<ReplayRequest>,
  74 ) -> Result<Json<ReplayResponse>, AppError> {
! 75     let command = service::ReplayCommand::from(request);
  76     Ok(Json(
! 77         service::admin_replay(&state.db, &state.uploads_dir, id, command, false).await?,
  78     ))
  79 }
  80 
  81 #[utoipa::path(post, path = "/admin/activity-imports/{id}/replay", params(("id" = i32, Path)), request_body = ReplayRequest,
```


---


```
  86     State(state): State<Arc<AppStorage>>,
  87     Path(id): Path<i32>,
  88     Json(request): Json<ReplayRequest>,
  89 ) -> Result<(axum::http::StatusCode, Json<ReplayResponse>), AppError> {
! 90     let command = service::ReplayCommand::from(request);
  91     Ok((
  92         axum::http::StatusCode::ACCEPTED,
! 93         Json(service::admin_replay(&state.db, &state.uploads_dir, id, command, true).await?),
  94     ))
  95 }
  96 
  97 #[cfg(test)]
```


---



## bike-rs/api/src/controllers/admin&#46;rs

```
  771     security(("bearer_auth" = [])),
  772     tag = "admin",
  773 )]
  774 pub async fn reprocess_activity_import(
! 775     AdminUserContext { user, .. }: AdminUserContext<AppStorage>,
  776     State(state): State<Arc<AppStorage>>,
! 777     headers: axum::http::HeaderMap,
  778     Json(request): Json<ReprocessActivityImportRequest>,
  779 ) -> Result<(StatusCode, Json<ReprocessActivityImportResponse>), AppError> {
! 780     let request_id = headers
! 781         .get("x-request-id")
! 782         .and_then(|value| value.to_str().ok())
! 783         .map(str::to_owned);
! 784     let command = crate::activity_reprocessing::ReprocessActivityCommand::from(request);
! 785     let result = crate::activity_reprocessing::submit(&state, user.id, request_id, command).await?;
! 786     Ok((StatusCode::ACCEPTED, Json(result.into())))
  787 }
  788 
  789 #[utoipa::path(
  790     post,
```


---



## bike-rs/api/src/storage&#46;rs

```
  62         let uploads_dir = Config::get().uploads_dir.clone();
  63         std::fs::create_dir_all(&uploads_dir).expect("Failed to create uploads directory");
  64 
  65         Self {
! 66             task_submission: std::sync::Arc::new(
! 67                 bike_temporal::submission::TaskSubmissionService::from_env(),
! 68             ),
  69             map_images: create_map_image_service(Config::get()).await,
  70             heatmaps: std::sync::Arc::new(bike_core::heatmaps::service::HeatmapService::default()),
  71             db,
  72             tasks,
```


---


