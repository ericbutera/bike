# Diff Coverage
## Diff: 7afb304c41f952e0fbc55803c9caf2d3f6b85c0d...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-core/src/background_jobs/worker/metrics&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/worker/metrics/diagnostics&#46;rs (100%)
- bike-rs/bike-core/src/background_jobs/worker/task_worker&#46;rs (88.2%): Missing lines 353-354

## Summary

- **Total**: 19 lines
- **Missing**: 2 lines
- **Coverage**: 89.47%



## bike-rs/bike-core/src/background_jobs/worker/task_worker&#46;rs

```
  349                 task_type = updated.task_type,
  350                 attempt = updated.attempts,
  351                 status = updated.status,
  352                 error = updated.error,
! 353                 request_id = origin
! 354                     .as_ref()
  355                     .and_then(|context| context.request_id.as_deref()),
  356                 trace_id = trace.as_ref().map(|(trace_id, _)| trace_id.as_str()),
  357                 span_id = trace.as_ref().map(|(_, span_id)| span_id.as_str()),
  358                 pipeline_run_id = origin.as_ref().map(|context| context.run_id.as_str()),
```


---


