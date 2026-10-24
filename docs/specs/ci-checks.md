# CI checks

## Build and release path

- One Woodpecker workflow runs on PRs, main pushes and manual requests:
  checkout → checks and coverage → application images → image acceptance →
  coverage publication → main-only deployment. Every step calls owning mise
  tasks. Failed or skipped prerequisites must prevent release actions.
- Rust formatting, locked Clippy, unit coverage and doctests
  run directly on every build. Application policy, preparation, revision guards
  and tile-cache behavior use fixture-backed unit tests without a database.
  Connected heatmap E2E checks use the existing migrated acceptance database for
  user isolation, recording exclusions, pagination and edit/delete invalidation.
  There is no separate PostgreSQL CI gate. Generated Strava SDK and shared-asset
  freshness checks belong to the tooling gate. Standalone PostgreSQL test suites
  and their launch tasks are retired. Migration-specific tests are retired too,
  including mocked schema/SQL assertions, migration-order checks and
  migration-local helper tests. Domain behavior belongs to application units;
  acceptance applies real migrations when starting its application database.
  UI and renderer checks retain lint, types, tests, formatting, audit and
  generated-contract checks. Warnings fail checks.
- Dependency audits and UI peer checks run in the initial lint phase, after
  locked dependencies are installed and before unit tests or coverage. Local
  lint tasks and hooks use the same checks as CI and fail on audit errors.
- The committed Cargo workspace and lockfile are the build inputs. The release
  Dockerfile uses the pinned Rust compiler and protoc, copies source, and runs
  one `cargo build --locked --release` for `api`, `temporal-worker`, `migration`,
  `recover-generated-imports`, and `e2e-fixtures`. Copy those binaries directly
  into one non-root backend image with CA certificates and OpenSSL libraries.
- Publish exactly three application images: backend, UI and Go/Chromium maps.
  API, worker and migration remain separate commands/services using the backend
  image. Preserve renderer fonts, Xvfb/WebGL and UI assets.
- Mise owns tool and base-image pins. The builder owns release compilation;
  CI does not compile release binaries again before packaging. A clean checkout
  without custom restored caches must support the complete path. Tasks that
  install tools must resolve those tools again before executing commands; an
  empty tool store must work on the first invocation.
- No cargo-chef, recipes, dummy workspace, explicit Rust compiler-cache mounts,
  successful-result hashes/receipts/archives/locks, or cross-workflow coverage
  transfer. Ordinary tool downloads and Cargo/Docker reuse remain permitted.
  Do not clear shared caches or require a separately published tools image.
- Keep the existing Kubernetes runner and its necessary disposable Docker-engine
  lifecycle. Engine shutdown runs after success or failure. No runner migration
  or branch-protection change is implicit in this implementation.

## Checks and coverage

- Unit tests cover Bike's core behavior with shared fixtures and mocks: activity
  parsing and imports, segment matching and achievements, training and reports,
  heatmaps, authentication, and map rendering. Cover successful results first;
  retain meaningful failure and ownership checks for these operations.
- Each unit test needs a distinct behavior or input case. Keep rules with their
  owning service or component; parent tests cover their own wiring and state
  rather than repeating child behavior. Shared setup or similar names do not make
  tests duplicates. E2E flows may overlap the units they exercise together.
  Retire generic route/contract smoke matrices that repeat focused product flows.
- Do not test infrastructure or scaffolding: CI graphs and image publication,
  storage maintenance, coverage/report tooling, fixture factories or exports,
  tracing, metrics, environment configuration, and service startup/shutdown.
  Keep native linters, type/build checks, generated-file freshness checks, and
  runtime admission checks. Product browser regressions remain in acceptance.
- Keep `coverage-projects.json` as the project inventory. Every owning project
  emits HTML, LCOV and JSON coverage reports into this workflow's checkout.
  Missing, empty or foreign-source reports fail the 80% changed-line gate.
  Never restore reports from a previously successful revision.
- Preserve first-parent coverage activation and rewritten-history comparison.
  Publish coverage from the same workflow workspace after image acceptance;
  publication credentials and deployment credentials are unavailable to PRs.
- Keep the workflow concurrency at one while native Cargo and coverage output
  share `/cache/bike/target/shared`. Rust, UI and renderer checks may run in
  parallel. Test/coverage output is distinct from release-build output.
- Shared downloaded tool/package stores keep their existing `.last-used`
  maintenance markers. Preserve existing CI resource/concurrency limits and
  Go cache inheritance for deployment tooling.
- Use native Woodpecker validation, ShellCheck, Dockerfile checks, generated
  Strava SDK/protobuf checks and shared-asset comparisons. Do not suppress
  unmatched lint paths or weaken warnings-as-errors.
- The old k6 synthetic suite, internal synthetics monitor, cross-implementation
  parity tools and standalone PostgreSQL upgrade rehearsal remain retired.
  Historical migrations remain append-only. Native API integration tests stay
  in `bike-rs/api/tests`; browser regressions stay in `bike-ui/tests/e2e`.

## Image acceptance and deployment

- Validate the resolved publication plan, build immutable revision tags, and
  verify every tag against its exported manifest digest before acceptance.
  Persistent BuildKit owns ordinary build layers; do not export duplicate
  registry build caches. Keep live/rollback image retention unchanged.
