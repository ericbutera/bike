# Deploying Bike

Bike's source and Woodpecker workflows live in this repository. Production
infrastructure is managed by
[`ericbutera/pulumi-iac`](https://github.com/ericbutera/pulumi-iac).
Local development uses [Docker Compose](development.md); production runs on
Kubernetes.

## Component workflows

Woodpecker reads [`.woodpecker/`](../.woodpecker). Main-branch pushes and pull
requests select workflows by their changed paths. Tests run before image
publication; main-branch pushes publish and deploy the affected components.
Documentation-only changes do not release images.

The [production verification record](production-verification.md) documents
the current development, CI, deployment, monitoring, and synthetic checks.

| Workflow         | Checks                                                                      | Release ownership                                                |
| ---------------- | --------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| `bike-rs`        | Rust formatting, Clippy, workspace tests including native HTTP integrations | API and worker images; `bike:appTag`                             |
| `bike-ui`        | ESLint, TypeScript and UI unit tests; image build                           | UI image; `bike:uiTag`                                           |
| `map-renderer`   | Renderer Node tests                                                         | Renderer image; `bike-services:imageTag`                         |
| `strava-gateway` | Go vet and gateway tests                                                    | Gateway and gateway-worker image; `bike-services:stravaImageTag` |
| `contracts`      | HTTP contracts, shared assets, UI inventory, route inventory                | Checks only                                                      |

Database-dependent gateway cases need `TEST_DATABASE_URL`; the regular CI test
step has no test database. UI browser tests remain available through the
[UI integration guide](../bike-ui/tests/e2e/README.md).

The UI workflow waits for the Rust workflow when both participate in a release.
API and worker images share a commit tag and the infrastructure stack runs its
migration Job before updating them. Renderer and gateway workflows publish and
promote their own images; gateway promotion waits for the renderer workflow
when both participate so their shared Pulumi stack updates in sequence.
For gateway-only changes the renderer workflow keeps that dependency present
and skips its renderer test, image build, and promotion steps.

For an explicit Woodpecker manual run, set `MANUAL_COMPONENT` to `bike-rs`,
`bike-ui`, `map-renderer`, `strava-gateway`, or `contracts`. A UI selection does
not publish a backend image.

## Image promotion

Release workflows clone complete source history and publish images tagged with
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

CI activation and secrets belong to the
[infrastructure repository](https://github.com/ericbutera/pulumi-iac).
For an initial repository move, publish the IaC release helper, apply the
Woodpecker repository configuration, then push Bike workflow changes. The CI
synchronization Job configures the replacement repository before deactivating
the previous release source.

Use the [production failure runbook](production-failures.md) for retained task
failures and the
[backup runbook](https://github.com/ericbutera/pulumi-iac/blob/main/docs/Bike-Backup-Runbook.md)
for database and file recovery. Record incomplete rollout work in
[`TODO.md`](TODO.md); a successful build alone does not establish live health.
