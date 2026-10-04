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

For a production smoke check, select an existing GPS activity and segment/effort IDs and
provide a short-lived authenticated test token in a private JSON file containing
`{"token":"<access-token>"}`:

```sh
BIKE_UI_URL=https://bike.example.com \
BIKE_API_URL=https://api.bike.example.com \
BIKE_TEST_AUTH_FILE=/private/path/test-auth.json \
BIKE_TEST_ACTIVITY_ID='<existing-gps-activity-id>' \
BIKE_TEST_SEGMENT_ID='<existing-segment-id>' \
BIKE_TEST_RACE_EFFORT_IDS='<effort-id>,<effort-id>' \
  mise run test:production
```

This journey reads real application data and renders through the production map
service. It does not load fixtures or edit production activities. Credentials
are attached only to the selected Bike UI and API origins; external basemaps
remain faked. Delete the private credential file after the run. Synthetic token
authentication does not verify the identity provider or Strava login flow.
Use the segment's initially selected efforts, which can contain up to three riders.

Other individual suites can run directly:

```sh
mise --cd bike-ui exec -- pnpm exec playwright test tests/e2e/auth-happy-path.spec.mjs
mise --cd bike-ui exec -- pnpm exec playwright test tests/e2e/activity-sync-fixture.spec.mjs
mise --cd bike-ui run test:e2e:heatmaps
```

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
