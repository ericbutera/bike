# Bike browser tests

Browser regressions belong to the UI. They use its package dependencies and
Playwright configuration, while backend HTTP checks live in
[`bike-rs/api/tests`](../../../bike-rs/api/tests/README.md).

## Setup and focused checks

From the repository root, start Bike with `mise run compose:up`, then install
the UI dependencies and browser:

```sh
mise --cd bike-ui run test:e2e:install
mise --cd bike-ui run test:e2e
```

The default check follows activity list/detail, segment list/detail, and race
playback. It expects a prepared development database containing activities
`1109,1110`, segment `5`, and efforts `5895,5912`. The PostgreSQL fixture is
[`four-views.sql`](../../../bike-rs/api/tests/fixtures/platform/four-views.sql);
it rejects populated databases. Use a disposable database and a UI/API configured
for it when applying scenario fixtures.

For another prepared dataset, set `BIKE_TEST_ACTIVITY_ID`,
`BIKE_TEST_SEGMENT_ID`, and `BIKE_TEST_RACE_EFFORT_IDS`. `BIKE_UI_URL` overrides
the UI address, which defaults to `http://localhost:3001`. Browser tests do not
start or stop the application.

The browser image runs independently of k6:

```sh
mise run e2e:build
BIKE_UI_URL=http://host.docker.internal:3001 mise run e2e:run
```

The image contains Chromium and the locked UI test dependencies. Its default
entrypoint runs the activity/segment/race journey. Pass a spec path to
`mise run e2e:run -- <spec>` for another slice. CI can run the same image
against a prepared test stack.
Set URLs and fixture configuration explicitly; it never uses Kubernetes or
starts the application. Use disposable fixtures for suites that mutate data.

The [k6 image](../../../integration-tests/README.md) only checks availability.
It does not run the browser journey or inspect activities, segments, or races.

For an internal browser run, set `BIKE_SYNTHETIC_KEY`, `BIKE_UI_URL`,
`BIKE_API_URL` (including `/api`), and `BIKE_PUBLIC_URL`, then run the owning
`test:e2e` task. The runner discovers the scenario's activity, segment, and
effort IDs and verifies public credential rejection before navigating. It
redirects the deployed UI's public API transport to the existing internal API;
application responses, SQL, and owned rendering remain real. External basemaps
remain fixtures. Synthetic runs disable credential-bearing traces and do not
verify live identity-provider or Strava authorization.

Other individual suites can run directly:

```sh
mise --cd bike-ui exec -- pnpm exec playwright test tests/e2e/auth-happy-path.spec.mjs
mise --cd bike-ui exec -- pnpm exec playwright test tests/e2e/activity-sync-fixture.spec.mjs
mise --cd bike-ui run test:e2e:heatmaps
```

`mise --cd bike-ui run test:e2e:diagrams` verifies Mermaid and patched KaTeX
rendering in a real browser using the locked package assets. It uses a standalone
HTML fixture, requires no running Bike services, and rejects browser warnings
and errors. The UI `check` task includes this regression before image builds.

The auth and activity-sync fixtures stub application API responses and external
map data. They verify UI behavior independently of live auth, provider, and
database behavior. The connected default journey uses real product APIs and the
owned PNG renderer while faking external basemaps. Heatmap verification requires
an enabled flag and prepared activity projections.

`mise --cd bike-ui run test:e2e:all` runs the broad retained regressions on demand;
some require additional scenario fixtures or modify disposable test data.
Run only the owning slice when checking an ordinary change.

## Artifacts and visual baselines

Playwright stores failure traces and results under `.artifacts/playwright/`.
Canonical screenshots are checked in under `snapshots/`. The `rust` screenshot
prefix identifies the existing baseline set. Update visual baselines only after
reviewing the rendered result; screenshot matching does not verify provider or
database behavior.
