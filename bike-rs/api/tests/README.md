# Bike API integration tests

Run from the repository root:

```sh
mise run test:integration
```

`platform.rs` exercises the real Axum router, authentication extractor,
application handlers, and SeaORM queries. It checks seven reads: health, current
user, activity list/detail, segment list/detail, and race comparison. It also
checks invalid bearer rejection and preference validation, persistence, and
restoration. The fixture
contains two rides and two efforts on one segment. Assertions require the known
records, metrics, usable route coordinates, and both race efforts; empty or
unrelated responses cannot pass.

The suite creates an isolated in-memory SQLite database from the owning entities,
loads `fixtures/platform/read-models.sql`, and uses Tower requests in process.
It needs no running server, Docker, identity provider, Strava account, or separate
load-testing tool. It runs with ordinary `cargo test` and backend CI.

This verifies HTTP adaptation and the native data path on SQLite. PostgreSQL
migration behavior, production query plans, and resource use remain separate
checks. External authentication and provider calls use their owning fixture
suites. The owning component instructions define required verification.

`fixtures/platform/` also retains deterministic PostgreSQL scenario fixtures and
small synthetic uploads for focused UI/workflow checks. SQL scenario files should
only be applied to an explicitly selected disposable database. The integration
test's SQLite fixture does not use those PostgreSQL files.
