# Diff Coverage
## Diff: 83498dc9713c86f8deba96555290751c6712047a...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-core/src/activity_import_pipeline&#46;rs (100%)
- bike-rs/bike-core/src/activity_import_pipeline/stage_execution&#46;rs (100%)
- bike-rs/bike-core/src/activity_parser&#46;rs (100%)
- bike-rs/bike-core/src/activity_source_recovery&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/entities/background_tasks&#46;rs (76.5%): Missing lines 102-105,107,112-114
- bike-rs/bike-core/src/bin/recover-generated-imports&#46;rs (0.0%): Missing lines 144
- bike-rs/bike-core/src/entities/integration_events&#46;rs (97.7%): Missing lines 93-96,98
- bike-rs/bike-core/src/entities/strava_connections&#46;rs (95.0%): Missing lines 16
- bike-rs/bike-core/src/entities/strava_workflows&#46;rs (100%)
- bike-rs/bike-core/src/integration_events_service&#46;rs (100%)
- bike-rs/bike-core/src/jobs/adapter&#46;rs (100%)
- bike-rs/bike-core/src/strava/activity&#46;rs (100%)
- bike-rs/bike-core/src/strava/client&#46;rs (100%)
- bike-rs/bike-core/src/strava/connection&#46;rs (81.8%): Missing lines 414,417
- bike-rs/bike-core/src/strava/objects&#46;rs (100%)
- bike-rs/bike-core/src/strava/paths&#46;rs (100%)
- bike-rs/bike-core/src/strava/provider_payload&#46;rs (100%)
- bike-rs/bike-core/src/strava/retention/files&#46;rs (100%)
- bike-rs/bike-core/src/strava/retention/protection&#46;rs (100%)
- bike-rs/bike-core/src/strava/transport&#46;rs (100%)
- bike-rs/bike-core/src/strava/workflow/action&#46;rs (100%)
- bike-rs/bike-core/src/strava/workflow/activity&#46;rs (100%)
- bike-rs/bike-core/src/strava/workflow/child&#46;rs (93.3%): Missing lines 43,50-51
- bike-rs/bike-core/src/strava/workflow/collection&#46;rs (100%)
- bike-rs/bike-core/src/strava/workflow/context&#46;rs (100%)
- bike-rs/bike-core/src/strava/workflow/execution&#46;rs (93.3%): Missing lines 37,48-52,58,73
- bike-rs/bike-core/src/strava/workflow/import_objects&#46;rs (100%)
- bike-rs/bike-core/src/strava/workflow/inbox&#46;rs (96.9%): Missing lines 244,280
- bike-rs/bike-core/src/strava/workflow/mod&#46;rs (100%)
- bike-rs/bike-core/src/strava/workflow/revocation&#46;rs (100%)
- bike-rs/bike-core/src/strava/workflow/sources&#46;rs (100%)
- bike-rs/bike-core/src/strava/workflow/stage&#46;rs (100%)

## Summary

- **Total**: 843 lines
- **Missing**: 30 lines
- **Coverage**: 96.44%



## bike-rs/bike-core/src/background_jobs/entities/background_tasks&#46;rs

```
   98         let workflow: Expr = match db.get_database_backend() {
   99             sea_orm::DatabaseBackend::Postgres => Func::cust(Alias::new("jsonb_extract_path_text"))
  100                 .args([payload, Expr::val("data"), Expr::val("workflow_id")])
  101                 .into(),
! 102             sea_orm::DatabaseBackend::MySql => Func::cust(Alias::new("JSON_UNQUOTE"))
! 103                 .arg(
! 104                     Func::cust(Alias::new("JSON_EXTRACT"))
! 105                         .args([payload, Expr::val("$.data.workflow_id")]),
  106                 )
! 107                 .into(),
  108             sea_orm::DatabaseBackend::Sqlite => Func::cust(Alias::new("json_extract"))
  109                 .args([payload, Expr::val("$.data.workflow_id")])
  110                 .into(),
  111             _ => {
! 112                 return Err(DbErr::Custom(
! 113                     "Unsupported database for Strava task lookup".into(),
! 114                 ))
  115             }
  116         };
  117         Entity::find()
  118             .filter(Column::TaskType.is_in(Self::strava_workflow_task_types()))
```


---



## bike-rs/bike-core/src/bin/recover-generated-imports&#46;rs

```
  140     if run.refresh_recent && recent_gaps {
  141         let connection = bike_core::strava::load_connection(&db, run.user_id)
  142             .await?
  143             .ok_or("Strava connection was not found")?;
! 144         strava::workflow::queue_sync(&db, &connection, false).await?;
  145     }
  146     println!(
  147         "{}",
  148         serde_json::json!({"next_after_id": last_id, "applied": run.apply, "failed": failed, "recent_refresh_requested": run.refresh_recent && recent_gaps})
```


---



## bike-rs/bike-core/src/entities/integration_events&#46;rs

