# Deploying Bike

Bike's source and Woodpecker workflows live in this repository. Production
infrastructure is managed by
[`ericbutera/pulumi-iac`](https://github.com/ericbutera/pulumi-iac).
Local development uses [Docker Compose](development.md); production runs on
Kubernetes.

## Continuous integration

Woodpecker reads [`.woodpecker/bike.yaml`](../.woodpecker/bike.yaml). Each run
uses one monorepo checkout, then follows **checkout → checks → image builds →
Pulumi apply → production k6 checks**. All five checks run in parallel; every
image build waits for them. Matching main-branch pushes and manual runs build
all five images, deploy them, then verify the running services. Pull requests
run the checks and builds without applying changes or receiving the production
synthetic credential.
Documentation-only changes do not release images.

| Check                 | Coverage                                                                    |
| --------------------- | --------------------------------------------------------------------------- |
| `test-contracts`      | Canonical HTTP contract and shared asset copies                             |
| `test-rust`           | Rust formatting, Clippy, workspace tests including native HTTP integrations |
| `test-ui`             | ESLint, TypeScript, UI unit tests, formatting, generated OpenAPI client     |
| `test-map-renderer`   | Renderer ESLint, Node tests, and formatting                                 |
| `test-strava-gateway` | golangci-lint (including Go vet), formatting, gateway tests                 |

The preparation step runs in the official mise **2026.10.3/debian** image,
pinned by digest. Its owning `ci:mise:prepare` task copies the executable into
ignored `.artifacts/bin/mise`; later checks and deployment use that executable
with existing named tasks in the compiler-equipped buildpack image. They wait
for preparation. The workflow fixes the shared workspace at `/woodpecker/src`,
matching the `.artifacts/bin` entry on `PATH` so nested mise commands resolve
the copied executable. No committed installer or additional system-package setup
is needed. Language tool versions come from the owning `mise.toml`; shared Node,
Go, formatter, and hook pins are inherited from the root. Mise selects and
installs the language toolchain.
CI installs only the tools needed by each check and shares its tool cache.
The same entry points run locally:

```sh
mise run ci:contracts
mise run ci:rust
mise run ci:ui
mise run ci:renderer
mise run ci:gateway
```

The root `rust:check`, `renderer:check`, and component `check` tasks own the
actual checks. `ci:deploy` calls `deploy:image` for each component. Image builds
use pinned Kaniko plugin **2.3.3**, verified to contain the maintained fork's
executor **1.28.5**. The final synthetic step uses the standalone
k6 image entrypoint; it needs no mise bootstrap or cluster credentials at runtime.

Install mise on developer machines using its
[installation instructions](https://mise.jdx.dev/installing-mise.html).
When upgrading mise, update `vars.mise_version`, `vars.mise_image`, the matching
Docker ARG defaults, and the workflow's image reference together. This follows
[mise's CI guidance](https://mise.jdx.dev/continuous-integration.html).

## Build version ownership

Root mise vars pin Node, npm, pnpm, Rust, Go, protoc, cargo-chef, k6,
mise/runtime/database images, and watchexec. Mise tools and exported build
variables use those values directly.
Compose passes the language variables as Docker build arguments; renderer,
synthetic, and browser image tasks do the same. Specialized protobuf generator
pins use mise's Go backend in the gateway config. There are no version-generation
or custom configuration-validation scripts.

Local image tasks pass complete image references and package/compiler pins as
build arguments. `ci:contracts` also calls `ci:images:prepare`, which writes only
public build pins to an ignored environment file. Each Kaniko build loads it,
then invokes the unchanged plugin with its native `build_args_from_env` input.
This small shell handoff is required because Kaniko's image has no mise/bootstrap
runtime. It contains no checks, version parser, secrets, or custom installer.
All builds wait for preparation; missing pins fail the shell step.

Rust **1.99.0**, Node **24.21.0**, Go **1.27.1**, and Debian **trixie** images
have explicit release/variant names and immutable multi-platform digests.
The UI pins Alpine **3.24**; renderer Playwright **1.63.0/noble** continues to
match its locked package. Release Rust images use cargo-chef's separate
`prepare`/`cook` stages to cache locked dependencies before copying application
source. API and worker share identical builder stages and the registry cache;
source-only edits reuse the cooked dependency layer. The vendored Rust patch
is copied before `cook`, so changing it invalidates that layer correctly.
Cargo-chef **0.1.78** repeats target editions, which Cargo 1.99 warns about.
`vars.cargo_chef_revision` temporarily pins upstream
[PR #369](https://github.com/LukeMathWalker/cargo-chef/pull/369) at
`449576bbc2645200936adb9dece80810c9a335f8`. It removes the redundant fields
without suppressing warnings. Replace this prerelease revision with a released
version containing the fix when available. No cache mounts are required;
the stages work with Docker and the existing Kaniko builder.

Rust build scripts and Go binding generation both use protoc **36.2** selected
by root mise. Rust no longer selects a separate vendored compiler. Rust images
use a separate official mise image stage to install the selected compiler through
mise's registry and copy its binary and standard includes into the Rust builder.
The compiler is copied after the dependency cache stage. There are no custom
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
pnpm overrides correct the peer declarations for three exact plugin releases;
the lint compatibility fixtures verify React, accessibility, import, and Next
rules still execute. Remove those entries when upstream peers include ESLint 10.
Two scoped security overrides replace Next's `fast-glob` with the maintained
`tinyglobby` API and use patched KaTeX for Mermaid. Root-directory glob and math
fixtures cover those consumer interfaces. These entries are temporary dependency
compatibility decisions, not advisory ignore lists. UI checks run `mise run audit`
and renderer checks run native npm audit; both fail on dependency advisories.

## Image promotion

The CI workflow clones complete source history and publishes images tagged with
the full source commit SHA. The [deployment wrapper](../.woodpecker/deploy.sh)
clones IaC, then invokes its `deploy:bike:image` mise task. That task owns
Go/Pulumi installation and calls the guarded `scripts/deploy-bike-image.sh`
helper. The IaC task change must be reviewed and published before a Bike
release uses this handoff.

The helper updates only the selected component's image pin, commits it to IaC,
and applies the owning stack. It checks source ancestry before changing a pin,
rejects divergent history, and skips an older release if a newer image already
won. Competing IaC pushes are retried against the latest main branch.

Agents keep work local until the user signs off the completed feature and
explicitly authorizes publishing its reviewed commits. Implementation,
corrections, and tests form one coherent feature commit; requested specs may
have a separate commit. Passing checks and earlier feature approvals do not
authorize a new push.

Consolidating already-published commits also changes release ancestry. Keep a
local recovery reference and review the old production image pin and new
source commit before authorizing a remote rewrite. The IaC release guard
requires an explicit reviewed history transition for divergent source history;
do not weaken that guard to deploy a rewritten branch.

| Pulumi project / stack           | Resources                                                              |
| -------------------------------- | ---------------------------------------------------------------------- |
| `bike` / `bike`                  | Rust API, worker, migration Job, UI, uploads, application ingress      |
| `bike-services` / `prod`         | Map renderer and cache, Strava gateway and worker, gateway artifacts   |
| `woodpecker` / `woodpecker-prod` | CI server, agents, cache, repository configuration and release secrets |

Existing namespaces, registry names, and persistent volumes retain their
identities. Image names such as `bike-services` identify the running map service.
Both service stacks build from this Bike repository.

## Infrastructure changes

Run these commands from a checkout of `pulumi-iac` with its configured Pulumi
backend and Kubernetes access:

```sh
mise trust
mise install
mise run test:bike:release
mise run test:woodpecker
mise run check:bike:alerting
mise run preview:bike:prod
mise run preview:bike-services:prod
mise run preview:woodpecker
```

Review the resource changes, then apply the relevant stack:

```sh
mise run deploy:bike:prod
mise run deploy:bike-services:prod
mise run deploy:woodpecker
```

The release-helper checks use temporary Git repositories and a fake Pulumi CLI.
The Woodpecker checks use a fake CLI. They verify promotion and synchronization
behavior without deploying infrastructure.
Alert validation uses pinned Prometheus and Alertmanager tools against local
fixtures and routing configuration; it sends no notifications.

CI activation, repository configuration, and secrets belong to the
[infrastructure repository](https://github.com/ericbutera/pulumi-iac).

Use the [gateway recovery guide](../strava-gateway/README.md#failure-recovery) for retained task
failures and the
[backup runbook](https://github.com/ericbutera/pulumi-iac/blob/main/docs/Bike-Backup-Runbook.md)
for database and file recovery. Record incomplete rollout work in
[`TODO.md`](TODO.md); a successful build alone does not establish live health.
