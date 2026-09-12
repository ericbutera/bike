# Strava Integration Specification

Strava is Bike's first live cloud activity integration. It must remain isolated behind a single Bike-owned client boundary, respect Strava's app-wide limits, and treat long-running imports as resumable work instead of all-or-nothing worker tasks.

## Product Intent

Connecting Strava should feel simple for riders while keeping provider behavior transparent to Bike operators. Sync status, rate-limit pauses, partial progress, webhook handling, and provider errors should be visible through integration events and admin tooling.

Large initial imports are expected. A rider with years of activity history should not lose partial progress because Strava limits are exhausted, the worker restarts, or a task times out.

## Current State

All outbound Strava HTTP calls are currently made through `StravaApiClient` in `api/src/strava.rs`. The worker does not call Strava directly; the `strava_sync` processor delegates to `api::strava::process_strava_sync`.

The existing client is centralized but still lives inside the broader Strava service module. It builds request URLs, sends `reqwest` calls, parses JSON, and converts HTTP failures to `AppError`. Strava calls now reserve provider quota before sending, reconcile Strava rate-limit headers after responses, and treat `429 Too Many Requests` as a structured retryable pause.

The current sync shape is:

- refresh the access token if needed;
- list athlete activities in pages of up to 100;
- request streams for each activity;
- persist each activity through the normal activity import pipeline;
- mark the whole sync succeeded or failed at the end.

This shape no longer calls Strava after local quota is exhausted, but it is still not fully checkpointed. A paused sync releases the user activity import lock and schedules a follow-up task for the limiter-provided retry time. Until durable sync checkpoints are added, resumed work still relies on existing source correlation ids and deduplication rather than an explicit page/activity cursor.

## Required Strava Limits

Bike must honor these app-level Strava limits:

- overall short window: 400 requests every 15 minutes;
- overall daily window: 4,000 requests per day;
- read short window: 200 requests every 15 minutes;
- read daily window: 2,000 requests per day.

Read requests consume both read quota and overall quota. Non-read requests consume overall quota. If Strava's headers report stricter or already-consumed usage, Bike should reconcile local state to the provider-reported values.

## Client Boundary

All outbound Strava calls should be isolated behind a dedicated Strava API client or SDK wrapper. Business flows should not construct Strava URLs, call `reqwest` directly, or perform ad hoc retry/rate-limit handling.

The client boundary should own:

- endpoint request construction;
- access-token bearer handling where applicable;
- request classification as read or non-read;
- shared quota reservation before sending;
- response header reconciliation after sending;
- `429` handling and next-attempt calculation;
- structured logging and OpenTelemetry spans;
- provider error normalization.

Service code should call high-level methods such as `exchange_authorization_code`, `refresh_access_token`, `list_activities`, `get_activity_streams`, `deauthorize`, `list_push_subscriptions`, and `create_push_subscription`.

## Postgres Rate Limiter

Bike should use Postgres for Strava rate-limit coordination. Traffic is expected to be low, Postgres is already required, and adding Redis only for this limiter would add operational complexity without enough benefit.

The limiter must coordinate across:

- API requests;
- worker jobs;
- multiple worker processes if the deployment scales out;
- future admin tools that may call Strava.

The limiter should store provider, window, limit, used count, reset time, and updated time in Bike-owned tables. Quota acquisition should happen inside a transaction using row-level locking so concurrent requests share the same counters.

If quota is available, the limiter reserves the needed units and allows the request. If quota is exhausted, the limiter returns the earliest safe retry time instead of letting callers sleep blindly or call Strava anyway.

Daily windows and 15-minute windows should both be checked before a request is sent. A read request must reserve from both read windows and both overall windows. A non-read request must reserve from both overall windows.

The rate limiter is intentionally provider-generic so future cloud integrations can reuse the same `provider_rate_limit_buckets` table and limiter API with provider-specific bucket definitions.

## Resumable Sync

Strava sync must become checkpointed work. The worker task should not be the only place progress exists.

A sync should persist enough state to resume safely after:

- the process exits;
- the task fails and retries;
- rate limits block further calls;
- the deployment restarts;
- the connection is disconnected mid-sync.

Useful checkpoint state includes:

- connection id and user id;
- sync mode, such as initial, manual, or webhook-triggered;
- current page or pagination cursor;
- `after` timestamp used for the activity list;
- last activity id or started-at timestamp processed within the current page;
- imported, duplicate, and failed counters;
- latest synced activity timestamp discovered so far;
- next eligible run time when paused by rate limits;
- terminal status and message.

When rate limits are exhausted, the sync should persist its checkpoint, record an integration event, release the user activity import lock, and requeue itself for the limiter-provided retry time. It should not block a worker thread for the rest of the window.

Partial imports already persisted through the activity pipeline should remain valid. Resuming should be idempotent through source correlation ids and existing deduplication.

## Worker Semantics

The current background task model is effectively all-worked or failed from the task runner's perspective. Strava sync should avoid making task completion the only durable marker of progress.

The desired pattern is:

