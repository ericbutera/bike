# Diff Coverage
## Diff: 8feed897db4c26335b6c9c3486bd3eb738d585f1...HEAD, staged, unstaged and untracked changes

- bike-rs/strava-temporal/src/activities&#46;rs (94.3%): Missing lines 278-280
- bike-rs/strava-temporal/src/client&#46;rs (100%)
- bike-rs/strava-temporal/src/workflows/activity_create&#46;rs (100%)
- bike-rs/strava-temporal/src/workflows/activity_delete&#46;rs (93.3%): Missing lines 24
- bike-rs/strava-temporal/src/workflows/activity_update&#46;rs (100%)
- bike-rs/strava-temporal/src/workflows/activity_upsert&#46;rs (100%)
- bike-rs/strava-temporal/src/workflows/disconnect&#46;rs (100%)
- bike-rs/strava-temporal/src/workflows/options&#46;rs (100%)
- bike-rs/strava-temporal/src/workflows/source&#46;rs (88.7%): Missing lines 44-46,98-101

## Summary

- **Total**: 218 lines
- **Missing**: 11 lines
- **Coverage**: 94.95%



## bike-rs/strava-temporal/src/activities&#46;rs

```
  274             event.operation,
  275             StravaOperation::Create | StravaOperation::Update | StravaOperation::Upsert | StravaOperation::Delete
  276         );
  277         if !source_operation {
! 278             return Err(activity_error(WorkflowError::bad_request(
! 279                 "Strava collection is not source reconciliation",
! 280             )));
  281         }
  282         let expected_id = event.source_workflow_id().map_err(activity_error)?;
  283         return require_workflow_id(workflow_id == Some(expected_id.as_str()));
  284     }
```


---



## bike-rs/strava-temporal/src/workflows/activity_delete&#46;rs

```
  20         logging::started(ctx, &event);
  21         let result = if ctx.patched("strava-source-reconciliation-v1") {
  22             super::source::reconcile(ctx, event.clone()).await
  23         } else {
! 24             delete(ctx, event.clone()).await
  25         };
  26         let result = options::command_result(ctx, result);
  27         let log_event = result.as_ref().map_or(&event, |value| &value.event);
  28         logging::finished(ctx, log_event, &result);
```


---



## bike-rs/strava-temporal/src/workflows/source&#46;rs

```
  40             }
  41             // A timed-out older Activity can outlive its child run. Its stale
  42             // snapshot cannot commit; refetch against the current domain row.
  43             if ctx.continue_as_new_suggested() {
! 44                 return ctx
! 45                     .continue_as_new(event, Default::default())
! 46                     .map(|never| match never {});
  47             }
  48         }
  49     }
  50 }
```


---


```
   94                 if ctx.continue_as_new_suggested() {
   95                     break;
   96                 }
   97             }
!  98             Err(error) => {
!  99                 let result: WorkflowResult<ActivityChange> = Err(WorkflowTermination::from(error));
! 100                 log.finished(ctx, &result);
! 101                 return result;
  102             }
  103         }
  104     }
  105     ctx.continue_as_new(event, Default::default())
```


---