- Preserve provenance with `push=true,oci-artifact=false` in the same image
  exporter definition. Image validation produces no images or layers.
- Browser tests use the current checkout and locked dependencies in the pinned
  Playwright environment. A small local adapter supplies pinned Node/package
  tools and Docker CLI/Compose; it contains no test source or dependencies and
  is never published as a per-commit application image.
- The E2E lifecycle copies current test source, fixtures and Compose files into
  its runner, records the source revision and selected image identities, and
  tests those exact application images. Collect logs and clean owned containers,
  networks, volumes and temporary output on both success and failure.
- Keep light/dark pixel, route, layer and theme-persistence tests in the default
  browser gate. Optional full-page comparisons use reviewed baselines for both
  themes. Do not regenerate baselines to make tests pass.
- Segment-detail interaction and screenshot coverage uses the original winding
  -g GPX route with 37 saved efforts, repeated runs, four pages and three compared
  efforts. Keep this representative scene rather than substituting the small
  Northbound baseline. Geometry is retained; the disposable effort history is
  deterministic fixture data.
- Disposable Temporal supports submission acknowledgment and the `BikeUserId`
  Keyword search attribute. Start PostgreSQL and Temporal and wait for health
  before API startup. Warm resets preserve both identities. Browser tests use
  fixtures for derived results and do not claim ingestion-worker coverage.
- Browser setup applies real migrations, waits for the services needed by product
  flows, and loads fixtures through the packaged fixture command. Do not start
  an ingestion worker for infrastructure smoke checks, invoke CLI/status commands
  just to prove availability, or assert container/database identities after tests.
  Unit tests retain mocked Activities/child workflows. One outer cleanup owns all
  run resources.
- Deployment uses the same backend digest for API, worker and migrations plus
  the accepted UI/maps digests. Main deployment requires successful checks,
  coverage and image acceptance. A companion infrastructure change updates the
  image consumers before cutover; merges and live rollout require authorization.

## Local development and storage

- All development execution, hooks, installs, builds, tests and browsers run
  inside Docker through owning mise tasks. Host Git/GitHub authentication and
  source editing remain supported. Keep host editor checkouts on `main`.
- `compose.yaml` owns shared services, `compose.dev.yaml` owns development
  differences, Temporal setup and BuildKit, and `compose.e2e.yaml` owns isolated
  test overrides. Keep ignored `compose.local.yaml` for machine settings.
  Service environment settings live in explicit `docker/*.env` files, never
  inline Compose environment maps, except checks paths derived from its native
  worktree anchor. Shared settings have one owning file;
  development/E2E env files contain only intentional differences. API and
  worker share database, Temporal and backend defaults; API and maps share
  renderer authentication/tracing settings. Checks reuse the development cache
  file and override only checks-specific paths. Keep credentials scoped to
  services that need them. Mise remains the source of tool and image pins.
  Copy all env files alongside Compose files into the acceptance runner.
- Retain the small standalone `compose.checks.yaml` so checks can bootstrap
  without host mise or exported image pins. Compose profiles cannot prevent
  interpolation of required variables in unrelated services.
- Agent checkouts, caches, artifacts, scratch and temporary output share the
  existing 200 GiB `bike-build-storage` disk. Verify its filesystem quota before
  local builds; fail if verification is unavailable. Keep PostgreSQL, Temporal
  databases and uploads on their existing persistent storage.
- Checks have a read-only root and no host source mounts. Preserve source-watch
  behavior, project/volume identities, optional tracing and data isolation.
  Host scratch/cache/artifact directories remain independently purgeable.
- Keep the pinned native Docker-in-Docker service in CI; mise sync/check tasks
  enforce its pin. Retain engine lifecycle code needed by the current runner.
- Follow the [development guide](../development.md) and
  [browser guide](../../bike-ui/tests/e2e/README.md) for commands. Source checks,
  CI results, merge state and production acceptance are separate evidence.

- Rust formatting keeps the existing workspace-member boundary with
  `cargo fmt --check`. The generated Strava SDK is a separate path dependency;
  its generator freshness task remains authoritative for its source formatting.

Image builds and CI prepare must pass the [storage capacity policy](image-storage.md).

## Real activity DAG regression

- Use activity 1671's frozen recording, fixed segments, effort windows and saved
  metrics as independent expectations for the current source-to-derived path.
  Keep the private provider payload and full reference outside the repository.
- Run connected comparisons against one migrated PostgreSQL database on tmpfs.
  Reuse the pinned E2E services and native fixture command; do not add another CI
  job, per-stage service startup, image build or PostgreSQL suite.
- Replace only the provider boundary with captured responses. Execute registered
  production workflows and operations, then reload typed rows and artifacts.
  Start new outputs empty; restored legacy rows must not hide missing producers.
- Compare recording order, nulls, units, matching, duration/distance/speed, metadata,
  laps, awards, climbs, training and card/heatmap outputs. Preserve old recording
  precision through its existing codec; explain every intentional matcher change.
- Repeat unchanged processing and verify effort identity, recording references and
  immutable awards. Report actual execution time separately from compilation and
  service setup. Ordinary matching/metric regression units remain fast.