- tasks are short enough to finish without holding process resources across external wait windows;
- long external workflows persist checkpoints in domain tables;
- a task may complete successfully after pausing and scheduling the next task;
- failed task attempts do not erase domain progress;
- abandoned running syncs can be detected and resumed or marked recoverable.

This pattern should be reused for future provider integrations and long backfills.

## Backfill Behavior

Initial Strava backfill should prefer steady, resumable progress over speed. It should process activities oldest-to-newest within fetched pages when possible, so imported history grows coherently and derived analytics can be finalized in batches.

If the daily read limit is exhausted, the connection should show a paused or waiting state rather than a failed state. Rider-facing messaging should make clear that Bike is waiting on Strava quota and will continue automatically.

Webhook-triggered syncs should remain incremental and cheap. Webhooks are the preferred way to keep ongoing usage well below daily limits after initial backfill is complete.

## Observability

Bike should add OpenTelemetry support for API and worker processes. The project already has metrics endpoints, but provider sync work needs distributed traces and structured span attributes that can flow into the existing Grafana stack.

The production observability stack installs into an `observability` namespace with `kube-prometheus-stack` for Prometheus, Grafana, Alertmanager, and CRDs; `grafana/loki-stack` with promtail enabled for logs; and `grafana/tempo` for traces. Bike's API and worker already have `ServiceMonitor` manifests in the deployment infrastructure, and production Prometheus is configured to discover ServiceMonitors from the observability and app namespaces. Bike observability work should therefore prefer real Prometheus metrics and OpenTelemetry spans over app-only admin counters when the signal is operational.

Prometheus scrape endpoints should remain internal. Bike's API mounts `/metrics`, and the worker exposes its own metrics port, but public ingress should only route user/API traffic and should not expose those scrape paths to the internet. The API scrape path is currently reached through the ClusterIP service selected by the `ServiceMonitor`, while public ingress forwards API traffic under `/api`.

Strava client spans should include:

- endpoint or operation name;
- request classification, such as read or overall-only;
- quota bucket decisions;
- wait or retry time when rate-limited;
- Strava response status;
- provider rate-limit header values when present;
- connection id and user id when safe to include;
- activity id for per-activity stream calls.

Metrics should include:

- Strava requests by operation and status;
- quota reservations by bucket;
- rate-limit pauses by bucket;
- sync checkpoints created and resumed;
- activities imported, duplicated, and failed by sync;
- sync duration and time spent waiting on provider quota.

Integration events remain the user/admin audit trail. OpenTelemetry and metrics are the operational view.

## Code Anchors

- Strava controller: `api/src/controllers/strava.rs`
- Current Strava service and client: `api/src/strava.rs`
- Strava provider payload parsing: `api/src/strava_provider_payload.rs`
- Provider rate limiter: `api/src/provider_rate_limit.rs`
- Provider rate-limit entity: `api/src/entities/provider_rate_limit_buckets.rs`
- Provider rate-limit migration: `migration/src/m20260912_000001_create_provider_rate_limit_buckets.rs`
- Worker processor: `worker/src/tasks/processors/strava_sync.rs`
- Task enqueueing: `api/src/tasks/adapter.rs`
- Activity import pipeline: `api/src/activity_import_pipeline.rs`
- Integration events: `api/src/integration_events.rs`

## Implementation Checklist

### Provider Rate Limiting

- [x] Rate-limit table: add `provider_rate_limit_buckets` with provider, bucket, limit, used count, reset time, created time, and updated time.
- [x] Rate-limit entity: add SeaORM entity for `provider_rate_limit_buckets`.
- [x] Limiter API: add provider-generic quota reservation and header reconciliation functions in `api/src/provider_rate_limit.rs`.
- [x] Transactional quota reservation: reserve all required buckets together before sending a provider request.
- [x] Postgres row locking: use exclusive row locks for quota rows when running on Postgres.
- [x] Strava bucket definitions: define overall 15-minute, overall daily, read 15-minute, and read daily buckets.
- [x] Strava request classification: classify token/deauthorize/subscription-create calls as overall-only and activity/subscription-list calls as read.
- [x] Strava header reconciliation: parse Strava overall and read rate-limit headers and move local counts forward when provider usage is stricter.
- [x] Structured `429` errors: expose `retry_at` through `AppError::too_many_requests`.
- [x] Retry-after header support: use provider retry headers if Strava or a future provider returns them.
- [ ] Per-provider configuration: allow limits to be overridden for test, staging, or provider plan changes without code edits.

### Strava Client Boundary

- [x] Centralize all Strava HTTP sends behind the Strava API client helper.
- [x] Route every Strava API client request through quota reservation before sending.
- [x] Route every Strava API client response through rate-limit header reconciliation.
- [x] Keep service code on high-level Strava client methods instead of raw URLs.
- [x] Add Prometheus counters for Strava provider API requests by operation, request class, and status.
- [x] Add Prometheus counters for local and remote Strava rate-limit pauses.
- [x] Add Prometheus gauges for provider quota bucket limit, usage, remaining quota, and reset timestamp.
- [ ] Extract the Strava client into a dedicated module separate from sync/business flow code.
- [ ] Normalize provider errors into typed Strava/client error variants instead of relying only on `AppError`.
- [ ] Add OpenTelemetry spans around Strava client calls with operation, bucket, status, and retry attributes.

