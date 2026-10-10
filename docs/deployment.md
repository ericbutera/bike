# Deploying Bike

Bike's source and Woodpecker workflows live in this repository. Production
infrastructure is maintained separately.
Local development uses [Docker Compose](development.md); production runs on
Kubernetes.

## Continuous integration

Woodpecker reads [`.woodpecker/bike.yaml`](../.woodpecker/bike.yaml). Each run
uses one monorepo checkout, then follows **checkout → tooling preparation → all
tests → one Bake image build → E2E → main-only Pulumi apply**. Preparation shares
mise and prepares the native package cache. Bake builds all seven image targets
after every test step passes. Deployment waits for the builds and E2E.
Every test step uses `failure: fail`: a nonzero exit fails the pipeline and blocks
all builds and deployment. A failed build also blocks deployment.
Pull requests targeting `main` run those same test and build steps;
only deployment has a main-only condition. Woodpecker's
repository setting must enable pull requests; IaC owns that setting. Opening a
PR or pushing another revision, including `git push --force-with-lease`, starts
the shared pipeline. Woodpecker cancels superseded PR runs. Manual branch runs
use the same checks and builds. All images use immutable commit tags, including
PR builds. PR and manual feature-branch runs stop before deployment.
The incorrect synthetic smoke step and its CI image build have been removed.
Documentation-only changes do not release images.

| Check                 | Coverage                                                                                      |
| --------------------- | --------------------------------------------------------------------------------------------- |
| `test-rust`           | Rust formatting, Clippy, workspace tests including native HTTP integrations                   |
| `test-ui-unit`        | ESLint, TypeScript, unit tests, formatting, generated OpenAPI client, dependency audit        |
| `test-map-renderer`   | Go formatting, vet, golangci-lint, tests, build, protobuf freshness, browser asset lint/audit |
| `test-strava-gateway` | golangci-lint (including Go vet), formatting, gateway tests                                   |

The preparation step runs in the official mise **2026.10.3/debian** image,
pinned by digest. Its owning `ci:mise:prepare` task copies the executable into
ignored `.artifacts/bin/mise`; later checks and deployment use that executable
with existing named tasks in the compiler-equipped buildpack image. The UI
unit check uses the buildpack image and runs no Playwright commands. The
[TEST11 implementation](E2E-TODO.md) configures a separate disposable browser
gate described below; runtime acceptance remains pending. Test steps wait for
preparation. Both workflows fix their workspace at `/woodpecker/src`,
matching the `.artifacts/bin` entry on `PATH` so nested mise commands resolve
the copied executable. No committed installer or additional system-package setup
is needed. Language tool versions come from the owning `mise.toml`; shared Node,
Go, formatter, and hook pins are inherited from the root. Mise selects and
installs the language toolchain.
CI installs only the tools needed by each check and shares its tool cache.
The same entry points run locally:

```sh
mise run ci:rust
mise run ci:ui:unit
mise run ci:renderer
mise run ci:gateway
```

### Disposable E2E gate (TEST11, under review)

- `mise run e2e` and the `test-ui-e2e` stage are implemented in source. The
  installed Kubernetes-backed Woodpecker runtime still needs registry, TLS,
  networking, and failure/cancellation acceptance verification.
- The companion infrastructure change owns the Bike-only
  privileged-step permission and tested digest deployment. Its permission
  preview/apply changed only the Bike sync ConfigMap/Job, which succeeded.
  No production deployment credentials are passed to the browser job.
- Run the same minimal Compose model and owning task locally and in CI, using
  independent project names, networks, databases, uploads, and caches. Reuse
  runtime definitions with development through small overrides; keep each test
  run separate from the persistent development stack.
- Local preparation can build the worktree through existing image tasks. CI
  supplies already-built application/test images. Record image identities and
  platform; no application compilation or dependency installation occurs during
  the browser run.
- Keep unit/native checks independent of PostgreSQL servers. The E2E environment
  owns its database, migrated baseline, scenario overlays, and attempt resets.
- The configured shared PR/main path is checks → all image builds →
  E2E → main-only `ci:deploy`. Deployment consumes the image digests that passed
  E2E; PR test jobs receive no deployment credentials.
- `bike.yaml` owns checks and image builds. The dependent `e2e.yaml` workflow
  starts a native Docker service with a private workspace Unix socket, using the daemon image built
  for that revision. Each workflow checks out the same commit and prepares mise
  through the existing task; only E2E and deployment share the tested digest file.
