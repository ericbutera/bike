# Strava Integration Specification

Task status is maintained only in [the Bike backlog](../../../docs/TODO.md).
The Rust-provider design below is a legacy reference. Production fetching,
quota, checkpoints, and delivery are owned by the
[shared gateway](../../../strava-gateway/docs/specs/strava-fanout.md).
The old implementation checklist is preserved at `200921f` in Git and its
open requests are accounted for in the backlog source inventory.

Strava is Bike's first live cloud activity integration. It must remain isolated behind a single Bike-owned client boundary, respect Strava's app-wide limits, and treat long-running imports as resumable work instead of all-or-nothing worker tasks.

## Product Intent

Connecting Strava should feel simple for riders while keeping provider behavior transparent to Bike operators. Sync status, rate-limit pauses, partial progress, webhook handling, and provider errors should be visible through integration events and admin tooling.

Normal usage is at most about 10 activities a day. Occasional historical imports use the existing paging and quota pauses. Expand recovery or concurrency tests for an observed failure or measured need.

## Legacy Rust provider flow

In the pre-gateway design, outbound Strava HTTP calls were made through `StravaApiClient` in `api/src/strava_client.rs`. The worker does not call Strava directly; the `strava_sync` processor delegates to `api::strava::process_strava_sync`.

The client is separated from the broader Strava service module. It builds request URLs, sends `reqwest` calls, parses JSON, and converts HTTP failures to `AppError`. Strava calls reserve provider quota before sending, reconcile Strava rate-limit headers after responses, emit OpenTelemetry spans, and treat `429 Too Many Requests` as a structured retryable pause.

Strava activity kind is provider metadata, not Bike's training/race classification. Bike imports only Strava cycling-family activities into the app's ride domain: `Ride`, `VirtualRide`, `MountainBikeRide`, `GravelRide`, `EBikeRide`, and `EMountainBikeRide` normalize to stored `sport = ride`; non-cycling kinds such as `Run`, `TrailRun`, `Walk`, and `Hike` are skipped and must not contribute to segment, fitness, training, or report analytics. The Bike `activity_type` remains app-owned (`training` or `race`) and is inferred from the title/filename or changed by the user, not from Strava road/gravel/mountain labels.

The current sync shape is:

- refresh the access token if needed;
- list athlete activities in pages of up to 100;
- skip non-cycling activities before requesting streams;
- request streams for each supported cycling activity;
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

## Provider Client Pattern

Future provider clients should follow the Strava boundary instead of adding one-off HTTP behavior in service code.

Each provider client should:

- declare named quota buckets with provider, bucket, limit, window, and request units;
- classify every operation into the buckets it consumes before the request is sent;
- reserve all required buckets transactionally through the provider rate limiter;
- reconcile provider response headers back into local bucket state when the provider reports stricter usage or changed limits;
- convert remote `429` responses and retry headers into structured retryable errors with `retry_at`;
- emit provider request and rate-limit metrics with stable provider, operation, request class, bucket, and status labels;
- expose only high-level domain methods to business flows.

User-supplied archive downloads are not provider API calls. They may need separate host allow/block rules, redirect limits, content-length limits, timeout limits, and download concurrency controls, but they should not consume provider quota buckets.

For a future provider, define its HTTP ownership and actual quota requirements, then reuse the existing provider tests for the behavior being added. The current backlog determines any further verification scope.

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

Most Strava provider metrics are emitted by the worker, because manual and webhook sync tasks do the outbound Strava calls there. The worker scrape endpoint must therefore include provider metrics in addition to generic task metrics.

The current Prometheus surface includes `bike_provider_api_requests_total`, `bike_provider_api_requests_15_minutes_total`, `bike_provider_api_requests_daily_total`, `bike_provider_rate_limit_pauses_total`, `bike_provider_rate_limit_limit`, `bike_provider_rate_limit_used`, `bike_provider_rate_limit_remaining`, `bike_provider_rate_limit_reset_timestamp_seconds`, and `bike_strava_connected_athletes`. The first Bike Grafana dashboard charts Strava request rate, non-2xx responses, 15-minute and daily request counts from counters, rate-limit pauses, remaining quota, quota usage, reset countdowns, connected athletes, and `strava_sync` task activity.

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
- Current Strava service: `api/src/strava.rs`
- Strava API client: `api/src/strava_client.rs`
- Strava provider payload parsing: `api/src/strava_provider_payload.rs`
- Provider rate limiter: `api/src/provider_rate_limit.rs`
- Provider rate-limit entity: `api/src/entities/provider_rate_limit_buckets.rs`
- Provider rate-limit migration: `migration/src/m20260912_000001_create_provider_rate_limit_buckets.rs`
- Worker processor: `worker/src/tasks/processors/strava_sync.rs`
- Task enqueueing: `api/src/tasks/adapter.rs`
- Activity import pipeline: `api/src/activity_import_pipeline.rs`
- Integration events: `api/src/integration_events.rs`
- Prometheus metrics: `api/src/metrics.rs`
- OpenTelemetry initialization and trace propagation: `api/src/observability.rs`
- Observability metric backlog: `docs/observability-metrics.md`
- Production API `ServiceMonitor`: `../../../pulumi-iac/bike/servicemonitor-bike-api.yaml`
- Production worker `ServiceMonitor`: `../../../pulumi-iac/bike/servicemonitor-bike-worker.yaml`
- Bike Grafana dashboard: `../../../pulumi-iac/bike/bike-grafana-dashboard.yaml`
- Grafana Loki/Tempo datasource provisioning: [infrastructure repository](https://github.com/ericbutera/pulumi-iac)

## Implementation and verification status

Use [the backlog](../../../docs/TODO.md) for current status. Gateway quota/provider and
checkpoint/lease verification is recorded as STRAVA01–08; public metrics
isolation is STRAVA14. These completed items do not require duplicate Rust tests
or a second checkpoint table.

Today's provider checks are STRAVA16 and STRAVA11: verify exchange/refresh and
sync through provider seams with supported response fixtures, then verify a
derived real-ride artifact and list/detail independently. Prefer fakes and the
existing Playwright harness; a full live SSO/provider chain is not required. The wider lifecycle,
outage/replay, and rollback scenarios are deferred as STRAVA09, STRAVA10, and
STRAVA13. Provider-alert follow-up is STRAVA15; large-batch and archive-limit
work is LATER08/LATER06. Local database/file recovery belongs to REC08/REC09.

Run the existing owning tests when changing provider behavior; add a regression
for a demonstrated gap. The historical concurrency, pause/disconnect, and
resume matrices are not extra acceptance gates for today's work.
