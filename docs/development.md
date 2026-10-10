# Developing Bike

Run commands from the repository root unless a component directory is specified.
Install [mise](https://mise.jdx.dev/getting-started.html) and
[Docker with Compose](https://docs.docker.com/compose/install/), then start Docker.

## Start the application

```sh
mise trust
mise install
mise run compose:config
mise run compose:up
```

The root Compose stack includes PostgreSQL, the Rust API and worker, the Next.js
UI, and the map renderer. Open [localhost:3001](http://localhost:3001).
`compose:up` runs in the background and rebuilds images when needed.
PostgreSQL must be healthy before the migration service runs. The API and worker
start after migrations complete successfully; this also initializes a new database.

Local authentication enables a development account and a local admin account.
Production uses configured OAuth/OIDC providers with local auto-login disabled;
see [authentication configuration](specs/auth-configuration.md).

Imports are processed by the worker, so start the complete stack when testing
uploads, archives, or analytics. Rust source changes trigger recompilation inside
the API and worker containers; Next.js reloads changes in `bike-ui`.

```sh
mise exec -- docker compose ps
mise exec -- docker compose logs -f bike-rs worker ui
mise run compose:down
```

Stopping the stack preserves the PostgreSQL volume and activity files. Local
uploads live under `bike-rs/uploads/` and are Git-ignored. The renderer cache is
regenerable and has its own volume.

## Try an activity import

1. Open [Upload](http://localhost:3001/upload).
2. Select a FIT, TCX, or GPX recording. Small synthetic GPX samples are available
   in [`bike-rs/api/tests/fixtures/platform/uploads/activity-imports`](../bike-rs/api/tests/fixtures/platform/uploads/activity-imports).
3. Wait for the import to finish, then open the activity from the activity list.
4. Inspect the route, signals, and derived metrics. Available fields depend on
   the source recording.

File import works without provider credentials. Connecting Strava is a separate
integration setup; the root stack does not start the gateway. See the
[gateway guide](../strava-gateway/README.md) for its processes and configuration.

## Configuration and ports

Defaults are defined in [`bike-rs/compose.yaml`](../bike-rs/compose.yaml) and
[`compose.yaml`](../compose.yaml). Override the published ports before starting:

```sh
BIKE_RUST_API_PORT=3100 BIKE_RUST_UI_PORT=3101 BIKE_RUST_POSTGRES_PORT=55432 \
  mise run compose:up
```

The UI API URL, CORS origin, and development return URL follow these port values.
The Compose project name defaults to `bike`; use the root tasks to keep
container and volume ownership consistent. For an existing stack with another
project name, set `COMPOSE_PROJECT_NAME` to the name shown by
`mise exec -- docker compose ls` before running these tasks. This reuses that
stack rather than creating another set of containers on the same ports.

[`bike-rs/.env.example`](../bike-rs/.env.example) documents backend environment
variables for running processes directly. Container database connections use
`postgres:5432`; host processes use the published PostgreSQL port.
UI server requests use `INTERNAL_API_URL`; browser requests use `API_URL`.

## Run focused checks

Mise manages pinned language and package-manager versions. Run only focused
tests and lints locally, in Docker. CI/CD is the primary gate for full suites,
workspace linting, coverage, production builds, and E2E tests. Git and GitHub
commands may run on the host. Keep required prek hooks active.

Use `mise run checks:docker <task>` for an owning focused task. Worker pipeline
tests include API receipt propagation, queue/attempt history, trace handoffs,
admin handlers, and the UI history/search/polling behavior:

```sh
mise run checks:docker test:workers
```

These tests live in the existing Rust unit and Vitest paths. `ci:rust` and
`ci:ui:unit` discover them as part of the full CI suites; the focused command
does not replace those gates. The PostgreSQL migration/concurrent-claim check
is a separate opt-in integration test, run by `bike-rs`'s
`test:workers:postgres` task with a disposable `BIKE_WORKER_TEST_DATABASE_URL`.

Tool versions and application build-image pins belong in the root `mise.toml`.
Dockerfiles consume build arguments and Compose requires mise's environment.
Change pnpm's pin there, then run `mise run pins:sync` to regenerate its required
`package.json` and lock metadata. `mise run pins:check` rejects drift in local
UI checks, prek, and the same UI tasks used by CI. Dockerfiles use `scratch` as
an empty image reference when no argument is supplied; builds still require
the actual image and tool arguments from mise.

```sh
mise trust bike-rs/mise.toml
mise trust bike-ui/mise.toml
mise trust strava-gateway/mise.toml
mise run hooks:install
mise tasks
```

The following broad tasks are CI entrypoints and reference commands. Do not
repeat them locally during iteration; publish updates to the draft PR and use
that revision's CI results for full validation.

| Command from the root              | Checks                                                                |
| ---------------------------------- | --------------------------------------------------------------------- |
| `mise run rust:check`              | Rust formatting, Clippy, workspace tests                              |
| `mise run lint`                    | `lint:rs`, `lint:go`, `lint:ui`, and `lint:maps`                      |
| `mise run hooks:check`             | All repository-root prek checks against tracked files                 |
| `mise run ui:check`                | UI ESLint, typecheck, tests, build, format, client freshness          |
| `mise run test`                    | Map renderer tests                                                    |
| `mise run strava:test`             | Gateway unit tests with coverage; PostgreSQL checks remain opt-in     |
| `mise run generate:protobuf:check` | Checked-in gateway protobuf bindings                                  |
| `mise run contracts:check`         | Canonical contract and shared asset copies                            |
| `mise run test:integration`        | Real Axum routes and SeaORM queries with isolated SQLite fixture data |
| `mise run compose:config`          | Complete local Compose configuration                                  |
| `mise run check`                   | Combined Rust, UI, renderer, gateway, protobuf checks                 |

The repository-root `prek.toml` routes checks to each owning mise task.
`mise run format:staged` runs the same checks for staged files. ESLint uses zero-warning enforcement; Clippy denies warnings.

For a targeted Rust check, use the component's toolchain:

```sh
# Inside the checks container, from bike-rs:
mise exec -- cargo test -p bike-core --lib <test_filter>
```

Use the [UI browser tests](../bike-ui/tests/e2e/README.md) for activity, segment, and
race-viewer workflows. Prefer unit tests, workflow tests, and provider fakes for
individual rules. Record fixture checks separately from live provider and
production verification.

## Coverage reports

CI collects all implemented coverage suites and enforces the changed-line gate.
When a full local coverage run is explicitly requested, run it in Docker from
the repository root:

```sh
mise run coverage
```

`coverage` runs the owning `coverage:unit` task in a disposable Docker container.
Tool versions and base images come from mise, matching the owning CI tasks.
Docker keeps reusable tool/build caches in `bike-checks-cache` and Linux Node
dependencies in `bike-checks-ui-deps` and `bike-checks-renderer-deps`; no host
mise cache access is needed. Containers are removed after each run. These
volumes contain regenerable dependencies and compiled output, not source copies.
The first container run installs tooling and builds instrumented Rust; later
runs reuse its caches. Reports remain in the checkout for viewing.
Run other owning checks in the same environment, for example
`mise run checks:docker coverage:test` or `mise run checks:docker ci:runtime`.

`coverage:unit` also supports direct execution where tooling is installed,
including CI. Both tasks collect unit coverage for all implemented services
without integration or E2E execution. Use `rust:coverage`, `ui:coverage`,
`renderer:coverage`, or `strava:coverage` to select an owning component.
The [happy-path checklist](coverage-checklist.md) records
the initially uncovered paths and progress adding unit tests.
Each component also provides `mise run coverage` from its own directory. The
tasks install their pinned tools/dependencies and create per-file reports and
totals. The first Rust run builds instrumented artifacts separately from ordinary
builds in `bike-rs/target/llvm-cov-target` when running directly, or
`/cache/target/<checkout>/llvm-cov-target` in Docker; later runs reuse that build cache.
Each Rust collection clears only prior `.profraw` measurements, preserving
compiled artifacts so a previous run cannot inflate the new report.

| Service / project | Implementation | HTML report                                          |
| ----------------- | -------------- | ---------------------------------------------------- |
| Bike API          | Rust           | `.artifacts/coverage/api/html/index.html`            |
| Bike worker       | Rust           | `.artifacts/coverage/worker/html/index.html`         |
| Shared Bike core  | Rust           | `.artifacts/coverage/bike-core/html/index.html`      |
| Bike UI           | Next.js        | `.artifacts/coverage/bike-ui/html/index.html`        |
| Map renderer      | Node.js        | `.artifacts/coverage/map-renderer/html/index.html`   |
| Strava gateway    | Go             | `.artifacts/coverage/strava-gateway/html/index.html` |

Open an HTML file in a browser to inspect coverage by directory, file, and
source line. Each collection replaces its generated reports so removed source
files and previous language implementations do not remain in the current report.
Raw captures and converter logs stay outside the published HTML.
Reports are generated locally and Git-ignored. JSON uses each
provider's native schema; percentages from different languages are not combined.
Each project's directory also contains `lcov.info` and `coverage-summary.json`.
Overall coverage is informational; the changed-line minimum is enforced separately.
Test failures and Rust compiler warnings fail the task.

Rust uses pinned `cargo-llvm-cov` and the matching toolchain's LLVM component.
Reports cover `bike-core`, API, and worker. The root `rust:coverage`
task also runs the native SQLite HTTP integration suite;
`coverage:unit` selects library and binary unit tests only. PostgreSQL tests
marked `#[ignore]` require an explicitly selected disposable database and are
not run by either task. Large real-archive tests marked `#[ignore]` also remain
excluded. Stable Rust coverage does not instrument doctests or
collect branch coverage. Upstream excludes separate test files, generated output,
and dependencies by default; inline test modules may appear in source coverage.
All paths under `migration/` are excluded from reports. The migration crate
still compiles when needed by application code. One workspace measurement is
rendered into separate API, worker, and shared-core reports; shared-core lines
are not counted again in the service reports.

Next.js uses Vitest's V8 provider, pinned to the installed Vitest version. The
report includes all TypeScript application sources in `app`, `components`, and
`lib`, including untested files, while excluding tests and type declarations.
It measures unit/component and route-handler tests, not Playwright browser
execution. Async server components need additional validation of framework
rendering and navigation beyond isolated unit tests.

The map renderer uses [c8](https://github.com/bcoe/c8) with native Node unit tests
and includes unloaded application files at 0%. Its smoke harness, ESLint config,
and test sources are excluded. Strava gateway uses native `go test` coverage
across all handwritten packages, including packages without tests. Generated
protobuf bindings are excluded. PostgreSQL tests remain opt-in and are not
included in these unit reports. The Go directory also contains its native
`coverage.out` profile and `statements.txt`; its native HTML reports statements,
while the shared dashboard and gate measure lines from pinned
[gcov2lcov](https://github.com/jandelgado/gcov2lcov) output.

[`coverage-projects.json`](../coverage-projects.json) defines stable project IDs,
source scope, language, and gate exclusions. To convert map-renderer to Go,
change its language to `Go` and include pattern to `map-renderer/**/*.go`, use
Go test/generated-source exclusions, and make its owning `coverage` task call
`mise --cd .. run go:coverage map-renderer`. The shared Go workflow resolves
that project's source directory and writes its existing report ID. The gate,
dashboard, publisher, and report URLs do not need another language-specific
branch. Further Go services can use the same task with their registered IDs.
Update their owning test/CI tasks and the root collection task together.
Language changes retain service history and start a new trend baseline.

Playwright and k6 are separate checks and do not contribute unit coverage.
Coverage shows execution, not the strength of assertions.

### CI coverage policy and viewing reports

Every owning `test` task produces unit coverage as part of its test run.
Woodpecker's Rust, UI, map-renderer, and Strava gateway test steps call those
tasks; each unit suite runs once. Rust then runs its existing integration tests and
doctests separately, without including their execution in the unit reports.
Its instrumented build cache persists at `/cache/bike/target/llvm-cov-target`;
the first instrumented build is still required, while subsequent runs reuse
unchanged compiled artifacts. Formatting, lint, types, audits, and contract
checks remain in the owning check tasks.

After all owning test steps finish, `mise run ci:coverage` checks their existing
reports through `mise run coverage:check`, without compiling or running tests.
The native [diff-cover CLI](https://github.com/Bachmann1234/diff_cover) reads
each project's LCOV report and requires **80% coverage of added or changed
executable lines**, separately for each service and shared core. Untouched existing source
has no minimum; modifying a line makes it subject to the policy. Tests, type
declarations, and all Rust `migration/**` paths are excluded. Overall percentages
are informational and may decrease without failing this gate.

The first commit creating `scripts/coverage-check.sh` in first-parent Git history
freezes the source that existed when this policy was introduced, including after
the repository's rebase merge. The first rollout grandfathers that tree. Later PRs
compare with the target branch's merge base. Main pushes compare with the prior
main push recorded by CI, falling back to the first parent when unavailable.
Neither comparison can precede the activation revision; missing activation
history fails the gate. New Rust unit tests
should live in separate `*_tests.rs` files so test bodies do not inflate source
coverage. Review still needs to verify meaningful happy-path assertions.

After generating local reports, run `mise run coverage:check`. Set
`COVERAGE_COMPARE_REF` to an explicit Git revision when checking another base.
The task writes uncovered line lists and HTML, Markdown, and JSON diff reports
to `.artifacts/coverage/diff/`. Run `mise run coverage:test` for isolated tooling
fixtures proving independent thresholds, legacy/migration exemptions, invalid
source-path or missing-report failure, historical report compatibility, and
native collection from two independent Go modules; prek and CI run these same fixtures.

View published reports at **[Bike test coverage](https://ericbutera.github.io/bike/coverage/)**.
GitHub Pages is free for this public repository; Codecov and other paid services
are not used. The landing page shows overall coverage, changes from the prior
report, changed-line coverage, full HTML reports, and downloadable HTML/LCOV/JSON
artifacts. Ten recent main revisions remain browsable, with `history.json` and
`latest-summary.json` available for automation.

GitHub Pages is configured once to serve the generated `gh-pages` branch.
Main and manual main pipelines publish using the repository's dedicated SSH
deploy key in the `coverage_publish_key` Woodpecker secret, restricted to push
and manual events. GitHub's SSH host key is pinned in mise; publication uses
SSH over port 443. The existing read-only `github_token` stays with deployment.
PRs enforce the gate and
print uncovered lines without receiving publishing credentials. Reports can
publish after a changed-line failure so failed main coverage remains inspectable;
the failed gate still blocks image builds and deployment. Superseded main runs
skip publication to preserve the latest source revision.

## Regenerate contracts and shared assets

Rust's Utoipa schema is the canonical HTTP contract. Regenerate the distribution
copies and frontend types together:

```sh
mise --cd bike-rs run generate:openapi
mise --cd bike-rs run openapi:check
mise --cd bike-ui run generate:typescript
mise --cd bike-ui run openapi:check
```

The Strava gRPC definition lives in
[`proto/bike/strava/v1/gateway.proto`](../proto/bike/strava/v1/gateway.proto).

```sh
mise run generate:protobuf
mise run generate:protobuf:check
```

Rust client bindings are generated during the Cargo build. Intentional asset
copies are explicit in root mise tasks. After changing an owning asset, run
`mise run assets:sync`, review the diff, and run `mise run contracts:check`.

## Inspect traces

Local span export defaults to `OTEL_TRACES_EXPORTER=none`. Enable the optional
Jaeger 2 service and point API/worker export to its container address. The pinned
image uses its built-in all-in-one configuration with transient in-memory trace
storage and OTLP receivers on ports 4317/4318:

```sh
OTEL_TRACES_EXPORTER=otlp OTEL_EXPORTER_OTLP_ENDPOINT=http://jaeger:4318 \
  mise exec -- docker compose --profile tracing up -d --build
```

Open [Jaeger at localhost:16686](http://localhost:16686). Host-run Rust processes
use `http://localhost:4318` as the OTLP endpoint. Stop the tracing stack with:

```sh
mise exec -- docker compose --profile tracing down
```

## Troubleshooting

After changing tooling or dependencies, run `mise run compose:up` to rebuild
and recreate the development containers. Restarting an existing container does
not update its image. The Rust development image includes the pinned `protoc`;
the UI image uses the pnpm version in mise. Old images can
fail with a missing `protoc` or `packages field missing or empty` during pnpm
installation. Keep any existing project-name and Compose-file overrides when
rebuilding an older stack.

The UI keeps `/app/.next` in its own Docker volume so host-side Next.js builds
and development servers cannot overwrite the container's manifests. Dependencies
remain in a separate `/app/node_modules` volume and the entrypoint synchronizes
them with the frozen lockfile on startup. The pnpm store lives in that volume
too, so dependency hard links do not cross filesystems.

| Symptom                                     | Next step                                                                                                    |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| Docker cannot connect                       | Start Docker, then run `mise exec -- docker info`.                                                           |
| A published port is occupied                | Set the matching port override and restart.                                                                  |
| The UI is unavailable after the first build | Inspect `docker compose ps` and API/UI logs through `mise exec`; the initial Rust compilation can take time. |
| An upload stays queued                      | Inspect worker logs and the import trace; both API and worker must be running.                               |
| A tool version is missing                   | Run `mise install` in the owning component directory.                                                        |
| A generated client is stale                 | Regenerate OpenAPI and TypeScript, then run `openapi:check`.                                                 |

Update the owning [product specification](specs/README.md) when
behavior changes. Track unfinished work in [`TODO.md`](TODO.md), and use
conventional commit messages such as `fix(import): preserve source metadata`.

See [deployment and CI](deployment.md) for component workflows, image promotion,
and production infrastructure commands.