- Export browser reports, failure traces/screenshots, service logs, and image
  identities before cleanup. Startup, migration, fixture, browser, export, or
  cleanup failures must fail the gate and block deployment.
- Retain CI reports on the existing cache PVC at
  `/cache/bike/e2e/<pipeline-number>/<project>/`, with seven-day cleanup. The
  engine socket stays in a mode-0700 directory on the disposable checkout PVC.
  The gate requests and verifies clean engine shutdown before Woodpecker tears
  down its service; unexpected daemon exits still fail the workflow. Local reports live at
  `.artifacts/e2e/<project>/`; both paths are printed by the owning runner.
- Merge the companion digest deployment change before enabling this workflow on
  main. Do not infer a verified CI/main release from a successful permission apply.

Follow the [canonical E2E checklist](E2E-TODO.md) for implementation and acceptance
criteria. Production availability monitoring remains a separate workflow.

### Owning tasks and tooling

The root `rust:check`, `renderer:check`, and component `check` tasks own the
actual checks. Deployment calls `ci:deploy` once for the release. Image builds
use mise-pinned Buildx **0.38.0** and the shared `docker-bake.hcl` definition.
The remote driver connects to persistent BuildKit **0.34.0** in the Woodpecker
Pulumi stack. The image-build step needs no Docker daemon or privileged mode.

