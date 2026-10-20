# CI refactor

Status: implemented in PR #106; acceptance evidence is recorded in the PR. Not merged or deployed.

## Outcome

Make the build easy to follow before making it fast. A clean checkout with no
restored build cache must support the complete build and test path.

```text
checkout → checks and coverage → build images → test images → deploy on main
```

- One Woodpecker workflow calls the same owning mise tasks used locally.
- Standard Cargo commands check the existing Rust workspace. One ordinary
  multi-stage Dockerfile compiles the release binaries and copies them into
  the runtime image.
- Target three application images: backend, UI, and the existing Go/Chromium map
  renderer. API, worker and migrations use separate commands from the backend
  image; this does not require rewriting them into one executable.
- Target three tracked Compose files: shared services, development differences,
  and E2E differences. Keep the optional ignored machine-local override.
- Delete replaced mechanisms in the slice that replaces them. Do not move their
  complexity into a new launcher or build framework.

Caching, compile-time benchmarks, incremental/profile tuning, prepared-tool-image
publication and runner migration are not prerequisites or deliverables here.
Ordinary Cargo/Docker reuse can happen automatically; correctness must not depend
on restored state. Do not clear existing shared storage or force clean builds on
every invocation. Optimize later, as a separate task.

This roadmap describes the intended replacement. The [CI specification](specs/ci-checks.md)
remains the source of truth for current behavior; update it before implementing
each slice. The [development guide](development.md) owns operating instructions.

## Inventory before the rewrite

Audit: 2026-10-20. There are seven tracked Compose files at the root, plus the
ignored local override, and another compatibility wrapper under `bike-rs/`.
“In use” below distinguishes source references from running-container labels;
it does not establish when an occasional task last ran successfully.

| File                           | Purpose                                                                        | Usage evidence                                                                              | Proposed disposition                                                                              |
| ------------------------------ | ------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| `compose.yaml`                 | Includes runtime, development, Temporal and checks files.                      | Supported default entry point; current local launches select the included files explicitly. | Make it the actual shared service definition.                                                     |
| `compose.runtime.yaml`         | API, migration, UI, maps and PostgreSQL base services.                         | Referenced by local and E2E tasks; present in running local service labels.                 | Move definitions into `compose.yaml`.                                                             |
| `compose.dev.yaml`             | Source builds, local ports, auth, development storage and optional tracing.    | Referenced by running local services.                                                       | Keep development differences here.                                                                |
| `compose.temporal.yaml`        | Development Temporal services, worker, database/schema/namespace setup and UI. | Referenced by running Temporal services and worker.                                         | Put shared worker wiring in the base and development setup in the dev file.                       |
| `compose.e2e.yaml`             | Disposable test state, auth/network overrides and a Temporal dev server.       | Referenced by the E2E tasks and CI; not a continuously running stack.                       | Keep only test differences here.                                                                  |
| `compose.checks.yaml`          | Docker-only tooling and agent worktrees on the shared disk.                    | Documented entry point; used for this review's checks container.                            | Fold into development tooling after proving standalone bootstrap still works.                     |
| `compose.storage.yaml`         | Local BuildKit service with state on the shared disk.                          | Running `bike-buildkit` container names this file.                                          | Fold into development tooling if the service remains necessary; do not discard the storage limit. |
| `compose.local.yaml` (ignored) | Machine-specific source, data and environment settings.                        | Present in running local service labels.                                                    | Keep optional and ignored; document the supported settings.                                       |
| `bike-rs/compose.yaml`         | Compatibility wrapper around the root entry point.                             | Nested Rust Compose tasks still support it.                                                 | Retire and delegate tasks to the root.                                                            |

The other sources of complexity are:

- Two Woodpecker workflows, 15 declared steps and five default image targets:
  API, worker, UI, maps and browser E2E.
- A cargo-chef planner/cook sequence and explicit registry, Git and target cache
  mounts around the release build. API and worker already share that compilation.
- A separate successful-Rust-result cache with input hashes, receipts, locks,
  coverage archives and regression fixtures. This can skip Cargo checks entirely.
- Coverage transfer between separate workflow workspaces and repeated tool setup.
- A per-commit browser image containing test source.
- A Kubernetes CI runner that needs a Docker engine for Compose tests. Its
  containerd/dockerd startup and shutdown code is not a Rust cache requirement.

Evidence: [workflows](../.woodpecker/), [mise tasks](../mise.toml),
[Rust Dockerfile](../bike-rs/Dockerfile), retired Rust result cache,
[image targets](../docker-bake.hcl), and [browser guide](../bike-ui/tests/e2e/README.md).

## The Rust build we want

Keep the Cargo workspace and committed `Cargo.lock`. Keep tool and base-image
versions owned by mise. The underlying commands should be recognizable:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked
cargo build --locked --release \
  --bin api --bin temporal-worker --bin migration \
  --bin recover-generated-imports --bin e2e-fixtures
