# Bike browser tests

Browser regressions belong to the UI and use its locked Playwright dependencies.
Backend HTTP integration tests live in
[`bike-rs/api/tests`](../../../bike-rs/api/tests/README.md).

## Disposable suite

CI separates `test-ui-unit` from `test-ui-e2e`. The UI `check` task owns unit
tests, lint, types, formatting, client freshness, and audit. Browser tests run
through the owning `test:e2e:disposable` task before any release image builds.
PRs and main use the same tasks and workflow; only main proceeds to deployment.

The disposable browser runner needs an empty PostgreSQL service with database
and role `bike_e2e`, password `bike-e2e-only`, on port 5432. It accepts only the
CI hostname `ci-postgres` or a loopback hostname. It refuses other databases,
roles, addresses, and connection options before resetting anything.

```sh
BIKE_E2E_DATABASE_URL=postgres://bike_e2e:bike-e2e-only@localhost:5432/bike_e2e mise run ci:ui:e2e
```

The task compiles the branch API, worker, and fixture tool. Playwright starts
the branch UI and owning map renderer. Each test recreates the database through
the existing migrations, applies `four-views.sql` and its scenario-specific SQL
fixtures, copies uploads into a temporary directory, and starts a fresh API.
Worker scenarios also start the real worker. Teardown stops these processes and
removes test uploads. CI destroys the database service with the workflow and
does not mount a database volume or supply production credentials.

Application responses, persistence, and owned rendering remain real for connected
scenarios. Auth/provider boundary regressions retain their explicit fakes;
external basemaps use fixtures. This does not establish live identity-provider
or Strava authorization.

`test:e2e` discovers the entire browser suite, including diagram rendering,
auth, imports, activity/segment actions, route matrices, and worker scenarios.
The retained visual baseline suite remains opt-in with `PLAYWRIGHT_VISUAL=1`
because its screenshots require the matching reviewed dataset. No CI command
hardcodes a single spec.

## Existing development stacks and focused checks

To use a prepared development stack instead, start Bike with
`mise run compose:up`, install dependencies and Chromium, and run the owning task:

```sh
mise --cd bike-ui run test:e2e:install
mise --cd bike-ui run test:e2e
```

Without `BIKE_E2E_DISPOSABLE=1`, browser tests do not start, stop, or reset
services. Use a disposable database for scenarios that change data and apply
their SQL fixtures explicitly. `BIKE_UI_URL`, `BIKE_API_URL`, and the
`BIKE_TEST_*` fixture variables select the existing stack and expected records.

Focused checks remain available:

```sh
mise --cd bike-ui run test:e2e:diagrams
mise --cd bike-ui run test:e2e:heatmaps
mise --cd bike-ui exec -- pnpm exec playwright test tests/e2e/auth-happy-path.spec.mjs
```

The standalone browser image uses the same Playwright configuration. Its default
entrypoint runs the connected activity/segment/race journey against an explicitly
configured stack; it does not start services or use Kubernetes credentials.

## Artifacts and visual baselines

Playwright stores traces and test results under `.artifacts/playwright/`.
Canonical screenshots are committed under `snapshots/`. Review rendered results
before updating baselines; screenshot matching does not verify provider or
database behavior.
