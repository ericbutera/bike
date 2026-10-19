# CI checks

- Local checks and prek hooks call the existing owning mise tasks inside the
  [checks container](../development.md#checks-container). Keep task commands and
  prek entries unchanged; the container supplies their execution environment.
  Run builds, dependency installation, formatting, linting, type checks, tests,
  coverage, and code generation there. Run `git` and `gh` on the host.
  CI retains its existing tasks and environment.
  Formatter, compiler, and linter warnings fail checks.
- Enter through `docker compose -p bike-checks -f compose.checks.yaml run --rm checks bash`.
  The checks container has a read-only root filesystem and no host source mounts.
  Agent clones/worktrees, including their Git metadata, live on the shared disk;
  all writable check output remains there. Do not create host worktrees.
  Host `.cache`, `.scratch`, and `.artifacts` must remain independently purgeable;
  never point them at Docker storage or store required launcher configuration there.
- Use `mise run coverage:unit` for native unit coverage.
  The [shared build volume](../development.md#shared-build-storage) caps local
  caches and generated output at 200 GiB total, excluding databases and uploads.
- `lint:maps` and `format:check` cover JavaScript under `map-renderer/`.
  Retire the obsolete script glob with the removed byte-guard scanner.
  Require every selected source pattern to exist;
  do not suppress unmatched paths or skip validation when files are removed.
- The retired standalone k6 synthetic suite has no tooling pins, tasks, hooks,
  lint patterns, workflow path filters, or image targets.
- Retired backend, UI, and Node-renderer comparisons have no standalone parity
  runners, fixtures, dependencies, or mise tasks. Validate the current Rust API,
  shared UI, and Go snapshot worker through their owning suites and browser tests.
- Preserve the current UI's light/dark pixel, route, layer, and theme-persistence
  regressions in the default browser gate. Full-page screenshot comparisons keep
  their reviewed baselines and run both themes when visual checks are selected.
  Do not regenerate baselines merely to make a comparison pass.
- The standalone PostgreSQL 17/18 upgrade rehearsal is retired, including its
  task and upgrade-image pin. Database integration tests use the current runtime.
- Native API integration tests remain in `bike-rs/api/tests`; browser regressions
  remain in `bike-ui/tests/e2e` and run through their owning tasks.
- The disposable browser stack uses a native Temporal development server for
  submission acknowledgment. It runs no ingestion worker; fixtures supply derived
  results, and unit tests exercise workflow execution with mocked Activities/children.
  Submission checks assert HTTP 202 and native workflow references and acceptance
  state, without waiting for derived results to change.
- Fixture preparation explicitly starts PostgreSQL and Temporal and waits for
  both health checks before starting the API. Warm resets preserve both services;
  runtime identity checks require Temporal to remain running across attempts.
- Register the `BikeUserId` Keyword search attribute when the disposable Temporal
  server starts, before the API can submit owned workflows or filter task history.
- Only the E2E workflow starts a disposable Docker daemon. It verifies generated
  Strava SDK freshness before browser tests using that same daemon. Unit tests
  and their reusable verification receipts do not depend on a Docker engine.
- The internal synthetics monitor is retired: scenario discovery, startup
  provisioning, special authentication, UI credential forwarding, dedicated
  tests, and monitor tasks are removed. The API exposes no synthetics routes.
- Authentication uses ordinary user sessions, API-client tokens, or explicitly
  enabled local-admin mode. Disabled accounts follow the normal rejection rules.
- Historical migrations remain append-only, including the former scenario table
  creation. The application no longer reads or writes that table.
- Ordinary generated test fixtures continue to cover authentication, uploads,
  activities, segments, maps, and races in the owning suites.
- Keep one native Rust compiler workspace at `/cache/bike/target/shared` for
  all branches. Woodpecker's `bike` workflow concurrency is one so different
  revisions cannot clean, replace, or report against the shared objects at once.
  Rust, UI, and renderer checks within that workflow continue in parallel.
- Successful Rust verification archives every Rust project listed in
  `coverage-projects.json`, including its HTML, LCOV, and JSON reports. Keep
  that manifest as the project inventory instead of duplicating crate names
  in the result cache. A clean checkout restoring PR results on main must
  retain every report through coverage transfer and publication.
- Register provider integrations in that same inventory. Each Rust report must
  contain only its owning crate's source files and pass its own changed-line gate.
- A push whose previous CI commit is absent after a history rewrite compares
  against its first parent without emitting Git errors. Rewrites retain the
  coverage activation boundary and the required coverage threshold.
- Cache regression fixtures must remove prior workspace reports before testing
  reuse; leftover cold-run output must not mask an incomplete cache archive.
- Expire the shared compiler cache after seven idle days, reports after two
  days, and successful verification receipts after seven idle days. Shared
  downloaded tool/package stores expire after seven idle days; prepare refreshes
  their `.last-used` markers so read-only reuse counts as activity. Cleanup
  coordinates with the maintenance Lease and scheduler pause. Do not terminate
  builds or evict current cache data based on byte counts.
- Backend and E2E/deploy workflows share `BIKE_GO_CACHE` on the existing cache
  volume. Root mise resolves both Go build/module caches there and the deployment
  child process inherits those paths. Do not let release tooling compile Pulumi
  dependencies in each temporary checkout. Native workflow checks reject a
  missing or different deploy cache. Preserve CI concurrency/resource settings.
  Resolve inherited environment values with yq's
  [standard merge-anchor mode](https://mikefarah.gitbook.io/yq/operators/traverse-read#note-yaml-fix-merge-anchor-to-spec-flag),
  using `--yaml-fix-merge-anchor-to-spec` rather than its legacy override behavior.
- The persistent BuildKit volume owns build layers and compiler/package cache
  mounts. Do not export a second build cache to the image registry. The registry
  stores immutable release images; unprotected Bike images expire after seven
  days, while live and rollback references and their OCI descendants survive.
- Publish only API, worker, UI, maps, and browser E2E images. The checks image
  remains an explicit local Bake target; CI uses native steps and does not
  consume a registry copy of that tool image.
- Release-image validation produces no images or layers. Clear the registry
  image destination only for `images:validate`; publish immutable releases in
  `images:push`.
- Release pushes preserve provenance using BuildKit's compatible attestation
  image manifest format (`oci-artifact=false`). Do not disable provenance or
  suppress unknown-media-type diagnostics.
- Set `push=true` in the same image-exporter definition as `oci-artifact=false`.
  Buildx 0.38.0 applies separate `--push` and output overrides in varying order;
  a successful export can otherwise omit publication for individual targets.
- Preflight the resolved publication plan, then verify every planned registry
  tag against its exported manifest digest. Missing tags, metadata, or mismatched
  digests fail the image step before E2E can try to pull an unpublished image.
- The E2E Docker daemon uses the immutable Docker-in-Docker
  image pin from mise. `pins:sync` refreshes its workflow service and
  `ci:check` rejects drift. Do not rebuild or republish a Bike engine image
  that is identical to its upstream base; it adds no application behavior,
  cannot produce useful build-cache layers, and can trigger missing local
  distribution-source metadata warnings when registry blobs already exist.
- Validate this retirement with the owning renderer lint and formatting tasks,
  Woodpecker validation, and release image-plan resolution inside the container.
  CI remains the required release gate; full suites, coverage, builds, and
  browser tests may also run locally through the owning mise tasks.

## Performance comparisons

- The [performance contract](performance.md#reproducible-performance-datasets)
  owns seeded dataset profiles and before/after measurement requirements.
  Implement its small fixture/runner first through existing owning suites and
  mise tasks; the larger capacity run is explicit, not a per-commit default.
- The ordinary browser fixture stack has no ingestion worker. Concurrent
  read/import qualification must run the real worker in its isolated benchmark
  stack and report its progress; seeded projections alone cannot prove throughput.
- Keep retired synthetics/parity services and special authentication retired.
  This is reproducible performance evidence for actual application paths.