```
   89         match backend {
   90             DatabaseBackend::Sqlite => Func::cust(Alias::new("json_extract"))
   91                 .args([payload, Expr::val(format!("$.{key}"))])
   92                 .into(),
!  93             DatabaseBackend::MySql => Func::cust(Alias::new("JSON_UNQUOTE"))
!  94                 .arg(
!  95                     Func::cust(Alias::new("JSON_EXTRACT"))
!  96                         .args([payload, Expr::val(format!("$.{key}"))]),
   97                 )
!  98                 .into(),
   99             _ => PgExpr::cast_json_field(payload, key),
  100         }
  101     }
```


---



## bike-rs/bike-core/src/entities/strava_connections&#46;rs

```
  12         db: &DatabaseConnection,
  13         id: i32,
  14     ) -> Result<Option<DatabaseTransaction>, DbErr> {
  15         if db.get_database_backend() != sea_orm::DatabaseBackend::Postgres {
! 16             return Ok(None);
  17         }
  18         let lease = db.begin().await?;
  19         // PostgreSQL advisory locks need the database's native lock primitive.
  20         lease
```


---



## bike-rs/bike-core/src/strava/connection&#46;rs

```
  410         active_model.athlete_first_name = Set(token_response.athlete.firstname);
  411         active_model.athlete_last_name = Set(token_response.athlete.lastname);
  412         active_model.athlete_profile_medium_url = Set(token_response.athlete.profile_medium);
  413         active_model.scopes = Set(token_response.scope);
! 414         active_model.access_token = Set(crate::strava::credentials::seal(
  415             token_response.access_token.as_bytes(),
  416         )?);
! 417         active_model.refresh_token = Set(crate::strava::credentials::seal(
  418             token_response.refresh_token.as_bytes(),
  419         )?);
  420         active_model.created_at = Set(Utc::now());
  421         active_model.expires_at = Set(expires_at);
```


---



## bike-rs/bike-core/src/strava/workflow/child&#46;rs

```
  39         &self,
  40         row: &strava_workflows::Model,
  41     ) -> Result<bool, WorkflowError> {
  42         let task = background_tasks::Model::latest_strava_workflow_task(self.db, &row.id).await?;
! 43         match task {
  44             Some(task) if matches!(task.status.as_str(), "failed" | "canceled") => {
  45                 Err(child_failure(&task))
  46             }
  47             Some(task) if task.status == "completed" && row.status == "completed" => Ok(true),
```


---


```
  46             }
  47             Some(task) if task.status == "completed" && row.status == "completed" => Ok(true),
  48             Some(task) if matches!(task.status.as_str(), "pending" | "processing") => Ok(false),
  49             Some(task) => Err(child_failure(&task)),
! 50             None if row.status == "completed" => Ok(true),
! 51             None => Err(WorkflowError::internal("Strava child has no worker task")),
  52         }
  53     }
  54 }
```


---



## bike-rs/bike-core/src/strava/workflow/execution&#46;rs

```
  33     let row = load(db, id).await?;
  34     if row.status == "completed" {
  35         if let Some(lease) = lease {
  36             lease.commit().await?;
! 37         }
  38         return Ok(Execution::Completed);
  39     }
  40     let row = clear_pause(db, row).await?;
  41     let source = Source::from_value(row.source.clone())?;
```


---


```
  44         .await?;
  45     let Some(connection) =
  46         connection.filter(|connection| connection.created_at == source.generation)
  47     else {
! 48         row.finish(db, None).await?;
! 49         if let Some(lease) = lease {
! 50             lease.commit().await?;
! 51         }
! 52         return Ok(Execution::Completed);
  53     };
  54     if superseded_by_disconnect(db, &connection, action, &source).await? {
  55         row.finish(db, None).await?;
  56         if let Some(lease) = lease {
```


---


```
  54     if superseded_by_disconnect(db, &connection, action, &source).await? {
  55         row.finish(db, None).await?;
  56         if let Some(lease) = lease {
  57             lease.commit().await?;
! 58         }
  59         return Ok(Execution::Completed);
  60     }
  61     let result = execute_owned(db, row.clone(), source, connection, action).await;
  62     match &result {
```


---


```
  69         }
  70     }
  71     if let Some(lease) = lease {
  72         lease.commit().await?;
! 73     }
  74     result
  75 }
  76 
  77 async fn load(db: &DatabaseConnection, id: &str) -> Result<strava_workflows::Model, WorkflowError> {
```


---



## bike-rs/bike-core/src/strava/workflow/inbox&#46;rs

```
  240             correlation_id = %activity_id.map(|id| id.to_string()).unwrap_or_else(|| id.into()),
  241             workflow_id = id,
  242             strava_activity_id = activity_id,
  243             athlete_id = connection.athlete_id,
! 244             action = action.as_str(),
  245             "Accepted Strava workflow"
  246         );
  247     }
  248     Ok(result > 0)
```


---


```
  276         at,
  277         if super::provider_work_enabled() {
  278             "rate_limit"
  279         } else {
! 280             "provider_paused"
  281         },
  282     )
  283     .await
  284 }
```


---