### Production Observability

- [x] Confirm production observability includes Prometheus, Loki, Tempo, Grafana, and Alertmanager.
- [x] Confirm Bike API and worker have `ServiceMonitor` manifests for Prometheus scraping.
- [x] Confirm production Grafana has Prometheus, Loki, and Tempo datasources provisioned.
- [x] Emit real Prometheus counters for outbound provider API calls and provider rate-limit pauses.
- [x] Confirm public ingress does not expose Bike's Prometheus scrape paths.
- [ ] Add a Bike Grafana dashboard ConfigMap for Strava/provider API request rate, error rate, and rate-limit pauses.
- [ ] Add a deployment or smoke-test check that fails if `/metrics` becomes reachable through a public ingress host.
- [ ] Consider moving API metrics to a dedicated internal metrics port if future ingress or gateway routing makes path-level isolation harder to reason about.
- [ ] Add Prometheus alerts for sustained Strava 429s, exhausted daily quota, sync failure rate, and worker backlog growth.
- [ ] Add OpenTelemetry tracing dependencies and OTLP exporter configuration to API and worker.
- [ ] Configure API and worker deployments with service name, environment, and OTLP endpoint variables for Tempo.
- [ ] Propagate trace context through queued worker tasks where useful for long Strava sync workflows.
- [ ] Add spans around Strava HTTP calls, quota reservation, checkpoint persistence, activity import persistence, and task requeueing.
- [ ] Correlate structured logs with trace ids so Loki and Tempo can pivot between logs and traces.
- [ ] Add a Grafana trace-to-logs configuration if the current Loki/Tempo datasources do not already support it.

### Paused Sync Behavior

- [x] Stop treating stream-fetch rate limits as per-activity failures.
- [x] Mark rate-limited syncs as queued with a rider/admin-visible pause message.
- [x] Record an integration event when a sync pauses for provider quota.
- [x] Release the user activity import lock before waiting for provider quota.
- [x] Requeue paused Strava syncs with `scheduled_for` instead of sleeping in the worker.
- [x] Show current provider quota used and remaining values on `/admin/metrics` through app metrics stats.
- [ ] Add a distinct paused/waiting sync status or structured detail alongside queued/running/succeeded/failed.
- [ ] Record explicit resume events when a paused sync restarts.
- [ ] Record exhausted daily quota as a separate integration event from short-window pauses.
- [ ] Detect abandoned running sync checkpoints and resume or mark them recoverable.

### Durable Sync Checkpoints

- [ ] Decide whether checkpoints live in a Strava-specific table or a generic provider-sync table.
- [ ] Add checkpoint table with connection id, user id, provider, sync mode, status, current page/cursor, `after` timestamp, last activity marker, counters, latest discovered activity timestamp, next eligible run time, and message.
- [ ] Create checkpoint rows when initial, manual, or webhook-triggered syncs start.
- [ ] Update checkpoint state after each fetched page and after each processed activity.
- [ ] Resume from checkpoint after worker retry, process restart, deployment restart, or rate-limit pause.
- [ ] Make disconnect cancel or terminally mark active checkpoints.
- [ ] Keep partial imports valid through existing source correlation ids and deduplication.
- [ ] Finalize activity import batches in bounded chunks rather than only at the end of a long sync.

### Other External APIs

- [x] Audit current backend `reqwest` usage for external API/provider calls.
- [x] Treat Strava as the first quota-managed cloud provider integration.
- [ ] Decide whether user-supplied archive URL downloads need separate host/download safety limits rather than provider quota.
- [ ] Document how future provider clients should declare buckets, classify operations, reserve quota, and reconcile headers.
- [ ] Add a provider-client checklist to any future external integration spec before implementation.

### Tests And Verification

- [x] Add unit tests for local quota exhaustion.
- [x] Add unit tests for provider header reconciliation moving local counts forward.
- [x] Add unit tests for Strava `Retry-After` parsing.
- [x] Run full API tests after AppError and Strava sync behavior changes.
- [ ] Add tests for concurrent Postgres quota acquisition.
- [ ] Add tests for Strava `429` response handling and `retry_at` propagation.
- [ ] Add tests for paused sync requeueing with `scheduled_for`.
- [ ] Add tests for checkpoint resume after a paused sync.
- [ ] Add tests for disconnect during a paused checkpointed sync.
- [ ] Add tests for idempotent duplicate handling across checkpoint resume.

## Open Decisions

- Whether webhook subscription list calls should be classified as read requests or overall-only. Until proven otherwise, treat them as read plus overall.
- Whether sync checkpoints should live in a Strava-specific table or a generic provider-sync table designed for future integrations.
- Whether to expose paused-for-rate-limit state as a new `last_sync_status` value or as structured detail alongside the existing queued/running/succeeded/failed states.
