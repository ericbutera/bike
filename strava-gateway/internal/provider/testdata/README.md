# Strava provider fixtures

These are synthetic responses in the shapes consumed by the provider adapter,
with intentionally fake credentials and IDs. They are not live Strava captures.
Exchange, refresh, and athlete tests replace `http.Client.Transport`; they make
no provider requests and require no PostgreSQL or login.

`provider.OAuthAPI` and `provider.SyncAPI` separate the application from the HTTP
adapter. The callback functional test supplies an `OAuthAPI` fake, saves a
known OAuth state, and checks site-user binding plus the queued initial sync.
It uses a temporary schema in the explicitly selected test database and removes
that schema afterward. Run it independently with `TEST_DATABASE_URL` pointing
at a disposable PostgreSQL instance. Replace the example port `54321` with the
port reported by `mise exec -- docker compose port postgres 5432` from the gateway
directory:

```sh
mise exec -- go test ./internal/provider
TEST_DATABASE_URL=postgres://gateway:gateway_local_only@127.0.0.1:54321/gateway_test?sslmode=disable \
  mise exec -- go test ./internal/oauth -run TestCallbackBindsSiteUserAndQueuesInitialSyncWithFakeProvider -v
```

Ordinary gateway tests remain database-free unless that variable is explicitly
set. No complete live SSO -> Strava -> app flow is required.

## Real-ride sync and downstream checks (2026-10-02)

`strava-real-ride.json` takes its first eight coordinates/elevations from the
original `bike-rs/data/activities/Morning_Ride.gpx` (SHA-256
`aa865c827fd7d67b36bb28305ee62d14a19d654fb330080e69280b058b4a98df`).
The source GPX has no timestamps. IDs, start date, ten-second sample intervals,
and derived speeds are synthetic; cumulative distance is calculated from those
coordinates. This small sample covers an ordinary stored route, not a complete
ride or a recorded Strava response. The originals remain unchanged/ignored.

Rust owns the canonical fixture in `bike-rs/bike-core/testdata`; intentional
component build-context copies are registered in `docs/shared-assets.json` and
synchronized by `mise run assets:sync`.

Independent checks passed:

- Gateway `TestSyncFetchesListDetailAndStreamsWithRealRideFixture`: fake HTTP
  transport checks list pagination/window, bearer auth, detail, requested stream
  keys, cycling classification, and response preservation. No provider or DB.
- Rust `parses_original_ride_fixture`: parser preserves distance, time, and all
  eight original route points. `imports_original_ride_fixture_without_duplicate`
  also passed against a disposable database migrated by the Rust task, checking
  the owning user's stored ride, correlation, route, and repeated delivery.
- `playwright/activity-sync-fixture.spec.mjs`: the recorded run included the deployed
  Rust UI assets. Every application API response was stubbed; the fixture
  appears once in the list with distance, then detail shows the title and route
  map. This proves the display step independently, without live login, provider
  calls, or live-data mutation. Unexpected API requests are rejected.

Run only the relevant owning test above. For browser checks, use the existing
runner with available UI URLs, for example:

```sh
cd bike-ui
BIKE_UI_URL=https://bike.example.com \
  mise exec -- pnpm exec playwright test tests/e2e/activity-sync-fixture.spec.mjs
```
