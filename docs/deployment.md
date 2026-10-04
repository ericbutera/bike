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
| `test-contracts`      | HTTP contracts, shared assets, UI inventory, route inventory                |
| `test-rust`           | Rust formatting, Clippy, workspace tests including native HTTP integrations |
| `test-ui`             | ESLint, TypeScript, UI unit tests                                           |
| `test-map-renderer`   | Renderer Node tests                                                         |
| `test-strava-gateway` | Go vet and gateway tests                                                    |

Database-dependent gateway cases need `TEST_DATABASE_URL`; the regular CI test
step has no test database. UI browser tests remain available through the
[UI integration guide](../bike-ui/tests/e2e/README.md).

The API, UI, renderer, and gateway builds run in parallel. The worker build
follows that wave so the two Rust builds remain separate. Rust Kaniko steps
request four CPUs and 8 GiB of memory each and reuse the registry build cache.

One deployment step invokes the Pulumi helper in order: Bike Rust, map
renderer, Strava gateway, then Bike UI.
API and worker images share a commit tag; the infrastructure stack runs its
migration Job before updating them. The map renderer and gateway share a Pulumi
stack, so their applies run one after the other.

After deployment, `test-k6` runs the [k6 journey](../integration-tests/README.md)
against the internal API and UI services, with separate public availability and
credential-rejection checks. Every assertion must pass for the pipeline to
succeed. Manual runs on `main` use the same sequence.

The root prek hook validates the k6 script with `k6 inspect` before commit.

## Image promotion

The CI workflow clones complete source history and publishes images tagged with
the full source commit SHA. The [deployment wrapper](../.woodpecker/deploy.sh)
clones IaC, then invokes its `scripts/deploy-bike-image.sh` helper.

The helper updates only the selected component's image pin, commits it to IaC,
and applies the owning stack. It checks source ancestry before changing a pin,
rejects divergent history, and skips an older release if a newer image already
won. Competing IaC pushes are retried against the latest main branch.

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
