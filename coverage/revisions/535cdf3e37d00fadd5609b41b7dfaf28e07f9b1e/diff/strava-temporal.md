# Diff Coverage
## Diff: c997295d36529a615e8daf459ead364e50f7f571...HEAD, staged, unstaged and untracked changes

- bike-rs/strava-temporal/src/activities&#46;rs (92.3%): Missing lines 418
- bike-rs/strava-temporal/src/client&#46;rs (100%)
- bike-rs/strava-temporal/src/workflows/backfill&#46;rs (100%)
- bike-rs/strava-temporal/src/workflows/collection&#46;rs (100%)
- bike-rs/strava-temporal/src/workflows/disconnect&#46;rs (0.0%): Missing lines 52
- bike-rs/strava-temporal/src/workflows/options&#46;rs (100%)
- bike-rs/strava-temporal/src/workflows/scheduled_sync&#46;rs (100%)
- bike-rs/strava-temporal/src/workflows/sync&#46;rs (100%)

## Summary

- **Total**: 114 lines
- **Missing**: 2 lines
- **Coverage**: 98.25%



## bike-rs/strava-temporal/src/activities&#46;rs

```
  414     #[test]
  415     fn oversized_activity_output_is_a_non_retryable_failure() {
  416         let error = checked_output(vec![i32::MAX; 30_000]).unwrap_err();
  417         let ActivityError::Application(failure) = activity_error(error) else {
! 418             panic!("Expected a terminal application failure")
  419         };
  420         assert!(failure.is_non_retryable());
  421     }
  422 }
```


---



## bike-rs/strava-temporal/src/workflows/disconnect&#46;rs

```
  48                 .build();
  49             options::execute_child_workflow(ctx, DeleteActivity::run, child, options).await?;
  50         }
  51         if more {
! 52             options::validate_input(ctx, &event)?;
  53             return ctx
  54                 .continue_as_new(event, Default::default())
  55                 .map(|never| match never {});
  56         }
```


---


