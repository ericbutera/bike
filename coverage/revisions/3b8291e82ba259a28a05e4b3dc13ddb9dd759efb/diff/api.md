# Diff Coverage
## Diff: e401699acdc0b5280a64d13d689fc3efd11cdd43...HEAD, staged, unstaged and untracked changes

- bike-rs/api/src/activity_import_history&#46;rs (100%)
- bike-rs/api/src/activity_reprocessing&#46;rs (100%)
- bike-rs/api/src/app_error&#46;rs (0.0%): Missing lines 232
- bike-rs/api/src/controllers/activities&#46;rs (97.4%): Missing lines 859
- bike-rs/api/src/controllers/activity_import_history&#46;rs (100%)
- bike-rs/api/src/controllers/activity_imports&#46;rs (85.1%): Missing lines 266-274,279
- bike-rs/api/src/controllers/admin&#46;rs (98.3%): Missing lines 604
- bike-rs/api/src/controllers/strava&#46;rs (100%)
- bike-rs/api/src/extractors/strava&#46;rs (100%)
- bike-rs/api/src/lib&#46;rs (0.0%): Missing lines 148

## Summary

- **Total**: 195 lines
- **Missing**: 14 lines
- **Coverage**: 92.82%



## bike-rs/api/src/app_error&#46;rs

```
  228     }
  229 }
  230 
  231 impl From<bike_core::jobs::TaskError> for AppError {
! 232     fn from(error: bike_core::jobs::TaskError) -> Self {
  233         Self::internal(error.to_string())
  234     }
  235 }
```


---



## bike-rs/api/src/controllers/activities&#46;rs

```
  855     let activity = activities::Model::find_owned(&state.db, id, user.id)
  856         .await?
  857         .ok_or_else(|| AppError::not_found("Activity not found"))?;
  858     if activity.activity_import_id.is_none() {
! 859         return Err(AppError::bad_request("Only uploaded activities can be regenerated"));
  860     }
  861     let job = state
  862         .tasks
  863         .reprocess_activity_import(activity.id)
```


---



## bike-rs/api/src/controllers/activity_imports&#46;rs

```
  262                 .mark_processing(&state.db, "raw_stored", None, Utc::now())
  263                 .await?
  264         }
  265         None => {
! 266             store_activity_upload_import(
! 267                 &state.db,
! 268                 &state.uploads_dir,
! 269                 &user_storage_key,
! 270                 user.id,
! 271                 upload,
! 272                 "manual_upload",
! 273             )
! 274             .await?
  275         }
  276     };
  277 
  278     if let Err(failure) = state.tasks.process_activity_import(user.id, import.id).await {
! 279         let error = AppError::internal(format!("Failed to queue activity import: {failure}"));
  280         let workflow_error = bike_core::workflow_error::WorkflowError::internal(error.message.clone());
  281         mark_activity_import_failed(&state.db, &import, &import.processing_stage, &workflow_error)
  282             .await
  283             .map_err(AppError::from)?;
```


---



## bike-rs/api/src/controllers/admin&#46;rs

```
  600                 }),
  601             )
  602         });
  603 
! 604     result.map_err(Into::into)
  605 }
  606 
  607 #[utoipa::path(
  608     post,
```


---



## bike-rs/api/src/lib&#46;rs

```
  144 }
  145 
  146 async fn request_id_response_header(request: Request<Body>, next: Next) -> Response {
  147     let request_id = request.headers().get(&REQUEST_ID_HEADER).cloned();
! 148     let mut response = next.run(request).await;
  149 
  150     if let Some(request_id) = request_id {
  151         response.headers_mut().insert(&REQUEST_ID_HEADER, request_id);
  152     }
```


---


