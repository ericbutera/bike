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

Mise manages pinned language and package-manager versions. Use `mise run` for
existing tasks and `mise exec --` for additional commands. Install frontend
dependencies before running host-side checks:

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
mise --cd bike-rs install
mise --cd bike-ui install
mise --cd strava-gateway install
mise --cd bike-ui run deps
mise run hooks:install
mise tasks
```

| Command from the root              | Checks                                                                |
| ---------------------------------- | --------------------------------------------------------------------- |
| `mise run rust:check`              | Rust formatting, Clippy, workspace tests                              |
| `mise run lint`                    | `lint:rs`, `lint:go`, `lint:ui`, and `lint:maps`                      |
| `mise run hooks:check`             | All repository-root prek checks against tracked files                 |
| `mise run ui:check`                | UI ESLint, typecheck, tests, build, format, client freshness          |
| `mise run test`                    | Map renderer tests                                                    |
| `mise run strava:test`             | Gateway tests; database cases require `TEST_DATABASE_URL`             |
| `mise run generate:protobuf:check` | Checked-in gateway protobuf bindings                                  |
| `mise run contracts:check`         | Canonical contract and shared asset copies                            |
| `mise run test:integration`        | Real Axum routes and SeaORM queries with isolated SQLite fixture data |
| `mise run compose:config`          | Complete local Compose configuration                                  |
| `mise run check`                   | Combined Rust, UI, renderer, gateway, protobuf checks                 |

The repository-root `prek.toml` routes checks to each owning mise task.
`mise run format:staged` runs the same checks for staged files. ESLint uses zero-warning enforcement; Clippy denies warnings.

For a targeted Rust check, use the component's toolchain:

```sh
cd bike-rs
mise exec -- cargo test -p bike-core <test_filter>
```

Use the [UI browser tests](../bike-ui/tests/e2e/README.md) for activity, segment, and
race-viewer workflows. Prefer unit tests, workflow tests, and provider fakes for
individual rules. Record fixture checks separately from live provider and
production verification.

## Coverage reports

Run all implemented coverage suites from the repository root:

```sh
mise run coverage
```

Use `mise run rust:coverage` or `mise run ui:coverage` to run one component.
Each component also provides `mise run coverage` from its own directory. The
tasks install their pinned tools/dependencies and print per-file coverage and
totals. The first Rust run builds instrumented artifacts separately from ordinary
builds in `bike-rs/target/llvm-cov-target`; later runs reuse that build cache.

| Suite   | HTML report                                | Machine-readable reports                                          |
| ------- | ------------------------------------------ | ----------------------------------------------------------------- |
| Rust    | `.artifacts/coverage/rust/html/index.html`   | `lcov.info`, `coverage-summary.json` in `.artifacts/coverage/rust/`   |
| Next.js | `.artifacts/coverage/nextjs/html/index.html` | `lcov.info`, `coverage-summary.json` in `.artifacts/coverage/nextjs/` |

Open either HTML file in a browser to inspect coverage by directory, file, and
source line. Reports are generated locally and Git-ignored. JSON uses each
provider's native schema; percentages from different languages are not combined.
Coverage currently records a baseline without enforcing a minimum percentage.
Test failures and Rust compiler warnings fail the task.

Rust uses pinned `cargo-llvm-cov` and the matching toolchain's LLVM component.
It instruments `bike-core`, API, worker, and migrations, including ordinary
workspace unit tests and the native SQLite HTTP integration suite. PostgreSQL
tests marked `#[ignore]` require an explicitly selected disposable database and
are not run by this task. Large real-archive tests marked `#[ignore]` also remain
excluded. Stable Rust coverage does not instrument doctests or
collect branch coverage. Upstream excludes separate test files, generated output,
and dependencies by default; inline test modules may appear in source coverage.
Historical migrations remain in the report.

Next.js uses Vitest's V8 provider, pinned to the installed Vitest version. The
report includes all TypeScript application sources in `app`, `components`, and
`lib`, including untested files, while excluding tests and type declarations.
It measures unit/component and route-handler tests, not Playwright browser
execution. Async server components still need browser tests for behavior.

The map renderer, Strava gateway, Playwright, and k6 are not yet coverage suites
in the root task. CI and prek retain their existing checks; coverage is an
opt-in local workflow. Coverage shows execution, not the strength of assertions.

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
