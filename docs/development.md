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
see [authentication configuration](../bike-rs/docs/specs/auth-configuration.md).

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
The Compose project name is pinned to `bike`; use the root tasks to keep
container and volume ownership consistent.

[`bike-rs/.env.example`](../bike-rs/.env.example) documents backend environment
variables for running processes directly. Container database connections use
`postgres:5432`; host processes use the published PostgreSQL port.
UI server requests use `INTERNAL_API_URL`; browser requests use `API_URL`.

## Run focused checks

Mise manages pinned language and package-manager versions. Use `mise run` for
existing tasks and `mise exec --` for additional commands. Install frontend
dependencies before running host-side checks:

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

## Regenerate contracts and shared assets

Rust's Utoipa schema is the canonical HTTP contract. Regenerate the distribution
copies and frontend types together:

```sh
mise --cd bike-rs run generate:openapi
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
copies are listed in [`shared-assets.json`](shared-assets.json). After changing
an owning asset, run `mise run assets:sync` and review the resulting diff.

## Inspect traces

Local span export defaults to `OTEL_TRACES_EXPORTER=none`. Enable the optional
Jaeger service and point API/worker export to its container address:

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

| Symptom                                     | Next step                                                                                                    |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| Docker cannot connect                       | Start Docker, then run `mise exec -- docker info`.                                                           |
| A published port is occupied                | Set the matching port override and restart.                                                                  |
| The UI is unavailable after the first build | Inspect `docker compose ps` and API/UI logs through `mise exec`; the initial Rust compilation can take time. |
| An upload stays queued                      | Inspect worker logs and the import trace; both API and worker must be running.                               |
| A tool version is missing                   | Run `mise install` in the owning component directory.                                                        |
| A generated client is stale                 | Regenerate OpenAPI and TypeScript, then run `openapi:check`.                                                 |

Update the owning [product specification](../bike-rs/docs/specs/README.md) when
behavior changes. Track unfinished work in [`TODO.md`](TODO.md), and use
conventional commit messages such as `fix(import): preserve source metadata`.

See [deployment and CI](deployment.md) for component workflows, image promotion,
and production infrastructure commands.
