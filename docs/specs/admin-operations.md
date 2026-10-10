# Admin Operations Specification

Admin operations give Bike a way to inspect system state, re-run derived processing, and diagnose integrations without hand-editing database rows.

## Product Intent

Admin tools should be boring, explicit, and recoverable. They exist to repair or inspect system state while preserving user data and respecting normal processing contracts.

## Admin Surfaces

Admin navigation exposes operational pages for:

- analytics and manual tasks;
- users;
- feature flags;
- metrics;
- integration events;
- background tasks.

Admin pages require an authenticated admin user. Non-admin users should not be able to reach admin-only operations through either UI or API.

## Task processing history

- Admin task search accepts an exact request, trace, or pipeline run ID. A
  matching origin returns its queued descendants, including work that has not
  started. A trace recorded on an execution attempt also locates that task.
- Task details show each recorded attempt's start/end, outcome, error,
  heartbeat, and trace/span IDs. Retry and recovery preserve earlier attempts;
  recovery never resets the execution number. A late result cannot overwrite a
  cancellation or a later execution. Admin cancellation records the cancellation;
  processors retain their existing cooperative cancellation behavior.
- Pipeline details show the immutable original receipt, first durable task
  acceptance, parent task, and receipt-to-task timing. Follow the parent link to
  inspect the processor that scheduled the task. Rerun starts a new admin origin;
  automatic retry and stale-execution recovery retain their existing origin.
- Receipt origins cover API requests, signed gateway inbox/sync deliveries,
  cron enqueue and worker startup. HTTP acceptance of a gateway source and its
  task intent commits together. Retries retain the first receipt.
- Activity processing opens all directly linked pipeline runs, including
  activities without import history. The interactive graph shows task parents,
  attempts, inline work, authoritative import stages, gateway intervals and
  output revisions. Selection shows receipt-to-start/result and stage/runtime.
  Log/trace links use the deployment's configured Grafana URL.
- Receipt-to-available uses committed, revision-matched publications and a closed
  successful task tree. Receipt-to-end uses the last terminal task timestamp.
  Failed downstream outputs remain unready. Unknown legacy evidence is N/A.
- The processor table includes the actual registered Rust processors with zero
  sample states, full-population p50/p90, sample count, outcome and window.
  Queued, scheduled, running, retrying and failed counts are distinct.
  Gateway queue operations and runtime distributions are in the gateway dashboard.
- Meaningful import stage/work completion advances progress; a heartbeat alone
  does not. Anomalies store reason, observed/expected value, policy version,
  evaluation time and sample count. Relative duration needs 30 earlier comparable
  completed runs. The same records drive dashboards and alerts.
- Task graphs use pages of 50 tasks, 25 work units and 100 outputs. Archive and
  bulk producers queue bounded pages; their child barrier retains the exact lock
  until descendants finish. An admin rerun starts a fresh run outside the old
  batch. See the [visibility plan](../plans/pipeline-visibility.md) for operational
  acceptance and the [worker runbook](../production-failures.md#bikeworkerpipelineblocked).

## Metrics

Bike exposes app metrics in addition to shared system metrics. Metrics should be suitable for dashboards and alerting without leaking sensitive user data.

The app metrics page should distinguish Bike-specific stats from framework or infrastructure metrics.

## Backfills And Manual Tasks

Admin manual tasks can enqueue work such as:

- analytics backfill;
- XC training backfill for a user;
- user segment regeneration;
- specific segment effort regeneration;
- user activity import reprocessing;
- duplicate activity cleanup;
- archive import for a user.

Manual tasks should queue background work where possible. They should return task ids, queued status, or clear summaries rather than doing expensive work in the admin request path.

## Integration Event History

Integration events provide an audit trail for provider behavior, especially Strava. Events should include provider, event type, level, user when known, message, metadata, and timestamp.

The user-facing account page may show a filtered connection history. Admin pages may show broader integration history for debugging.

## Feature Flags

Feature flags are operational switches. They should not become hidden product requirements. If a feature flag changes intended behavior, the relevant spec should describe both the enabled behavior and the fallback.

## Failed import recovery

Use the alert's namespace and count to inspect failed rows through that site's
protected database connection using the deployment's `DATABASE_URL`. Start
with this read-only query:

```sql
SELECT id, user_id, source, format, status, processing_stage,
       processing_attempts, processing_error, storage_path, activity_id
FROM activity_imports
WHERE status = 'failed'
ORDER BY last_processing_event_at DESC NULLS LAST
LIMIT 50;
```

For one import, inspect its durable processing history:

```sql
SELECT event_type, created_at, message, payload
FROM integration_events
WHERE provider = 'activity_processing'
  AND payload->>'import_id' = '<import-id>'
ORDER BY created_at;
```

Compare the stage/error with the original file at `storage_path` on that site's
uploads volume. Preserve a parse/format failure, add a small fixture from the
file (sanitize personal data), and deploy the parser/workflow fix before retrying.
Choose the recovery operation for the failed import:

- When an activity is linked: an administrator can
  `POST /api/admin/activity-imports/reprocess-activity` with JSON
  `{"activity_id": <activity-id>}`. Track the returned task ID.
- When parsing failed before an activity was created: find the matching
  `process_activity_import` task using the query below. Rust has no per-import
  retry endpoint; uploading the same bytes returns the existing import. After
  the fix is deployed, set only that failed import back to `processing`, then
  use the admin task's Rerun action or `POST /api/admin/tasks/<task-id>/rerun`.
  The worker skips imports whose status is still `failed`. If there is no
  matching task, preserve the failure and escalate with its original file;
  do not replay an entire archive or sync to recover one import.

```sql
SELECT id, status, error, payload
FROM background_tasks
WHERE task_type = 'process_activity_import'
  AND COALESCE(payload->'data', payload)->>'import_id' = '<import-id>'
ORDER BY created_at DESC
LIMIT 5;
```

For that Rust task rerun, verify its payload's user/import IDs and that no
matching task is already pending/processing. Then prepare only that import:

```sql
UPDATE activity_imports AS i
SET status = 'processing', processing_stage = 'raw_stored', updated_at = now()
WHERE i.id = <import-id> AND i.status = 'failed'
  AND NOT EXISTS (
    SELECT 1 FROM background_tasks AS t
    WHERE t.task_type = 'process_activity_import'
      AND t.status IN ('pending', 'processing')
      AND COALESCE(t.payload->'data', t.payload)->>'import_id' = i.id::text
  )
RETURNING i.id, i.status;
```

Require one returned row, rerun the matching task once, and follow its new task
ID to completion. If the rerun request fails to enqueue, restore this import's
status to `failed` while preserving its error. A zero count during processing
is not proof of recovery: confirm the task succeeds and the activity is present.
Do not mark a missing-activity import processed or delete its file/history.

## Code Anchors

- Admin API: `bike-rs/api/src/controllers/admin.rs`
- Integration event API: `bike-rs/api/src/controllers/integration_events.rs`
- Metrics: `bike-rs/api/src/metrics.rs`
- Admin task UI: `bike-ui/components/admin/AdminTaskTools.tsx`
- Admin metrics UI: `bike-ui/components/admin/BikeMetricsSection.tsx`
- Admin navigation: `bike-ui/components/admin/Nav.tsx`

## Follow-up tracking

Follow-up status and priority live only in the [Bike TODO](../TODO.md).
This specification is the behavior reference for OPS02.
