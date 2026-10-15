# Diff Coverage
## Diff: 87f8fa643022dbf795def001041f74ffd2c02e34...HEAD, staged, unstaged and untracked changes

- bike-rs/api/src/controllers/activities&#46;rs (95.2%): Missing lines 925
- bike-rs/api/src/controllers/activity_imports&#46;rs (100%)
- bike-rs/api/src/controllers/admin&#46;rs (8.3%): Missing lines 241,247,310,540,578,632,681,747,818,952,956
- bike-rs/api/src/controllers/fitness&#46;rs (100%)
- bike-rs/api/src/controllers/segments&#46;rs (84.4%): Missing lines 658,661,677-678,827
- bike-rs/api/src/controllers/user_preferences&#46;rs (100%)
- bike-rs/api/src/metrics&#46;rs (100%)

## Summary

- **Total**: 93 lines
- **Missing**: 17 lines
- **Coverage**: 81.72%



## bike-rs/api/src/controllers/activities&#46;rs

```
  921         .ok_or_else(|| AppError::not_found("Activity not found"))?;
  922     let activity_import_id = activity
  923         .activity_import_id
  924         .ok_or_else(|| AppError::bad_request("Only uploaded activities can be regenerated"))?;
! 925     let activity_import =
  926         activity_imports::Entity::find_owned(&state.db, user.id, activity_import_id)
  927             .await?
  928             .ok_or_else(|| AppError::not_found("Activity import not found"))?;
  929     let reprocessed = reprocess_activity_from_import(
```


---



## bike-rs/api/src/controllers/admin&#46;rs

```
  237     _admin: AdminUserContext<AppStorage>,
  238     State(state): State<Arc<AppStorage>>,
  239     Path(activity_id): Path<i32>,
  240 ) -> Result<Json<AdminActivityImportTraceResponse>, AppError> {
! 241     let activity = activities::Model::find_by_id(&state.db, activity_id)
  242         .await?
  243         .ok_or_else(|| AppError::not_found("Activity not found"))?;
  244     let Some(import_id) = activity.activity_import_id else {
  245         return Ok(Json(AdminActivityImportTraceResponse { trace: None }));
```


---


```
  243         .ok_or_else(|| AppError::not_found("Activity not found"))?;
  244     let Some(import_id) = activity.activity_import_id else {
  245         return Ok(Json(AdminActivityImportTraceResponse { trace: None }));
  246     };
! 247     let import = activity_imports::Entity::find_for_administration(&state.db, import_id)
  248         .await?
  249         .ok_or_else(|| AppError::not_found("Activity import not found"))?;
  250     let trace = build_admin_activity_import_trace(&state.db, import, Some(activity)).await?;
```


---


```
  306 async fn load_activity_for_admin_import_trace(
  307     db: &DatabaseConnection,
  308     import: &activity_imports::Model,
  309 ) -> Result<Option<activities::Model>, AppError> {
! 310     Ok(activities::Model::for_import_inspection(db, import).await?)
  311 }
  312 
  313 #[derive(Debug, Serialize, ToSchema)]
  314 #[schema(example = json!({"user_count": 12,"segment_count": 35,"fitness_task_count": 12,"segment_task_count": 2,"total_tasks_enqueued": 14,"segment_chunk_size": 20}))]
```


---


```
  536     _admin: AdminUserContext<AppStorage>,
  537     State(state): State<Arc<AppStorage>>,
  538     Json(request): Json<BackfillUserXcTrainingRequest>,
  539 ) -> Result<(StatusCode, Json<ReprocessUserActivityImportsResponse>), AppError> {
! 540     users::Model::find_optional(&state.db, request.user_id)
  541         .await?
  542         .ok_or_else(|| AppError::not_found(format!("User {} was not found", request.user_id)))?;
  543 
  544     let (status, message) =
```


---


```
  574     _admin: AdminUserContext<AppStorage>,
  575     State(state): State<Arc<AppStorage>>,
  576     Json(request): Json<RegenerateUserSegmentsRequest>,
  577 ) -> Result<(StatusCode, Json<RegenerateUserSegmentsResponse>), AppError> {
! 578     users::Model::find_optional(&state.db, request.user_id)
  579         .await?
  580         .ok_or_else(|| AppError::not_found(format!("User {} was not found", request.user_id)))?;
  581 
  582     acquire_user_activity_import_lock(
```


