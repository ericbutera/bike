# Admin Operations Specification

Admin operations give Bike a way to inspect system state, re-run derived processing, and diagnose integrations without hand-editing database rows.

## Product Intent

Admin tools should be boring, explicit, and recoverable. They exist to repair or inspect system state while preserving user data and respecting normal processing contracts.

## Admin Surfaces

Admin navigation exposes operational pages for:

- manual tasks;
- activities;
- users;
- feature flags;
- integration events;
- import history and stage replay;
- background tasks.

Admin pages require an authenticated admin user. Non-admin users should not be able to reach admin-only operations through either UI or API.

## List presentation

- Use the Users page's shared card and zebra data grid for integration events,
  import history, activities, registered processors, and task history.
- Keep compact dropdowns and search inputs in a toolbar above the grid, aligned
  to the right, with Search and Clear controls. Wrap controls on smaller screens
  and allow wide grids to scroll horizontally.
- Integration events keep provider and positive user/activity/import ID filters.
  Text search covers the latest 100 matching events and is labeled accordingly.
- Import history keeps owner, source, outcome, archive scope, pagination, and
  links to import diagnostics. Outcome, stage, error, and format have separate
  columns; long filenames and errors expose their full text in tooltips.
- Activities filter the full stored population by title or exact activity ID,
  positive owner ID, source, and normalized sport (including legacy spellings)
  before counting and pagination. Filters reset pagination. ID, owner, sport,
  source, format, import ID, and import status
  use separate columns. Long titles use tooltips.
- Tasks has two independent cards: All registered processors and task history.
  Processor window, outcome, and name filters affect only the processor grid.
  Task history defaults to completed tasks, retains status/type/error/date and
  correlation search, pagination, detail inspection, rerun, and cancellation.
  A correlation deep link opens all matching statuses, including queued work.
- `/admin/analytics` and its navigation entry are removed. Analytics maintenance
  remains available through Manual tasks.

## Task processing history

- Admin task search accepts an exact request, trace, or pipeline run ID. A
  matching origin returns its queued descendants, including work that has not
  started. A trace recorded on an execution attempt also locates that task.
- Task details show each recorded attempt's start/end, outcome, error,
  heartbeat, and trace/span IDs. Retry and recovery preserve earlier attempts;
  recovery never resets the execution number. A late result cannot overwrite a
  cancellation or a later execution. Admin cancellation records the cancellation;
  processors retain their existing cooperative cancellation behavior.
- Every worker poll recovers executions whose heartbeat lease has been stale
  for five minutes. Recovery preserves source payloads and lineage, records the
  interrupted attempt, and rejects snapshots superseded by a heartbeat.
- Pipeline details show the immutable original receipt, first durable task
  acceptance, parent task, and receipt-to-task timing. Follow the parent link to
  inspect the processor that scheduled the task. Rerun starts a new admin origin;
  automatic retry and stale-execution recovery retain their existing origin.
- Receipt origins cover API requests, signed gateway inbox/sync deliveries,
  cron enqueue and worker startup. HTTP acceptance of a gateway source and its
  task intent commits together. Retries retain the first receipt.
- Activity processing opens the combined `/admin/imports/{id}` view for linked
  imports; activities without import history retain a worker-run dialog.
  The combined view includes durable import attempts/stage replay and all
  directly linked pipeline runs. Visible history cards replace selection
  dropdowns. Labeled progress and timing components separate receipt, durable
  acceptance, work completion, and confirmed publication. The expandable
  interactive worker graph shows task parents, attempts, inline work,
  authoritative import stages, gateway intervals and
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
  Barrier reconciliation visits every running batch in ordered pages; active
  batches cannot prevent later completed batches from releasing their locks.

## Import history

`/admin/imports` provides the import list; `/admin/imports/{id}` combines durable
attempts, stage summaries, reviewed replay actions, and the activity's worker
runs/publication. Admin activity processing actions link to this same page.
Admin navigation links to Import history. History defaults to the
administrator's account and supports
selecting another owner by positive user ID. The existing admin trace endpoint
reads any owner's import; replay always keeps that import's recorded owner.

History and replay APIs live under `/api/admin/activity-imports`; normal rider
and synthetic credentials do not authorize them. The old rider history, trace,
and replay routes are removed. Upload and archive progress remain available
to the owning rider. The [ingestion specification](activity-ingestion.md#import-history-and-administration)
defines source filtering, durable attempts, recovery, and replay invariants.

## Metrics

- Grafana's Bike System Health owns the former admin metrics information.
  `/admin/metrics`, `/api/admin/metrics` and `/api/admin/metrics/app` are removed,
  together with their navigation, client hooks and JSON/OpenAPI contracts.
- Prometheus `/metrics` scrapes reload retained activity/import/segment totals,
  their rolling 30-day creation counts, new accounts over 24 hours, and retained
  auth events/refresh-token rows over 30 days from PostgreSQL.
- These are database-backed gauges, not lifetime event counters. Labels are
  bounded record kinds, windows and event types; user IDs, tokens, emails and
  other personal data never become labels. A failed database query fails the
  scrape rather than reporting a fresh zero.
- Auth event counts include retained legacy password-auth records; they are
  not the identity provider's audit. Consumed/deleted refresh tokens are absent
  from the retained-token count, so it must not be labeled cumulative issuance.
- Reuse existing worker queue gauges and terminal outcome counters for current
  runnable work and selected-range completed/failed tasks. Retained failed-task
  rows must not substitute for incident counts.
- Reuse connected-Strava-account and shared quota gauges; expired and missing
  quota windows follow the [provider contract](strava-integration.md#api-limits).
- Grafana deduplicates shared snapshots across replicas with `max` and handles
  counter resets per instance before summing outcomes. Missing samples remain
  unknown; rolling-window cards state their fixed windows explicitly.

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

Use Admin → Import history to select the owner and failed import, including
failures before an activity exists. Inspect durable attempts and the failed
stage. Preserve missing/corrupt source evidence and deploy parser fixes before
replay; follow the [source recovery runbook](../runbooks/activity-source-recovery.md)
when original bytes need restoration.

Request `GET /api/admin/activity-imports/{id}/replay?stage={stage}` to review the
actual start stage, reusable attempt, and fallback reason. Queue the reviewed
plan with `POST` to the same route, including `expected_start_stage` and
`expected_reused_attempt_id`. A changed prerequisite or active execution
returns 409. The accepted response identifies the durable attempt; follow that
attempt to its terminal outcome. Replay keeps the recorded owner, activity
identity, original source, and earlier attempt history.

This per-import operation covers both linked activities and failures without
an activity. Do not hand-edit import status or rerun an entire archive/sync to
recover one input. Bulk repair still uses the explicit admin manual tasks and
the ingestion contract.

## Code Anchors

- Admin API: `bike-rs/api/src/controllers/admin.rs`
- Integration event API: `bike-rs/api/src/controllers/integration_events.rs`
- Metrics: `bike-rs/api/src/metrics.rs`
- Admin task UI: `bike-ui/components/admin/AdminTaskTools.tsx`
- Admin navigation: `bike-ui/components/admin/Nav.tsx`

## Follow-up tracking

Follow-up status and priority live only in the [Bike TODO](../TODO.md).
This specification is the behavior reference for OPS02.
