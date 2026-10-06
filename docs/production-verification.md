# Production verification

Historical verification date: 2026-10-04.

The port-forward wrappers and broad k6 journey described below have been retired.
Current runners are documented in [the synthetic guide](../integration-tests/README.md)
and [the browser guide](../bike-ui/tests/e2e/README.md). These historical results
do not establish verification of the replacement images.

Verification date: 2026-10-04. This record covers the restored Bike repository,
its component release workflows, and the running production services.

## Development checks

| Check                                                           | Result                                                                                                                                                |
| --------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `mise run lint`                                                 | Rust formatting and strict Clippy, UI ESLint, gateway Go vet, and renderer formatting pass.                                                           |
| `mise run hooks:install` / `mise run hooks:check`               | The root Git hook is installed; all eight prek checks pass against tracked files. The installed hook also passes during commits.                      |
| `mise run rust:check`                                           | 235 tests pass, including five native HTTP integration cases. Nine explicitly ignored cases are outside this run.                                     |
| `mise run ui:check`                                             | TypeScript, 160 tests in 32 files, production build, formatting, and OpenAPI generation checks pass. ESLint has zero errors and 40 existing warnings. |
| `mise run test` / `mise run strava:test`                        | Six renderer tests and the regular gateway suite pass. Gateway PostgreSQL cases requiring `TEST_DATABASE_URL` are outside the regular suite.          |
| `mise run generate:protobuf:check` / `mise run contracts:check` | Generated gateway bindings, HTTP contracts, shared assets, and route inventory pass.                                                                  |
| `mise exec -- woodpecker-cli lint --strict .woodpecker`         | The single monorepo workflow validates, including parallel checks/builds, ordered deployment, and the final k6 step.                                  |
| `mise run synthetics:check`                                     | The pinned k6 runner configuration validates.                                                                                                         |
| IaC `mise run test:bike:release` / `mise run test:woodpecker`   | Release ordering, Git publication, and managed repository synchronization pass with fake Pulumi/CLI boundaries.                                       |
| IaC `mise run check:bike:alerting`                              | Pinned rule validation, rule fixtures, Alertmanager configuration, and three receiver-routing checks pass without notifications.                      |

The UI uses the compatible ESLint 9 / Next 16 configuration. React Compiler
adoption diagnostics remain warnings while the compiler is disabled; Next,
hook-usage, and TypeScript checks remain gates.

## Release and deployment

The Rust deployment step completed in pipeline 167. Pipeline 168 completed
the UI and gateway releases, and manual pipeline 169 completed the renderer
release. All six application deployments are ready and migration Jobs completed.
The application Pulumi preview reports 21 unchanged resources.

These results establish the production baseline before source-history cleanup.
The recovery tag `bike-pre-history-rewrite-2026-10-04` preserves that baseline.
Production pins and subsequent releases remain owned by the infrastructure
repository.

The current release is `cce228e8527e75d18f1897cbd70f4df60d0db44e`.
Pipeline 178 passed all six parallel checks, all five image builds, the four
ordered Pulumi component applies, and production k6. All six application
deployments are ready at that source tag, and its migration Job completed.
The internal scenario reused the IDs provisioned by the preceding release.

## Monitoring

All five Bike dashboards load from Grafana's Kubernetes service. The Rust
overview renders in Chromium, and 91 queries across the overview, map renderer,
Strava gateway, telemetry pipeline, and trace dashboards return successfully.
Seventy-three queries contain data; quiet error paths and rate series can have
no data. This check uses the internal Grafana service through a port-forward.

All five Bike Prometheus targets are up and all 24 Bike alert rules are healthy.
There are no active Bike alerts. The live rules, Alertmanager receiver, and
inhibition configuration match their IaC sources. The trace dashboard uses the
current Rust and shared-service names.

The requested notification test used one explicitly labeled temporary
`BikeAlertDeliveryTest` alert. Its firing notification arrived through the live
ntfy receiver at `2026-10-04T12:48:50Z`; its matching recovery notification
arrived at `2026-10-04T12:53:50Z`. The temporary alert is cleared. This verifies
notification delivery separately from the local Prometheus rule fixtures.

## Production synthetics

Production HTTP synthetics use k6. Pipeline 178 passed **28/28 checks** in
1.7 seconds; `mise run test:production` repeated them successfully through
temporary localhost port forwards in 0.7 seconds. Both runs made 18 real HTTP
requests with zero unexpected failures.

The runner discovers the environment's `platform-smoke/v1` manifest and checks
health, the disabled non-admin identity, activity and segment list/detail,
positive metrics and route coordinates, both race efforts, and a real PNG
through the UI and owned renderer. It also verifies rejection of the credential
on four public API/image routes and rejection of internal mutation, logout,
and admin access. Public API and UI availability pass without authentication.
HTTP 200 or empty records alone cannot pass.

`mise run test:production:browser` independently passed **1/1 Chromium journey**
in 7.0 seconds against the same deployed release. It checks an activity card,
owned PNG, activity detail and route, segment comparison, race playback,
timeline seeking, canvas changes, and preserved selection when returning to
the segment. The deployed UI's API transport is routed to the internal API;
business responses and PostgreSQL data remain real. External browser basemaps
are fixtures. Neither suite verifies live SSO or Strava authorization.

The managed credential remains in the process environment. Tests discover
database IDs rather than requiring a user ID or manually prepared bearer token.
API startup provisions the isolated dataset once and validates it on restart;
the verification requests do not create activities or edit another owner's
records. Credentials, payloads, screenshots, and browser traces are excluded
from this record.

Repeat the checks using the [k6 instructions](../integration-tests/README.md)
or the separate [browser instructions](../bike-ui/tests/e2e/README.md).

### Earlier restoration baseline

Before the isolated account was provisioned, seven meaningful HTTP reads and
the connected Chromium journey passed against existing production records.
That manual baseline used a private, short-lived bearer token for an enabled
user, real APIs/PostgreSQL/owned rendering, and external basemap fixtures. It
did not verify live SSO or Strava authorization or modify production activities.
The automated internal credential and manifest replace that manual setup.
