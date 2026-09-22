# Bike State And Service Boundary Plan

## Summary

Treat Bike as a coordinated Rust application with two binaries, not as microservices. Do not add gRPC now. The real contract is Postgres schema plus task payloads; the real drift fix is shared domain code, immutable same-commit releases, explicit migration orchestration, and backward-compatible schema changes.

## Progress Checklist

- [x] Release guardrails prevent API/worker tag drift.
- [ ] Worker no longer depends on the `api` crate.
- [ ] Shared `bike-core` crate owns entities, services, durable job payloads, and shared domain helpers.
  - [x] Shared config, observability, and database connection helpers live in `bike-core`.
  - [x] Durable job payloads and queue facade live in `bike-core::jobs`; worker processors stay in `worker`.
  - [x] SeaORM entities live in `bike-core::entities`; `api::entities` is a compatibility re-export.
  - [x] Provider API metrics live in `bike-core::provider_metrics`; API keeps only HTTP/database metrics.
  - [x] Shared activity import lock policy lives in `bike-core::activity_import_lock`; API maps it to HTTP errors.
  - [x] XC goal backfill state and queue coordination live in `bike-core::xc_goal_backfill`; API maps it to HTTP errors.
  - [x] Training analysis backfill/rebuild workflows live in `bike-core::activity_training_analysis`; API keeps a compatibility re-export.
  - [x] Fitness freshness, segment analytics, and activity analytics workflows live in `bike-core::analytics`; API keeps a compatibility re-export.
  - [ ] Activity import lifecycle, archive import, Strava sync, and segment regeneration workflows move behind `bike-core` boundaries.
  - [ ] Domain services/workflows move behind `bike-core` boundaries.
- [ ] API is only the HTTP adapter.
- [ ] Worker is only the task adapter.
- [ ] Migrations run as an explicit release step, not implicitly from API startup.
- [ ] API and worker both enforce a startup schema guard.
- [x] Durable task payload compatibility is covered by tests.
- [ ] Architecture checks prevent `worker -> api` dependencies from returning.
- [ ] Full workspace tests and Clippy pass.

Current progress: `bike-core` has been introduced for shared config, observability, database connection setup, durable job contracts, storage JSON value types, SeaORM entities, provider API metrics, activity import locks, XC goal backfill coordination, training-analysis cache workflows, and analytics rebuild workflows. Worker processors stay in the `worker` crate; API keeps compatibility re-exports during the broader boundary migration.

## Key Changes

- Add a shared workspace crate, `bike-core`, and move database-facing domain code there: SeaORM entities, entity query helpers, durable job payloads/queue helpers, domain services, shared errors, config pieces used by both binaries, metrics/observability primitives that are not HTTP-specific.
- Keep `api` as the HTTP adapter: auth extraction, request/response DTOs, OpenAPI, controllers, and API-only storage wiring. Controllers stay thin and call `bike-core` services.
- Keep `worker` as the background adapter: task processor registration, payload decoding, retry/error adaptation, and calls into `bike-core` services. `worker` must no longer depend on the `api` crate.
- Keep `migration` append-only and separate. Postgres remains the source of durable truth; SeaORM entities mirror tables, while services own workflow/business policy.

## State And Compatibility

- Move migrations out of API startup into a release migration step or Kubernetes Job built from the same commit as API/worker.
- Add a startup schema guard used by both API and worker: fail if the DB has not applied the minimum migration required by the binary; allow newer additive schemas by default.
- Use expand-and-contract for breaking schema changes: add nullable/additive schema first, dual-read/write where needed, backfill, deploy all binaries, then remove old columns/paths in a later release.
- Version durable task payloads. Existing payloads deserialize as v1; breaking task changes use either backward-compatible serde defaults or a new task type name until the queue drains.
- Enforce idempotent service commands and transactions around shared mutable state, especially imports, analytics rebuilds, provider rate limits, and task finalization.

## Release Guardrails

- Replace separate deploy drift for API/worker with one immutable release tag, for example `bike:appTag`, used by both `bike-api` and `bike-worker` images. Stop using `latest` for either.
- Annotate both deployments with the same release metadata: git SHA, app tag, and required schema migration.
- Add a Pulumi validation/test that fails when API and worker tags differ.
- Add an architecture check that fails if `worker` depends on `api` or imports `api::...`.

## Test Plan

- `mise exec -- cargo test --workspace`
- `mise exec -- cargo clippy --workspace --all-targets --all-features -- -D warnings`
- Add serde compatibility tests for queued task payloads.
- Add migration/schema-guard tests for “DB behind” and “DB already ahead with additive migrations.”
- Add Pulumi tests for shared API/worker release tag and no `latest`.
- Add a focused architecture test or script checking crate boundaries.

## Assumptions

- Bike remains one repo, one Postgres database, and one coordinated deployment pipeline.
- gRPC is deferred unless Bike later needs independently owned services with no shared DB access.
- Runtime feature removal must preserve historical migrations and migration registration.
