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

## Ride fixture provenance

`strava-real-ride.json` takes its first eight coordinates/elevations from the
original `bike-rs/data/activities/Morning_Ride.gpx` (SHA-256
`aa865c827fd7d67b36bb28305ee62d14a19d654fb330080e69280b058b4a98df`).
The source GPX has no timestamps. IDs, start date, ten-second sample intervals,
and derived speeds are synthetic; cumulative distance is calculated from those
coordinates. This small sample covers an ordinary stored route, not a complete
ride or a recorded Strava response. The originals remain unchanged/ignored.

Rust owns the canonical fixture in `bike-rs/bike-core/testdata`; intentional
component build-context copies are synchronized by `mise run assets:sync` and
compared by `mise run contracts:check`.

The owning gateway tests use fake transport for list/detail/stream responses.
Rust tests consume the same sample at parsing and import boundaries. The UI's
`activity-sync-fixture.spec.mjs` uses fake API responses to verify presentation.
These checks make no live provider or login claim.
