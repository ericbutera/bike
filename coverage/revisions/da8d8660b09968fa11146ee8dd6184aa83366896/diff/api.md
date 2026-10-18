# Diff Coverage
## Diff: 394f518cc12ab5dd623b454f093dad77c20d3398...HEAD, staged, unstaged and untracked changes

- bike-rs/api/src/admin_task_history&#46;rs (88.9%): Missing lines 16,24,36
- bike-rs/api/src/admin_tasks&#46;rs (99.0%): Missing lines 128
- bike-rs/api/src/admin_tasks/history&#46;rs (100%)
- bike-rs/api/src/admin_tasks/provider&#46;rs (98.3%): Missing lines 51
- bike-rs/api/src/controllers/admin_task_history&#46;rs (100%)
- bike-rs/api/src/controllers/admin_tasks&#46;rs (68.8%): Missing lines 34-43
- bike-rs/api/src/controllers/mod&#46;rs (100%)

## Summary

- **Total**: 312 lines
- **Missing**: 15 lines
- **Coverage**: 95.19%



## bike-rs/api/src/admin_task_history&#46;rs

```
  12 
  13 pub async fn list(state: &AppStorage, query: HistoryQuery) -> Result<TaskExecutionPage, AppError> {
  14     let token = query.page_token.unwrap_or_default();
  15     if token.len() > 32768 {
! 16         return Err(AppError::bad_request("Invalid history page token"));
  17     }
  18     let token = hex::decode(token).map_err(|_| AppError::bad_request("Invalid history page token"))?;
  19     read(state.tasks.execution_history(query.status, token)).await
  20 }
```


---


```
  20 }
  21 
  22 pub async fn input(state: &AppStorage, workflow_id: String, run_id: String) -> Result<TaskExecutionInput, AppError> {
  23     if workflow_id.is_empty() || workflow_id.len() > 1000 {
! 24         return Err(AppError::bad_request("A workflow ID is required"));
  25     }
  26     uuid::Uuid::parse_str(&run_id).map_err(|_| AppError::bad_request("A valid run ID is required"))?;
  27     read(state.tasks.execution_input(workflow_id, run_id)).await
  28 }
```


---


```
  32         Ok(Ok(value)) => Ok(value),
  33         error => {
  34             if let Ok(Err(error)) = error {
  35                 tracing::error!(%error, "Temporal history read failed");
! 36             }
  37             Err(AppError {
  38                 status: axum::http::StatusCode::BAD_GATEWAY,
  39                 ..AppError::internal("Task history is unavailable. Try again or open Temporal.")
  40             })
```


---



## bike-rs/api/src/admin_tasks&#46;rs

```
  124 }
  125 
  126 fn validate_command(command_id: Uuid) -> Result<(), AppError> {
  127     if command_id.is_nil() {
! 128         return Err(AppError::bad_request("A non-empty command_id is required"));
  129     }
  130     Ok(())
  131 }
```


---



## bike-rs/api/src/admin_tasks/provider&#46;rs

```
  47 ) -> Result<JobReference, AppError> {
  48     let operation = match action {
  49         UserAction::StravaSync => ManualOperation::Sync,
  50         UserAction::StravaBackfill { after, before } => ManualOperation::Backfill { after, before },
! 51         _ => return Err(AppError::bad_request("Unsupported Strava action")),
  52     };
  53     let connection = connection(state, user.id).await?;
  54     events::request_manual(
  55         &state.strava_intake,
```


---



## bike-rs/api/src/controllers/admin_tasks&#46;rs

```
  30         (status = 422, description = "Unknown task or malformed task parameters"),
  31         (status = 500, description = "Submission not confirmed; retry the same command ID", body = ApiErrorResponse)
  32     ), tag = "admin", security(("bearer_auth" = []))
  33 )]
! 34 pub async fn activity_task(
! 35     admin: AdminUserContext<AppStorage>,
! 36     State(state): State<Arc<AppStorage>>,
! 37     Path(id): Path<i32>,
! 38     headers: HeaderMap,
! 39     Json(request): Json<ActivityTaskRequest>,
! 40 ) -> Result<(StatusCode, Json<AcceptedTask>), AppError> {
! 41     let result = admin_tasks::activity(&state, context(admin.user.id, &headers), id, request).await?;
! 42     Ok((StatusCode::ACCEPTED, Json(result)))
! 43 }
  44 
  45 #[utoipa::path(
  46     post, path = "/admin/users/{id}/tasks",
  47     params(("id" = i32, Path, description = "Bike user ID")),
```


---


