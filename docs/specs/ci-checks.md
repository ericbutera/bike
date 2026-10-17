# CI checks

- Local Docker checks, prek hooks, and CI call the owning mise tasks with pinned
  tooling. Formatter, compiler, and linter warnings fail checks.
- `lint:maps` and `format:check` cover JavaScript under `map-renderer/`.
  Retire the obsolete script glob with the removed byte-guard scanner.
  Require every selected source pattern to exist;
  do not suppress unmatched paths or skip validation when files are removed.
- The retired standalone k6 synthetic suite has no tooling pins, tasks, hooks,
  lint patterns, workflow path filters, or image targets.
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
  remains an explicit local Bake target and is built by `checks:docker`; current
  CI uses native steps and does not consume a registry copy of that tool image.
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
- Validate this retirement by running the actual renderer lint and formatting
  tasks, native Woodpecker validation, and release image-plan resolution in
  Docker. CI owns the full suites, coverage, builds, and browser tests.
