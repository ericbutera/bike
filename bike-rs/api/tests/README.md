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

`fixtures/platform/` retains the owning in-memory read fixture and small synthetic
uploads. Browser scenario data lives in
[`Playwright seed builders`](../../../bike-ui/tests/e2e/helpers/seeds.mjs), is
validated through current ORM models by `e2e-fixtures`, and is snapshotted after
current migrations. Browser tests restore data while services remain running.
The default browser suite excludes the job worker; worker E2E is a future opt-in.