Install mise on developer machines using its
[installation instructions](https://mise.jdx.dev/installing-mise.html).
When upgrading mise, update `vars.mise_version`, `vars.mise_image`, the matching
Docker ARG defaults, and the workflow's image reference together. This follows
[mise's CI guidance](https://mise.jdx.dev/continuous-integration.html).

## Build version ownership

Root mise vars pin Node, npm, pnpm, Rust, Go, protoc, cargo-chef, Buildx, k6,
mise/runtime/database images, and watchexec. Mise tools and exported build
variables use those values directly.
Compose passes the language variables as Docker build arguments; renderer,
synthetic, and browser image tasks do the same. Specialized protobuf generator
pins use mise's Go backend in the gateway config. There are no version-generation
or custom configuration-validation scripts.

Local image tasks and `ci:images` resolve image references and package/compiler
pins through the same Bake definition and mise environment. Tasks explicitly
select `docker-bake.hcl`, preventing discovery of development Compose targets.
Local builds load images into Docker; CI pushes full source-SHA tags and records
`.artifacts/image-builds.json`.

`ci:images` connects with Bike-only client credentials synchronized by Pulumi:
`buildkit_ca_cert`, `buildkit_client_cert`, and `buildkit_client_key`. The client
writes temporary certificate files with private permissions and removes them on
exit. Credentials never become Docker build arguments. The builder has one
StatefulSet replica, a dedicated `bike-buildkit-cache` PVC, bounded garbage
collection, a private ClusterIP endpoint, and mutual TLS. The existing
`woodpecker-cache` retains tooling, native-check package caches, and E2E reports.
BuildKit package cache mounts live on its own PVC; exporting registry layers
does not persist those mutable mounts.

CI exports intermediate layers with `mode=max` to
`registry.registry:5000/bike-build-cache:<target>-<scope>`. Main writes `main`;
PRs and other branches write separate scopes and cannot overwrite main's cache.
The persistent builder normally reads its local cache. For recovery after
replacing its disk, set `CACHE_IMPORT=true` only when the corresponding registry
caches exist. Normal builds do not attempt to import absent cache manifests.

Rust **1.99.0**, Node **24.21.0**, Go **1.27.1**, and Debian **trixie** images
have explicit release/variant names and immutable multi-platform digests.
The UI pins Alpine **3.24**. The renderer downloads only the Chromium headless
shell from locked Playwright **1.63.0** during its asset build and packages it
with a Go executable in Debian trixie. Release Rust images use cargo-chef's separate
`prepare`/`cook` stages to cache locked dependencies before copying application
source. `bike-rs/Dockerfile` has one builder and separate `api`/`worker` runtime
targets. One Bake invocation shares their compilation of the API, worker,
migration, recovery, and E2E fixture binaries. Cargo registry/git downloads and
platform-specific release artifacts use persistent cache mounts. Binaries are
copied out of those mounts before export. The vendored Rust patch
is copied before `cook`, so changing it invalidates that layer correctly.
Cargo-chef **0.1.78** repeats target editions, which Cargo 1.99 warns about.
`vars.cargo_chef_revision` temporarily pins upstream
[PR #369](https://github.com/LukeMathWalker/cargo-chef/pull/369) at
`449576bbc2645200936adb9dece80810c9a335f8`. It removes the redundant fields
without suppressing warnings. Replace this prerelease revision with a released
version containing the fix when available.

Development also has one `bike-rs/Dockerfile.dev` with separate API and worker
targets. Compose selects each target while sharing protoc and watchexec setup;
source mounts and watch commands retain their existing behavior.

Rust build scripts and Go binding generation both use protoc **36.2** selected
by root mise. Rust no longer selects a separate vendored compiler. Rust images
use a separate official mise image stage to install the selected compiler through
mise's registry and copy its binary and standard includes into the Rust builder.
The compiler is available during dependency and source compilation. There are no custom
archive-download, architecture-selection, checksum, or unzip steps. Cargo's
existing `tonic_prost_build` build script generates Rust bindings from the
shared protocol using that compiler; generated Rust bindings are not committed.
Go generators are **1.36.12** and **1.6.2**. Run
`mise run generate:protobuf:check` after regeneration.

Docker ARG defaults, immutable image digests, native package metadata, and the
workflow's pinned clone/bootstrap/plugin images remain explicit synchronization
points during upgrades. Docker and Woodpecker must select those inputs before
mise can run. Change a tool version and its corresponding image reference
together; language versions reach actual builds from mise, not YAML defaults.

Run `mise run images:check` for native Docker checks. Keep dependency locks in their native
package managers; use frozen installs. The renderer Dockerfile lives in
`map-renderer/` and retains the root build context for the shared protocol.
The UI and renderer use ESLint 10. All lint invocations reject warnings.

The Rust API owns activity-map PNGs. Its deployment needs `MAP_RENDERER_GRPC_ADDRESS`,
`MAP_SERVICE_TOKEN`, `MAP_IMAGE_CACHE_DIR`, and `MAP_IMAGE_CACHE_TTL_SECONDS`;
the existing cache volume mounts on the API. The UI proxies image requests to
Rust and needs only its existing API URL. The Go snapshot worker has no cache
volume. Coordinate these settings with the API, UI, and worker image release;
see the [maps deployment contract](specs/maps.md#deployment-constraints) and
[release acceptance](specs/maps.md#acceptance-and-evidence). The worker Service
exposes only gRPC 50051 and metrics 9090, with named gRPC health readiness.
Loopback browser assets are not a Service port. Cache metrics and alerts scrape
the API; render and gRPC metrics scrape the worker. OTLP must be configured for
both processes before checking the connected snapshot trace.

## PostgreSQL patching and major-upgrade rehearsal

Application and optional gateway databases share the pinned PostgreSQL
**17.11/trixie** image. The owning IaC definition and backup image use the same
reference; publish/review their companion change before applying the shared
production StatefulSet. The observed running versions on 2026-10-06 were
**17.7** in production and **17.9** locally. A configuration PR is not evidence
that those instances have rolled out.

Create a private custom-format `pg_dump -Fc` of Bike, retain its source version
and checksum, then run `mise run db:rehearse /absolute/path/to/bike.dump`.
The task calls [the Bash rehearsal script](../scripts/db-rehearse.sh), with
both database image pins supplied by mise. `mise run lint:sh`, prek, and CI
check shell scripts with the pinned ShellCheck.
This explicitly selected task restores the backup into two fresh, disposable
volumes on **17.11** and **18.6**, compares every public table's exact row count,
runs the existing PostgreSQL heatmap and gateway storage regressions on each,
and verifies that the
retained 17 instance restarts with the same counts after the 18 instance is
removed. Connections are published only on random loopback ports. Cleanup owns
only those generated container/volume names. It is outside unit tests and CI.
Private backups and activity exports must never be committed.

PostgreSQL 17 mounts `/var/lib/postgresql/data`; PostgreSQL 18 mounts the parent
`/var/lib/postgresql` and uses `/var/lib/postgresql/18/docker`. Production Bike
currently has only `plpgsql`; additional extensions require their own
compatibility rehearsal. A production 18 migration requires a
write freeze, verified database **and activity-file** backups, fresh storage,
restore/migrations, application checks and an explicit traffic switch. Retain
the old 17 volume/backup for recovery. Do not retag a 17 data directory to 18.
Recovery to 17 discards writes accepted only after the switch to 18; keep writes
paused until cutover validation is accepted. The rehearsal does not perform a
live upgrade or guarantee third-party extension compatibility.

Next 16.4 adapts its plugins to the ESLint 10 rule-context API. The UI's native
pnpm overrides correct the peer declarations for three exact plugin releases.
The UI lint gate runs the React, accessibility, import, and Next rules. Remove
those entries when upstream peers include ESLint 10.
Two scoped security overrides replace Next's `fast-glob` with the maintained
`tinyglobby` API and use patched KaTeX for Mermaid. The root-directory glob
fixture and Playwright diagram rendering check cover those consumer interfaces.
These entries are temporary dependency
compatibility decisions, not advisory ignore lists. UI checks run `mise run audit`
and renderer checks run native npm audit; both fail on dependency advisories.

## Image promotion

The CI workflow publishes images tagged with the full source commit SHA and calls
`mise run ci:deploy`. The root [`ci:deploy` task](../mise.toml) clones IaC once into
a temporary directory and invokes that checkout's `ci:deploy` task. Its exit trap
removes the clone after success or failure.
The repository location comes from the `DEPLOYMENT_REPO` environment variable,
supplied by Woodpecker's `deployment_repo` secret. Keep its value in private CI
configuration; configure this secret before enabling deployment.
IaC installs its pinned Go/Pulumi tools, compiles the Bike program once, and calls
`pulumi up --yes --skip-preview` for `ericbutera/bike/bike`. Its four `--config`
arguments set the API/worker, UI, map, and gateway image pins to `CI_COMMIT_SHA`.
There are no component deploy wrappers or IaC Git commits/pushes.

The single `bike` Go program compiles once through `build:bike:prod`. Pulumi's
`runtime.options.binary` runs that executable during the update.
Mise checks Go sources, module files, and tool pins for changes;
image-pin changes do not trigger compilation. The infrastructure preview/apply
tasks use those same builds. Publish the reviewed IaC change before the Bike
change so the cloned infrastructure supports this handoff.

Pulumi stores the successful update's configuration. Before a later manual
infrastructure edit, run `mise exec -- pulumi config refresh --stack
ericbutera/bike/bike --cwd bike --force` in IaC to pull the deployed pins, then
make the intended config changes and preview/apply. The workflow needs its
read-only IaC clone token and Pulumi token; an IaC SSH deploy key is no longer
used.

Permission to commit includes publishing, merging into remote and local main,
and removing the finished worktree and task branches, unless the user requests
a narrower scope. Implementation, corrections, and tests form one coherent
feature commit; requested specs may have a separate commit. See [AGENTS.md](../AGENTS.md).

Local `.artifacts` contains generated coverage, browser reports, logs, image
metadata, and temporary tooling. These outputs can be regenerated with their
owning tasks. Remove completed task output at handoff; do not keep code copies,
Git bundles, or stale deployment checkouts there. Preserve domain knowledge in
the owning specs and runbooks and use Git for source history. Published coverage
reports remain available on GitHub Pages; CI browser reports follow their
configured seven-day retention.

| Pulumi project / stack           | Resources                                                                                                   |
| -------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| `bike` / `bike`                  | Rust API, worker, migration Job, UI, uploads, maps and cache, Strava gateway and worker, artifacts, ingress |
| `woodpecker` / `woodpecker-prod` | CI server, agents, cache, repository configuration and release secrets                                      |

All Bike workloads use the `bike` namespace. Maps and the gateway publish
`bike-maps` and `bike-strava-gateway` images. Internal endpoints, monitoring,
and backup sources use that same namespace. The first namespace cutover requires
transferring state and copying artifact data before starting the moved worker.

## Infrastructure changes

Run these commands from the private infrastructure checkout with its configured
Pulumi backend and Kubernetes access:

```sh
mise trust
mise install
mise run test:woodpecker
mise run check:bike:alerting
mise run preview:bike:prod
mise run preview:woodpecker
```

Review the resource changes, then apply the relevant stack:

```sh
mise run deploy:bike:prod
mise run deploy:woodpecker
```

The existing Woodpecker synchronization checks use a fake CLI.
Alert validation uses pinned Prometheus and Alertmanager tools against local
fixtures and routing configuration; it sends no notifications.

CI activation, repository configuration, and secrets are maintained separately.

Use the [gateway recovery guide](../strava-gateway/README.md#failure-recovery) for retained task
failures and the private backup runbook for database and file recovery.
Record incomplete rollout work in
[`TODO.md`](TODO.md); a successful build alone does not establish live health.