```

These describe the command shape, not a new host execution path. Owning mise
tasks run the checks inside Docker and invoke the image build. Formatting keeps
the existing workspace-member boundary; `--all` also traverses the generated
Strava path dependency, whose freshness is checked by the SDK generator task.
The Dockerfile owns release compilation; CI does not compile release binaries
first and then compile them again while packaging.

- Check formatting, Clippy and tests directly. Preserve warnings-as-errors,
  doctests, explicit PostgreSQL suites, generated-contract checks and coverage.
  `cargo test` alone does not replace the existing ignored database tests or
  coverage collection.
- Build from the checked-out source and lockfile using the pinned Rust compiler
  and required native tools, including protoc. Copy binaries from that builder
  stage into a non-root runtime with the necessary libraries and certificates.
- Start with source copy, `cargo build --locked --release`, and runtime copies.
  No cargo-chef, dummy workspace, sccache, custom cache keys, explicit compiler
  cache mounts or exported release-artifact handoff.
- Keep test/coverage and release builds distinct. Sharing the workspace does not
  make their compiler output interchangeable.

This uses Cargo's documented [workspace/build selection and lockfile behavior](https://doc.rust-lang.org/cargo/commands/cargo-build.html),
[standard test discovery](https://doc.rust-lang.org/cargo/commands/cargo-test.html),
and Docker's [multi-stage build](https://docs.docker.com/build/building/multi-stage/).

## Review slices

The implementation is one complete PR at the user's request, with one coherent commit and corrections amended.
Update the relevant spec first, then implementation, tests and this status table.
This single replacement spans the workflow, image consumers and Compose files.
Record actual checks and remaining gaps, without requiring benchmarks.
Implementation, merging and deployment are separate states.

| Slice | Reviewable result                                 | Status                                                                |
| ----- | ------------------------------------------------- | --------------------------------------------------------------------- |
| 0     | This inventory and the simple replacement plan    | Ready for review: [#106](https://github.com/ericbutera/bike/pull/106) |
| 1     | Plain Cargo release build                         | Implemented; validation in progress                                   |
| 2     | One CI workflow that runs checks directly         | Implemented; validation in progress                                   |
| 3     | Simple application images and browser test runner | Implemented; validation in progress                                   |
| 4     | Consolidated Compose entry points                 | Implemented; validation in progress                                   |
| 5     | Release acceptance and removal of leftovers       | Implemented; validation in progress                                   |

### Slice 1: replace the Rust release build

- **Change:** replace the chef/planner/cook sequence with one builder stage and
  the explicit release command above. Retain required native dependencies and
  mise-owned pins. Keep existing API/worker image names and commands working
  until slice 3 changes their consumers.
- **Delete:** cargo-chef installation/revision, recipe generation, special SDK
  copies required only for cooking, and explicit Rust build-cache mounts. Remove
  their obsolete arguments from image tasks, Bake and Compose together.
- **Validate:** native Dockerfile checks, resolved build arguments, a release
  build without restored custom caches, and API/worker/migration startup against
  disposable dependencies. Preserve helper binaries and the deployment platform.
- **Review:** the complete path should be visible in the Dockerfile. A successful
  build matters; a faster build is not an acceptance condition.

### Slice 2: replace the workflow and result-cache protocol

- **Change:** one Woodpecker workflow runs owning Rust, UI and renderer checks,
  coverage, image builds, browser tests, report publication and main-only deploy.
  Use ordinary pinned tool installation inside containers; a separately published
  tools image is not required to begin.
- **Delete:** successful-result hashing, receipts, archives, locking and their
  dedicated protocol fixtures; duplicate workflow preparation; cross-workflow
  coverage copying; obsolete task/hook entries and assertions of the old graph.
- **Preserve:** the existing 80% changed-line coverage gate, all required suites,
  warnings-as-errors, report ownership, immutable image identities, failure
  gating and deployment credential restrictions.
- **Validate:** native workflow validation and shell checks, actual Rust/UI/map
  checks, a PR pipeline and an isolated failure that blocks downstream release
  actions. Missing reports must fail rather than reuse old successful results.
- **Review:** each workflow step names its purpose and invokes an owning task.
  UI-only changes may rerun Rust checks. Accept that cost for this rebuild.
- **Cutover:** coordinate required GitHub checks when retiring workflow names.
  Keep required checks effective throughout the change.

### Slice 3: simplify image and browser responsibilities

- **Change:** publish one backend image with separate API, worker and migration
  commands. Keep the UI and Go/Chromium renderer images because their runtimes
  differ. Update deployment image consumers through a companion PR.
- Run current checked-out browser tests with the pinned Playwright environment
  and locked dependencies. Prefer the upstream image; retain a small adapter
  only for required tools. No independent tools-image release system is needed.
- **Delete:** redundant backend image targets and per-commit browser-image
  publication once every consumer uses their replacements.
- **Preserve:** non-root execution, runtime libraries, UI assets, renderer fonts,
  Xvfb/WebGL, fixture CLI, upload resets, logs and failure cleanup.
- **Validate:** backend commands, migrations and the existing full E2E suite
  against the exact release images. Prove tests use the current checkout.
- **Review:** API and worker remain independently configured services. Browser
  E2E currently exercises Temporal submission, not the ingestion worker; do not
  claim worker coverage from a browser pass.

### Slice 4: consolidate Compose

- **Change:** `compose.yaml` owns common services, `compose.dev.yaml` owns local
  differences and tooling, and `compose.e2e.yaml` owns disposable test overrides.
  Keep optional `compose.local.yaml` for machine settings.
- **Delete:** the root include wrapper, separate runtime/Temporal layers, nested
  Rust wrapper, and separate checks/storage files after their replacements work.
- **Preserve:** existing project/volume identities, local PostgreSQL and uploads,
  optional tracing, source-mount behavior, agent worktrees and isolated E2E data.
  All generated state stays on the existing shared 200 GiB Docker disk; refuse
  builds if its quota cannot be verified.
- **Validate:** rendered configs and actual bootstrap for development, checks,
  local overrides and E2E, including a fresh Docker-only entry point with no host
  mise or pre-exported image variables. Profiles do not bypass interpolation.
- **Review:** show the final startup commands and service/mount changes. Retain a
  small standalone checks file if merging it would require a complicated launcher;
  explain that choice rather than meeting a file-count target at any cost.

### Slice 5: accept the replacement and remove leftovers

- **Change:** align specs, development instructions, tasks, hooks, image consumers
  and required branch checks with the working replacement.
- **Delete:** remaining temporary compatibility and unused build/cache scripts,
  pins, images and docs. Keep necessary coverage reporting and release checks.
- **Validate:** required PR/main pipelines, a fresh build without custom restored
  state, published coverage, E2E and failure cleanup. After authorized deployment,
  verify running image digests, migrations, readiness, public API/UI behavior and
  rollback to a previously built release.
- **Review:** final inventory of files and mechanisms retained, checks that passed
  and any unresolved gaps. No compile-time improvement claim is required.
- Clean up this task's scratch and merged worktrees, preserving active work,
  shared tools, databases, uploads and required rollback artifacts.

## Boundaries

- Keep the existing runner backend for this rebuild. Retain the Docker engine
  support actually needed for Compose E2E. Moving off Kubernetes and removing
  its engine lifecycle code can be a separate infrastructure task; ordinary
  Cargo does not require a runner migration.
- Keep the existing builder/storage plumbing where needed to honor the storage
  contract. Remove only parts whose consumers have been replaced. Disk limits
  and data isolation remain required even with no custom compiler cache.
- Preserve the release path until its replacement passes equivalent acceptance.
  Any temporary compatibility must name the later slice that removes it.
- This request authorizes implementation and reviewable PRs. Merges, deployments,
  runner cutovers and branch-protection changes require their own authorization.

## Implementation and acceptance

- At the user's request, all implementation slices are delivered together in
  [PR #106](https://github.com/ericbutera/bike/pull/106). The larger scope is the
  single CI/CD replacement; corrections remain one coherent commit.
- The [CI specification](specs/ci-checks.md) owns the resulting behavior.
  The release Dockerfile uses plain Cargo; Bake publishes backend, UI and maps.
  The one workflow runs checks directly and retains coverage in its checkout.
- Removed cargo-chef and its pins, the successful-Rust-result cache protocol,
  cache regression fixtures, cross-workflow report transfer, the second
  workflow, duplicate backend image targets and browser-image publication.
- Four Compose files remain: shared services, development, E2E and standalone
  checks. The checks exception preserves Docker-only bootstrap without a new
  launcher. Shared backend/API defaults live in explicit env files. One
  acceptance environment checks backend startup/helpers, then removes the worker
  before loading browser fixtures; the remaining services stay running.
  The separate runtime/Temporal/storage files and Rust wrapper are
  removed. BuildKit and persistent volume identities are preserved.
- A companion infrastructure PR changes API, worker and migration consumers
  to the same tested backend digest. It must merge before the Bike cutover.
- The approved Vite patch update fixes the existing UI audit blocker; it is
  limited to the affected transitive dependency resolution.
- Passed locally: Rust formatting/Clippy, unit coverage, integration tests and
  doctests; all 439 UI tests plus lint/types/format/audit; renderer checks;
  coverage policy fixtures and changed-line gates; generated SDK/contracts;
  native Dockerfile/workflow/Compose checks; standalone checks bootstrap;
  shared backend API, worker, migration and helper acceptance checks.
- Native Woodpecker execution with harmless substitute commands verified the
  real graph for successful PR/main events and a failing check: PRs cannot
  publish/deploy, failures block release actions, and cleanup still runs.
  This is graph acceptance, not a server pipeline or production deployment.
- The disposable PostgreSQL runner passes all Strava, heatmap and ingestion
  suites. Focused fixture repairs supply existing publication metadata and
  assert the current recording-retry behavior without changing production code.
- All 82 browser tests pass, including light/dark map acceptance. Successful
  and deliberately failed runs preserve reports and remove owned runtime state.
- The PR records server pipeline results and the companion infrastructure
  preview for the reviewed revision. No merge, deployment, runner migration,
  branch protection change, live acceptance or rollback is part of this PR.
  Production rollout requires the authorized release and existing readiness
  checks.
