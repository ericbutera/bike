# Bike browser tests

Browser regressions belong to the UI. They use its package dependencies and
Playwright configuration, while backend HTTP checks live in
[`bike-rs/api/tests`](../../../bike-rs/api/tests/README.md).

## Planned disposable runner (TEST11)

- The [E2E plan](../../../docs/E2E-TODO.md) defines a proposed root
  `mise run e2e` command for the required suite and an explicit spec filter for
  focused runs. That command is not implemented yet; browser E2E remains excluded
  from CI until its runtime and acceptance checks are verified.
- Each invocation will own a disposable Compose project, using shared runtime
  definitions with small development/test overrides. Development services and
  persistent volumes are independent of the test environment.
- Local preparation will build or reuse cached application/test images; CI will
  supply the revision's already-built images to the same owning task. Actual test
  execution will run release services without source mounts or package installs.
- Connected attempts will clone a small migrated baseline, apply explicitly
  selected scenario SQL, restore uploads/caches, and start the required services.
  Queue-display scenarios omit the worker; processing scenarios wait for its
  persisted results. Connected writes/admin actions use normal authentication.
- Browser regressions with mocked application APIs will keep their own browser
  isolation and required services. Standalone diagrams will need no Bike stack.
  External provider responses will remain controlled at browser and server seams.
- The default will include all declared required coverage. Missing connected
  fixtures will fail setup; optional visual comparisons will be reported
  separately. Failure artifacts will be exported before resource cleanup.

## Current runner: manually prepared environment

The existing commands require an explicitly prepared disposable UI/API environment
and scenario data. They do not provision, reset, or stop application services.
Install the UI dependencies and browser, then select the prepared UI URL:

```sh
mise --cd bike-ui run test:e2e:install
BIKE_UI_URL="${BIKE_UI_URL:?Set the prepared disposable UI URL}" mise --cd bike-ui run test:e2e
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
BIKE_UI_URL="${BIKE_UI_URL:?Set a disposable UI URL reachable from Docker}" mise run e2e:run
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
mise --cd bike-ui exec -- pnpm exec playwright test tests/e2e/map-rendering.spec.mjs
mise --cd bike-ui exec -- pnpm exec playwright test tests/e2e/heatmap-controls.spec.mjs
mise --cd bike-ui run test:e2e:heatmaps
```

`mise --cd bike-ui run test:e2e:diagrams` verifies Mermaid and patched KaTeX
rendering in a real browser using the locked package assets. It uses a standalone
HTML fixture, requires no running Bike services, and rejects browser warnings
and errors. Browser E2E is temporarily excluded from CI and the UI `check` task.
The [E2E plan](../../../docs/E2E-TODO.md) defines the work needed before restoring
a separate containerized browser gate.

The heatmap-controls suite needs a running UI, but fixtures replace authentication,
API responses, basemap styles, and heatmap tiles. It runs the real MapLibre worker,
browser geolocation with a fixed position, and the bundled state-boundary lookup.
Desktop and mobile checks cover automatic browser location and saved cameras, Region/Full/GPS actions,
toolbar and navigation order, Help, filter retention, and console diagnostics.
Delayed-route checks resize the loading map, recover the old `0,0,2` placeholder
URL with location permission denied, and verify local framing survives reload.
An automatic-location check centers on a fixed browser position before route
locations arrive and preserves that view when they finish loading.
The owning heatmaps task also includes the existing connected tile/filter suite;
those connected checks require a prepared API and authenticated session.

The auth and activity-sync fixtures stub application API responses and external
map data. They verify UI behavior independently of live auth, provider, and
database behavior. The connected default journey uses real product APIs and the
owned PNG renderer while faking external basemaps. Heatmap verification requires
an enabled flag and prepared activity projections.

The map-rendering regression uses fixture account/activity responses and a
GeoJSON basemap with the real bundled MapLibre worker. Run it against a built UI
to verify that both heatmap and activity maps paint their basemap; it rejects
browser warnings and errors and requires no production account.

`mise --cd bike-ui run test:e2e:all` runs the broad retained regressions on demand;
some require additional scenario fixtures or modify disposable test data.
Run only the owning slice when checking an ordinary change.

## Artifacts and visual baselines

Playwright stores failure traces and results under `.artifacts/playwright/`.
Canonical screenshots are checked in under `snapshots/`. The `rust` screenshot
prefix identifies the existing baseline set. Update visual baselines only after
reviewing the rendered result; screenshot matching does not verify provider or
database behavior.
