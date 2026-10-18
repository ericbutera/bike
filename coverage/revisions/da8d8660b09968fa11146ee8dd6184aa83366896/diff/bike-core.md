# Diff Coverage
## Diff: 394f518cc12ab5dd623b454f093dad77c20d3398...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-core/src/entities/strava_activity_states&#46;rs (100%)
- bike-rs/bike-core/src/jobs/adapter&#46;rs (100%)
- bike-rs/bike-core/src/jobs/commands&#46;rs (95.7%): Missing lines 43
- bike-rs/bike-core/src/jobs/history&#46;rs (96.8%): Missing lines 105-106,146
- bike-rs/bike-core/src/jobs/native&#46;rs (100%)
- bike-rs/bike-core/src/test_support/temporal_client&#46;rs (100%)

## Summary

- **Total**: 216 lines
- **Missing**: 4 lines
- **Coverage**: 98.15%



## bike-rs/bike-core/src/jobs/commands&#46;rs

```
  39             | Self::ArchiveFitImports(page)
  40             | Self::XcTraining(page) => page.user_id > 0 && page.after_id == 0,
  41         };
  42         if !valid {
! 43             return Err(WorkflowError::bad_request("A valid maintenance target is required"));
  44         }
  45         Ok(())
  46     }
  47 }
```


---



## bike-rs/bike-core/src/jobs/history&#46;rs

```
  101             .payloads
  102             .into_iter()
  103             .map(|payload| match payload.metadata.get("encoding").map(Vec::as_slice) {
  104                 Some(b"json/plain") => serde_json::from_slice(&payload.data).map_err(Error::from),
! 105                 Some(b"binary/null") => Ok(serde_json::Value::Null),
! 106                 _ => Err("Workflow input encoding is not supported".into()),
  107             })
  108             .collect::<Result<_, Error>>()?;
  109         Ok(TaskExecutionInput { parameters })
  110     }
```


---


```
  142             4 => TaskStatus::Canceled,
  143             5 => TaskStatus::Terminated,
  144             6 => TaskStatus::ContinuedAsNew,
  145             7 => TaskStatus::TimedOut,
! 146             _ => TaskStatus::Unspecified,
  147         },
  148         started_at: info
  149             .start_time
  150             .and_then(|time| DateTime::from_timestamp(time.seconds, time.nanos as u32)),
```


---