---


```
  628     _admin: AdminUserContext<AppStorage>,
  629     State(state): State<Arc<AppStorage>>,
  630     Json(request): Json<RegenerateSegmentEffortsRequest>,
  631 ) -> Result<(StatusCode, Json<RegenerateSegmentEffortsResponse>), AppError> {
! 632     segments::Model::find_by_id(&state.db, request.segment_id)
  633         .await?
  634         .ok_or_else(|| {
  635             AppError::not_found(format!("Segment {} was not found", request.segment_id))
  636         })?;
```


---


```
  677     _admin: AdminUserContext<AppStorage>,
  678     State(state): State<Arc<AppStorage>>,
  679     Json(request): Json<ReprocessUserActivityImportsRequest>,
  680 ) -> Result<(StatusCode, Json<ReprocessUserActivityImportsResponse>), AppError> {
! 681     users::Model::find_optional(&state.db, request.user_id)
  682         .await?
  683         .ok_or_else(|| AppError::not_found(format!("User {} was not found", request.user_id)))?;
  684 
  685     acquire_user_activity_import_lock(
```


---


```
  743     _admin: AdminUserContext<AppStorage>,
  744     State(state): State<Arc<AppStorage>>,
  745     Json(request): Json<ReprocessActivityImportRequest>,
  746 ) -> Result<(StatusCode, Json<ReprocessActivityImportResponse>), AppError> {
! 747     let activity = activities::Model::find_by_id(&state.db, request.activity_id)
  748         .await?
  749         .ok_or_else(|| {
  750             AppError::not_found(format!("Activity {} was not found", request.activity_id))
  751         })?;
```


---


```
  814     _admin: AdminUserContext<AppStorage>,
  815     State(state): State<Arc<AppStorage>>,
  816     Json(request): Json<CleanupUserDuplicateActivitiesRequest>,
  817 ) -> Result<(StatusCode, Json<CleanupUserDuplicateActivitiesResponse>), AppError> {
! 818     users::Model::find_optional(&state.db, request.user_id)
  819         .await?
  820         .ok_or_else(|| AppError::not_found(format!("User {} was not found", request.user_id)))?;
  821 
  822     acquire_user_activity_import_lock(
```


---


```
  948     }
  949 }
  950 
  951 async fn load_user_ids_with_activities(state: &Arc<AppStorage>) -> Result<Vec<i32>, AppError> {
! 952     Ok(activities::Model::owner_ids(&state.db).await?)
  953 }
  954 
  955 async fn load_segment_ids(state: &Arc<AppStorage>) -> Result<Vec<i32>, AppError> {
! 956     Ok(segments::Model::all_ids(&state.db).await?)
  957 }
  958 
  959 async fn enqueue_segment_backfill_tasks(
  960     state: &Arc<AppStorage>,
```


---



## bike-rs/api/src/controllers/segments&#46;rs

```
  654     UserContext { user, .. }: UserContext<AppStorage>,
  655     State(state): State<Arc<AppStorage>>,
  656     Json(payload): Json<CreateSegmentFromActivityRequest>,
  657 ) -> Result<Json<SegmentResponse>, AppError> {
! 658     let segment = segments::Model::find_owned(&state.db, id, user.id)
  659         .await?
  660         .ok_or_else(|| AppError::not_found("Segment not found"))?;
! 661     let activity = activities::Model::find_owned(&state.db, payload.activity_id, user.id)
  662         .await?
  663         .ok_or_else(|| AppError::not_found("Activity not found"))?;
  664     let title = normalize_segment_title(&payload.title)?;
  665     let activity_route_points =
```


---


```
  673         .last()
  674         .and_then(|point| point.distance_meters);
  675 
  676     if let Some(existing_segment) =
! 677         segments::Model::find_duplicate(&state.db, user.id, distance_meters, &segment_route_points)
! 678             .await?
  679     {
  680         if existing_segment.id != segment.id {
  681             return Err(AppError::validation_field(
  682                 "activity_id",
```


---


```
  823             "Segment imports require GPX or TCX route data with coordinates",
  824         ));
  825     }
  826 
! 827     if let Some(existing_segment) = segments::Model::find_duplicate(
  828         &state.db,
  829         user.id,
  830         segment_summary.distance_meters,
  831         &segment_detail.route_points,
```


---


