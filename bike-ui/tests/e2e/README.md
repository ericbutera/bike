# Bike browser tests

Browser regressions belong to the UI and use its locked Playwright dependencies.
Backend HTTP and database regressions remain in
[`bike-rs/api/tests`](../../../bike-rs/api/tests/README.md).
[TEST11](../../../docs/E2E-TODO.md) tracks implementation and runtime acceptance;
configured CI steps alone do not establish a verified release gate.

## Disposable local runs

Install the root mise tools and have a reachable Docker engine with Compose.
Each run records the engine's native metadata alongside its test artifacts.
Prepare cached release images separately from test execution:

```sh
mise install
mise run e2e:prepare
mise run e2e
mise run e2e 'activity-import-result|warm-reset'
mise run e2e 'activity-list|segment-detail|race-viewer'
mise run e2e 'auth-happy-path|heatmap-controls'
PLAYWRIGHT_VISUAL=1 mise run e2e frontend-visual
```

- The default selects all required specs, including connected, API-mocked, and
  standalone diagram coverage. Screenshot comparisons run only when explicitly
  selected with `PLAYWRIGHT_VISUAL=1`; reviewed baselines remain in `snapshots/`.
- Each run resolves its image inputs once and owns a unique Compose project,
  network, PostgreSQL volume, uploads, renderer styles, and cache. It exposes no
  host ports and shares no development data or source mounts.
- `compose.runtime.yaml` owns the shared release-service definitions.
  `compose.dev.yaml` retains development builds, mounts, ports, and persistence;
  `compose.e2e.yaml` contains only test differences. Production remains in Pulumi.
- Services start once. Playwright defines baseline/scenario data in
  [`helpers/seeds.mjs`](helpers/seeds.mjs); the prepared API image's fixture CLI
  validates records through current SeaORM models. Migrations and scenario
  seeding happen once per run; native `pg_dump` captures fresh data snapshots.
- Connected attempts and retries restore rows and sequences in one transaction,
  retaining the database, schema, pooled connections, and application processes.
  Uploads and image caches are restored through run-owned volumes. Fresh normal
  sessions and the existing flag-update API keep authentication/caches coherent.
  Snapshot hashes, container IDs/start times/restart counts, and database object
  IDs are checked after every attempt. One Playwright worker owns the database.
- The default browser gate excludes the job worker. Browser tests verify real
  queue submission and seeded persisted results; they never wait for job
  execution. Heatmap fixtures reuse the owning projection builder during seeding.
  A future worker E2E suite must be independently selected and opt-in.
- Execution installs no packages and compiles no application code. To select
  existing release images, set `BIKE_API_IMAGE`,
  `BIKE_UI_IMAGE`, `BIKE_RENDERER_IMAGE`, and `BIKE_E2E_IMAGE` explicitly.
  Defaults are the `:review` images produced by `e2e:prepare`; PostgreSQL is pinned
  by mise. Local images are local evidence, not production release verification.
