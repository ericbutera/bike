# Worker operations

## BikeWorkerFailures

- Each failed execution increments `bike_rust_worker_tasks_failed_total` once.
  Failed retries count; successful executions increment the completion counter.
- The alert uses `sum(increase(...[1h])) > 0`. It clears after those errors leave
  the last-hour window.
- Open the worker dashboard's error logs and the task's retained error in admin.
  Repair the underlying error and rerun failed work when needed.
- Search logs in the alert's time window for `event=worker_execution_error`.
  Each counter increment logs the error, task, attempt, request ID and trace ID.
  Use those IDs to follow the original request and related jobs through traces.
  Legacy work without an originating request has no fabricated request ID.
- Grafana error cards use the selected time range and the same execution counter.
  Old failed task rows never become new errors.
- Persisted anomaly evaluation, processor health budgets,
  `BikeWorkerPipelineBlocked`, `BikeWorkerAnomalousRuns` and
  `BikeWorkerDiagnosticsUnavailable` are retired.

- Queue depth and task history remain dashboard diagnostics. Database-derived
  backlog and capacity alerts are retired.

## History and verification

- Admin task errors, attempts and pipeline links remain diagnostic history.
- Historical migrations are preserved; this simplification requires no new fields
  or data repair.
- Verify the deployed image, worker readiness, live counters and dashboard after
  release. Source tests alone do not establish production recovery.
