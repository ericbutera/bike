# Production synthetic checks

The production synthetic runner is k6. It runs one small journey with one
virtual user after deployment; it is a correctness check rather than a load test.

With mise installed and kubectl connected to the production cluster, run:

```sh
mise install
mise run test:production
```

The task reads the managed synthetic credential and public origin from Kubernetes,
opens temporary localhost forwards to the existing API and UI, and closes them
after the run. It does not print or save credentials. `BIKE_KUBE_NAMESPACE`
selects another prepared namespace. No user, activity, segment, or effort IDs
need to be supplied.

Woodpecker runs the same [`production.js`](production.js) directly against the
ClusterIP services after a trusted main-branch deployment. Its synthetic secret
is unavailable to pull-request jobs. Callers already inside the cluster can set
`BIKE_SYNTHETIC_KEY`, `BIKE_API_URL` (including `/api`), `BIKE_UI_URL`, and
`BIKE_PUBLIC_URL` and run the same mise task.

The API provisions `platform-smoke/v1` once when synthetic authentication is
enabled. Its disabled, non-admin owner cannot sign in or obtain ordinary
sessions. The provisioner allocates database IDs normally and records them in a
scenario manifest. It never adopts an existing account, resets sequences, loads
the empty-database development fixture, or edits another owner's records.

The checks require meaningful activity and segment metrics, route coordinates,
both discovered race efforts, and a real PNG from the UI proxy and owned renderer.
They also require public API and UI image routes to reject the credential, and
internal routes to reject account mutation, logout, and administration. Public
UI and API availability are checked separately without credentials. Every check
must pass; empty lists and HTTP 200 alone cannot establish success.

These HTTP checks do not verify browser interaction, live SSO, or Strava login.
The separate [Playwright suite](../bike-ui/tests/e2e/README.md) owns browser
activity navigation, map display, and race playback. The [authentication design](../docs/specs/production-synthetics.md)
describes the internal boundary. Remaining work is tracked only in
[`docs/TODO.md`](../docs/TODO.md).
