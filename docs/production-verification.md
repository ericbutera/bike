# Production verification

Verification date: 2026-10-04. This record covers the restored Bike repository,
its component release workflows, and the running production services.

## Development checks

| Check                                                           | Result                                                                                                                                                |
| --------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `mise run lint`                                                 | Rust formatting and strict Clippy, UI ESLint, gateway Go vet, and renderer formatting pass.                                                           |
| `mise run hooks:install` / `mise run hooks:check`               | The root Git hook is installed; all seven prek checks pass against tracked files. The installed hook also passes during commits.                      |
| `mise run rust:check`                                           | 233 tests pass, including the three native HTTP integration cases. Nine explicitly ignored cases are outside this run.                                |
| `mise run ui:check`                                             | TypeScript, 158 tests in 32 files, production build, formatting, and OpenAPI generation checks pass. ESLint has zero errors and 40 existing warnings. |
| `mise run test` / `mise run strava:test`                        | Six renderer tests and the regular gateway suite pass. Gateway PostgreSQL cases requiring `TEST_DATABASE_URL` are outside the regular suite.          |
| `mise run generate:protobuf:check` / `mise run contracts:check` | Generated gateway bindings, HTTP contracts, shared assets, and route inventory pass.                                                                  |
| `mise exec -- woodpecker-cli lint .woodpecker`                  | All five workflow files validate.                                                                                                                     |
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

Seven read-only HTTP checks against the deployed Bike API validate
health, the current user, activity list/detail, segment list/detail, and race
comparison. Assertions require the selected existing records, positive metrics,
usable route coordinates, and real race efforts; HTTP 200 or empty data alone
cannot pass.

`mise run test:production` also passes against the deployed Bike UI.
The Chromium journey checks an activity card and owned PNG thumbnail, activity
detail and route, segment comparison, race playback, timeline seeking, canvas
changes, and preserved selection when returning to the segment.

These requests use real production APIs, PostgreSQL data, and owned map
rendering. External browser basemaps are faked. Authentication uses a private,
short-lived synthetic bearer token for an existing enabled user; this does not
verify live SSO or Strava authorization. The checks do not load fixtures or
modify production activities. Credentials, activity payloads, screenshots,
and browser traces are excluded from this record.

Repeat the browser check using the [production test instructions](../bike-ui/tests/e2e/README.md).
