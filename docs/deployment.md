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

The [production verification record](production-verification.md) documents
the current development, CI, deployment, monitoring, and synthetic checks.

| Check                 | Coverage                                                                    |
| --------------------- | --------------------------------------------------------------------------- |
| `test-contracts`      | Canonical HTTP contract and shared asset copies                             |
| `test-rust`           | Rust formatting, Clippy, workspace tests including native HTTP integrations |
| `test-ui`             | ESLint, TypeScript, UI unit tests, formatting, generated OpenAPI client     |
| `test-map-renderer`   | Renderer ESLint, Node tests, and formatting                                 |
| `test-strava-gateway` | golangci-lint (including Go vet), formatting, gateway tests                 |

Every workflow command calls the committed [`bin/mise`](../bin/mise) wrapper
and an existing named task. The wrapper installs the verified mise version in
the root config's `vars.mise_version`. Language tool versions come from the
owning `mise.toml`; shared Node, Go, formatter, and hook pins are inherited
from the root. A generic build image supplies bootstrap dependencies;
mise selects and installs the check toolchain.
CI installs only the tools needed by each check and shares its tool cache.
The same entry points run locally:

```sh
./bin/mise run ci:contracts
./bin/mise run ci:rust
./bin/mise run ci:ui
./bin/mise run ci:renderer
./bin/mise run ci:gateway
```

The root `rust:check`, `renderer:check`, and component `check` tasks own the
actual checks. `ci:deploy` calls `deploy:image` for each component. Image builds
use Woodpecker's Kaniko plugin. The final synthetic step uses the standalone
k6 image entrypoint; it needs no mise bootstrap or cluster credentials at runtime.

When upgrading mise, change `vars.mise_version`, then run
`mise run mise:bootstrap:sync`. This follows
[mise's CI guidance](https://mise.jdx.dev/continuous-integration.html).

## Build version ownership

Root mise vars pin Node, npm, pnpm, Rust, Go, k6, runtime/database images, and
cargo-watch. Mise tools and exported build variables use those values directly.
Compose passes the language variables as Docker build arguments; renderer,
synthetic, and browser image tasks do the same. Specialized protobuf generator
pins use mise's Go backend in the gateway config. There are no version-generation
or custom configuration-validation scripts.

Standalone Docker/Kaniko builds must retain explicit matching ARG defaults:
Docker chooses base images before project configuration can execute. Update
those defaults and native package-manager metadata when upgrading a pin.
The workflow's bootstrap and plugin images are explicit CI inputs. These are
manual synchronization points, not automatically enforced version ownership.
The renderer's Playwright image must match its locked package version.

Run `mise run images:check` for native Docker checks. Update the mise bootstrap
with `mise run mise:bootstrap:sync`. Keep dependency locks in their native
package managers; use frozen installs. The renderer Dockerfile lives in
`map-renderer/` and retains the root build context for the shared protocol.
The UI explicitly uses ESLint 9 because its Next plugins require that major;
the renderer uses ESLint 10. All lint invocations reject warnings.

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

Use the [production failure runbook](production-failures.md) for retained task
failures and the
[backup runbook](https://github.com/ericbutera/pulumi-iac/blob/main/docs/Bike-Backup-Runbook.md)
for database and file recovery. Record incomplete rollout work in
[`TODO.md`](TODO.md); a successful build alone does not establish live health.
