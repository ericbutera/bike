# Strava cutover to Bike Rust

- Implement [the minimum Strava design](../specs/strava-integration.md).
- Rust API receives webhooks; existing Rust workers execute the DAGs.
- Remove the gateway, its worker, site delivery outbox, signed delivery endpoints,
  gateway client, images, CI checks and obsolete monitors.
- Keep the generated vendor client untouched. All Bike customization belongs
  in its wrapper and supported observing middleware.

## Cutover

- Build and validate the native API and worker images.
- Apply the native workflow schema migration.
- Stop the old gateway API and worker; do not transfer or replay their jobs,
  deliveries, checkpoints or retained artifacts.
- Preserve the registered vendor callback URLs and route them to Rust.
  These aliases receive native Strava callbacks, not signed gateway deliveries.
- Use the Strava application's credentials and subscription identity in Rust.
- Connect Strava through native OAuth; existing Bike activities remain in place.
- Enable native provider work after the API and worker are ready.
- Remove the old gateway deployments, services, migration job and monitors.
- Historical schema migrations stay append-only.

## Acceptance

- Verify actual deployed image identity, migration completion and readiness.
- Confirm accepted webhooks have source objects, correlated workflows and jobs.
- Initial signup/connection sync requests only the previous 15 days; verify the
  accepted cutoff on every listing page. Backfill uses the same 15-day window;
  incremental sync uses the saved offset.
- Sync and backfill expose stage outcomes and saved object paths.
- Find an activity/workflow ID in the Strava dashboard's Loki logs and Tempo
  traces; confirm trace-to-log navigation and selected-range aggregate counts.
- Verify native Strava imports in admin history, retained-source replay, active
  collection state and the filtered integration-event feed.
- Unchanged activities create no applied-change events or duplicate derived jobs.
- Repeated deletes are reported as already absent.
- A failed activity/stream request is visible and does not delete an activity.
- Interrupted workflows reuse saved pages, raw responses and transforms.
- Verify quota usage and scheduled resume times.
- Record local fixture evidence separately from live vendor and production results.

## Local verification

- Docker validation uses the repository's pinned mise tooling.
- The initial cutover baseline passed 368 Rust unit tests, including individual
  happy paths; updated PR coverage belongs to CI.
- `mise run test:strava:docker` passes native HTTP callbacks through persisted
  jobs and the real worker processor, using PostgreSQL and a fake Strava server.
- Verify create, metadata/privacy updates, delete, sync, backfill, disconnect and
  vendor revocation; assert retained DAG outputs, correlation and request counts.
- Regressions cover duplicate/concurrent delivery, interrupted stages, unchanged
  sync, failed reads, stale events, quota pauses and completed-job redelivery.
- SDK regeneration is byte-identical; applicable prek hooks pass without warnings.
- Companion infrastructure's `mise run check:bike` passes Go quality gates and
  Pulumi mock assertions for native settings and preserved callback routes.
- CI coverage handoff and cleanup pass. Coverage dependencies persist in the
  ref's instrumented target; native workspace cleanup prevents stale reports.
- A two-run cache regression verifies compiled dependency reuse and exclusion
  of removed source and test executables, in CI and pre-commit.
- Follow-up checks cover the 15-day cutoff, admin list/trace/replay/events,
  queued retry correlation exports, UI import/event components, and native
  dashboard Promtool queries. Replay makes no additional vendor requests.
- Production cutover on 2026-10-11 passed migrations and readiness for all four
  tested image digests; retired gateway services were removed.
- Eric's production sync completed with two activities inspected, one applied,
  one unsupported activity excluded and no failures. Native token refresh worked.
- The existing callback URL passed its verification handshake. Two identical
  manually submitted update callbacks for a real activity produced one workflow
  and one set of stage events. A fresh vendor-originated delivery and the full
  create/update/delete vendor lifecycle remain unverified.
- Native admin import history and import trace endpoints returned the activity;
  integration history populated workflow stages. Activity-ID searches returned
  its production logs in Loki and traces in Tempo.
