# Worker polling audit — 2026-10-17

The worker dispatches work from requests and activity changes. Temporal owns
execution, retries, and child completion. Idle application loops must not scan
PostgreSQL or create empty workflows to discover work.

Behavior is defined in [background work](specs/background-work.md) and
[heatmaps](specs/heatmaps.md). This report records the audit and cutover work.

| Dispatch path          | Previous behavior                                           | Correction                                                                                                                                 |
| ---------------------- | ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| Heatmap reconciliation | A new workflow every 15 seconds, including empty scans      | Remove automatic schedule creation. Upload/reprocess already await preparation children; manual edits now submit preparation after commit. |
| Strava event inbox     | Query pending SQL events every second, then start workflows | Remove the dispatcher. Request acceptance starts the workflow directly before returning success.                                           |
| Strava inbox telemetry | Scan unsubmitted events every 30 seconds                    | Remove the sampler and its gauges with the dispatcher.                                                                                     |
| Strava connection scan | Start a sync for every eligible connection every six hours  | Remove automatic schedule creation. Webhooks, manual sync/backfill, and OAuth completion submit directly.                                  |

## Direct submission and tests

- Keep event persistence and canonical webhook identity for request history and
  deduplication. On redelivery, start the original stored event/workflow ID.
- Temporal start failure or uncertainty returns an error to the caller; never
  acknowledge work merely because it was written to SQL. A duplicate native start
  is successful. Keep the callback's existing total 1.5-second acceptance deadline.
- Sync, backfill, disconnect, OAuth completion, and the recovery CLI share the
  same acceptance implementation; they no longer depend on a worker-side relay.
- Manual activity edits submit heatmap preparation after the edit transaction.
  A retry may repeat preparation; it must not depend on the edit changing twice.
- Heatmap evidence recovery previously returned success after invalidating its
  own generation. It now returns a retryable error, allowing native Activity retry
  to reload and publish the current generation.
- Unit coverage exercises direct starts, duplicate acknowledgment, start/SQL
  failures, loaded-connection reuse, edit dispatch, generation refresh, and the
  existing upload/reprocess parent-child sequences. Run Cargo's full workspace
  suite without test-name filters.

## Production adjustments

- At **14:21 America/Detroit on 2026-10-17**, both schedules were verified paused.
  Heatmap reconciliation remained at **828 completed executions**, with none
  running; the count did not grow after pausing. The SQL inbox contained **zero
  unsubmitted events** at that check. These are point-in-time observations.
- Production still runs the previous API/worker images and its inbox dispatcher.
  Code removal requires the release below.
- The user confirmed unchanged source data and explicitly waived inbox draining.
  Both `bike-heatmap-reconciliation` and `bike-strava-sync` schedules were deleted.
  Native batch `5989c75d-6520-4fa1-8f1c-b3e93942754c` deleted **828/828**
  `reconcile_heatmaps` executions with **zero failures**. The namespace schedule
  list is empty and the matching execution count is zero. Retained activity data
  and source objects were not removed. The deployed old worker can recreate the
  schedules on restart until the creation-code removal is released.
- Coordinate the API and worker release: deploy the new API first while the old
  relay still runs, then deploy the worker without its relay.
  Stable workflow IDs make overlapping submissions safe. The old API only records
  SQL events, so deploying the new worker first would strand requests. Confirm API
  access to the configured Temporal namespace and Strava task queue before cutover.
- No inbox drain or data-repair stage is required for this release. Keep existing
  event/history rows. A failed submission after cutover requires request retry or
  an explicit operator resubmission; there is no automatic SQL retry service.
- Retire `BikeStravaHandoffStalled` and `BikeStravaHandoffTelemetryStale`, with their
  rule fixtures, in `pulumi-iac/nibelheim/observability/bike-alerts{,.test}.yaml`.
  Otherwise the intentionally absent gauges trigger a false stale-telemetry alert.
  Preserve native workflow failure and provider availability monitoring.
- Historical projection upgrades, feature re-enablement, and direct source-repair
  writes require an explicit heatmap repair. The retained `reconcile_heatmaps`
  workflow pages pending/failed projections only when deliberately started.
  Source-repair utilities must request preparation or run that repair after a batch;
  pending SQL rows alone do not dispatch work.
- Verify a real webhook and file upload create their expected workflow/children;
  a manual edit starts preparation; an idle worker creates no reconciliation runs.
  Test a rejected start and redelivery without duplicate activities.

## Other timed behavior reviewed

- Temporal SDK task-queue long polling: required native worker transport; retained.
- Strava source contention: a request-owned workflow retries a rejected child
  start with native timers (1–30 seconds). This preserves PR #80 source ordering;
  it does not scan idle domain data or create periodic root jobs. Retained.
- Provider Activity heartbeats and retry/rate-limit waits: run only for active
  requests; retained.
- Sync/archive paging and Continue-As-New: finite work within an explicit request;
  retained. Existing scan/reconciliation workflow registrations also support
  retained-history replay and explicit operator repairs.
- Hourly map-image cache pruning: bounded storage maintenance predating this
  correction, also performed after writes; outside worker dispatch scope.
- Browser status refreshes (imports, account, admin, training, heatmaps): read-only
  UI refreshes, not background job dispatch. No UI transport redesign in this change.

## Validation

- The import and reprocess entry points now call one shared source/derived
  sequence, preserving the same Temporal command order. Both await segment
  analytics, heatmap, and fitness children; achievements and training use the
  same domain Activities. Regression coverage runs the child code for both
  foreground and background priorities.
- Archive/import storage now shares the injected `ObjectStore` backend.
  Tests use the in-memory backend for archived ZIP retention, retry after the
  local source is removed, paging, upload/artifact bytes, and multipart staging.
  Production uses the existing filesystem adapter; S3/GCS are not enabled here.

- Full Rust workspace: **638 passed, 17 explicitly ignored, zero failures or
  filtered tests** with `cargo test --workspace --locked`.
- Formatting and Clippy checked every workspace target and feature with warnings
  denied. Required commit hooks also enforce the owning checks.
- Before this storage/shared-sequence update, changed-line coverage passed the
  per-project 80% gate: API **95.24%**, worker
  **91.67%**, integrations **94.07%**, and core **100%**. Other projects have no
  changed measured lines in this correction.
- The extra cross-project coverage run passed 281 UI tests but emitted a pnpm
  update notice (12.10.1 → 12.14.0). It is not counted as a clean cross-project
  validation. Tool upgrades are outside this worker correction; Rust coverage
  is validated separately without changing tool pins or suppressing the notice.
- Production checks above verify schedule/history deletion only; the new request
  submission path has not been deployed or exercised against the live provider.