- The revised default passed 74 tests locally in 3.4 minutes on Linux arm64.
  Initial migration/seed/snapshot setup took 10 seconds; database restoration
  had a 90 ms median, and complete connected-attempt setup had a 268 ms median.
  Counts, raw timings, runtime identities, and revised CI acceptance are recorded
  in [TEST11](../../../docs/E2E-TODO.md#implementation-evidence).

## Scenario and coverage inventory

All connected scenarios use PostgreSQL, the real API/UI/renderer, restored public
GPX uploads, normal session authentication, and a freshly restored Playwright
scenario snapshot. Activity, segment, and task IDs can recur across independent
attempts. Data snapshots are generated for the current migrations and owning
models, rather than committed SQL datasets or database dumps.

| Scenario/project    | Specs                                                                                                                                             | Additional data/services and mutations                                                                      |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| `baseline`          | `activity-actions`, `activity-detail`, `activity-edit`, `activity-import-duplicate`, `activity-import-new`, `activity-lifecycle`, `activity-list` | Activity reads, edits, upload/queue operations, deletion; no worker                                         |
| `baseline`          | `segment-actions`, `segment-builder`, `segment-delete`, `segment-detail`, `segment-import`, `segment-list`, `race-viewer`, `xc-target`            | Segment/effort reads, construction/import, edits/deletion, race playback                                    |
| `baseline`          | `admin-tasks`, `admin-users`, `archive-import-queue`, `training-reports`, `frontend-matrix`, `platform-synthetic`                                 | Admin/task/user/report reads and actions, archive queue, connected read journey                             |
| `baseline`          | `four-view-states`                                                                                                                                | Real successful reads and missing-record API checks; loading/empty/error UI responses explicitly controlled |
| `baseline`          | `warm-reset`                                                                                                                                      | Real deletion followed by data restoration and live pooled reads; unchanged database/service identities     |
| `laps`              | `activity-laps`                                                                                                                                   | Seeded lap summaries                                                                                        |
| `climb`             | `activity-climb`                                                                                                                                  | Seeded sustained-climb geometry                                                                             |
| `zones`             | `activity-zone-without-chart`                                                                                                                     | Zone rollup with empty chart samples                                                                        |
| `eleven-efforts`    | `segment-page-two-race`                                                                                                                           | Eleven efforts and matching summaries                                                                       |
| `partial-race`      | `segment-partial-race`                                                                                                                            | Sparse route points and matching effort indexes                                                             |
| `task-cancel`       | `admin-task-cancel`                                                                                                                               | Persisted processing task; no worker                                                                        |
| `manual-processing` | `admin-manual-processing`                                                                                                                         | Dedicated users for admin queue operations                                                                  |
| `archive-states`    | `archive-import-states`                                                                                                                           | Seeded queued/running/succeeded/failed progress                                                             |
| `upload-result`     | `activity-import-result`                                                                                                                          | Seeded processed import linked to its visible activity                                                      |
| `heatmap`           | `heatmap`                                                                                                                                         | Persisted projections seeded through the owning builder; real tile/filter/zoom checks                       |
| `mocked`            | `auth-happy-path`, `activity-sync-fixture`, `map-rendering`, `heatmap-controls`, `frontend-auth-state`                                            | UI with explicit application API fakes; fresh browser contexts; no claim of database/provider behavior      |
| `standalone`        | `diagram-rendering`                                                                                                                               | Bundled Mermaid/KaTeX assets in standalone HTML; no running Bike services                                   |
| `visual` (optional) | `frontend-visual`                                                                                                                                 | Fresh baseline and explicit reviewed screenshot comparison                                                  |

- Browser tiles use a shared external-provider fake. Renderer processes receive
  a synthetic style through their own volume, so browser interception is not
  mistaken for renderer isolation. No live SSO or Strava authorization is required.
- Archive queue tests submit a controlled local URL through the real API.
  Queued/progress/success/failure display uses explicit persisted fixtures.
  Job execution is covered by owning native suites; worker E2E remains deferred.
- Explicit error-state browser fakes construct a `Response` at the fetch boundary.
  They test the client's failure behavior without deliberately generating
  Chromium failed-resource errors. Unexpected warnings, console errors, and
  uncaught exceptions in every browser context fail the shared fixture.
  Negative cases declare their exact expected API error messages and counts
  through a Playwright option. The complete diagnostic list must match those
  assertions; warnings, page exceptions, and additional errors always fail.
  Setup, connected attempts, and mocked cases attach service logs and fail on
  service warnings, notices, fatal errors, or unhandled server errors as well.
- Missing required fixtures and feature flags fail setup. No required spec uses
  a conditional skip; visual comparisons remain a separate opt-in project.
- The container supplies Xvfb and Mesa OpenGL for Chromium's WebGL maps, including
  pixel assertions, without a host GPU. Browser diagnostics remain unfiltered.
  Both disposable runs and the external containerized runner use this backend.
  See [Playwright's Xvfb guidance](https://playwright.dev/docs/ci#running-headed)
  and [Chromium's rendering configuration](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/docs/gpu/using-gpu-hardware-in-headless-chrome.md).
  Chromium treats only the isolated UI origin as secure so native geolocation
  permissions work with the test environment's HTTP URLs.
  Mise keeps Buildx state in the ignored `.cache/docker-buildx` directory.

## CI and artifacts

- Woodpecker runs checks, then all release/browser/engine image builds, then
  `mise run ci:e2e`, which calls the same `mise run e2e` lifecycle. It uses a
  job-local Docker daemon with TLS client certificates on the disposable
  workspace. The Bike-only permission is owned by the separately maintained
  infrastructure configuration.
- PR tests receive no deployment secrets. Main deployment depends on E2E success
  and consumes the recorded registry digests and source revision. The companion
  IaC change must merge before that deployment task is enabled on main.
- Local artifacts live in `.artifacts/e2e/<project>/`: HTML/JSON reports, failure
  traces/screenshots, service logs, image/platform identities, source revision,
  worktree patch, and cleanup inventory. The runner copies artifacts before
  removing project containers, networks, and volumes. Cleanup/export failures
  fail the run, as do startup, migration, fixture, readiness, and browser failures.
- CI retains those files on the existing cache PVC at
  `/cache/bike/e2e/<pipeline-number>/<project>/`, printed in the job log; the
  ephemeral checkout is not the only copy. Runs older than seven days are
  removed by the owning CI task. Docker TLS certificates are not copied there.

## Explicit external monitoring

`mise run e2e:run` retains the separate read-only platform browser monitor against
explicit URLs. Set `BIKE_SYNTHETIC_KEY`, `BIKE_UI_URL`, `BIKE_API_URL` (including
`/api`), and `BIKE_PUBLIC_URL`. It discovers the production fixture identifiers,
checks public credential rejection, and keeps credential-bearing traces disabled.
It does not provision or reset production data, verify live OAuth/Strava, or
replace the disposable suite. The [k6 monitor](../../../integration-tests/README.md)
continues to establish availability separately.

For a standalone local diagram check, use
`mise --cd bike-ui run test:e2e:diagrams`. Other direct UI Playwright tasks require
an explicitly prepared external environment and scenario; prefer `mise run e2e`
for disposable connected coverage.
