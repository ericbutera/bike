# Local development

```sh
mise run compose:up
```

| Service                 | Address                                         |
| ----------------------- | ----------------------------------------------- |
| Bike                    | http://localhost:3001                           |
| Bike API                | http://localhost:3000/api                       |
| Temporal history        | http://localhost:8080/namespaces/bike/workflows |
| Temporal gRPC           | localhost:7233                                  |
| Native SDK metrics      | http://localhost:9092/metrics                   |
| Strava provider metrics | http://localhost:9093/metrics                   |

- The API submits Strava and Bike commands directly to Temporal and acknowledges
  work only after native acceptance. One `temporal-worker` polls `bike-strava`
  and `bike-tasks`. See the [worker spec](specs/background-work.md).
- Setup containers create Temporal's databases, schema, namespace, and six
  search attributes, then exit successfully.
- Temporal uses the `bike` namespace and seven days of closed-history retention.
  Its databases share PostgreSQL's persistent Docker volume.
- API and Temporal worker share the persistent `bike_uploads` volume, outside
  build storage. `BIKE_UPLOADS_DIR` can select an existing absolute data directory;
  preserve that override when upgrading an existing installation. Never select
  a directory inside the build disk or a disposable agent checkout.
- Compose services and checks share the quota-limited build volume below.
  Restart `temporal-worker` after code changes.
- Set `STRAVA_CLIENT_ID`, `STRAVA_CLIENT_SECRET`,
  `STRAVA_WEBHOOK_VERIFY_TOKEN`, and `STRAVA_SUBSCRIPTION_ID` for real Strava.
  API and Temporal worker must share the token encryption key.
- `BIKE_TEMPORAL_ADDRESS` overrides the worker's server address.
- `COMPOSE_PROJECT_NAME` defaults to `bike`; preserve the existing project's
  database volume when switching checkouts.
- `NEXT_PUBLIC_TEMPORAL_UI_URL` configures admin job links; default is localhost.
- Run the existing mise commands inside the checks container described below.
  Builds, dependency installation, formatting, linting, type checks, tests,
  coverage, and code generation must not run on the host. Run `git` and `gh`
  on the host for authentication.
- Run `mise run lint:ui`, `mise run lint:maps`, and `mise run lint:rs` for the
  owning JavaScript/TypeScript and Rust checks.
- Run `mise --cd bike-ui run test` for all UI unit tests and coverage, and
  `mise run test:rs` for `cargo test --workspace --locked`, with automatic
  discovery of workspace tests. `mise run coverage:unit` collects native service
  unit coverage.
  Activities and child workflows use fixture-backed mocks; no running database,
  HTTP service, or Temporal server is required.
- Playwright and Chromium also run inside Docker. The disposable connected suite uses Docker for its
  PostgreSQL, Temporal, application services, and scenario resets; see the
  [browser test guide](../bike-ui/tests/e2e/README.md).
- Run `mise exec -- docker compose logs temporal-worker` to inspect correlated logs.
- Run `mise run compose:down` to stop the stack without deleting its volumes.
- See [the spec](specs/background-work.md#strava-protocol-and-workflows) for webhook routes, processors, and job searches.
- [TODO](TODO.md) records production rollout, retention, restart/load verification,
  and fresh-server startup diagnostics. Local fixtures do not prove production delivery.

Image versions and digests are pinned in `mise.toml`.

## Checks container

The primary host checkout is for editing and review. Agent clones and worktrees,
including Git metadata, live under `worktrees/` on the shared quota disk. Bootstrap
the Docker checkout once from the primary repository's Git data:

```sh
docker run --rm --mount type=volume,source=bike-build-storage,target=/cache --mount "type=bind,source=$PWD/.git,target=/source.git,readonly" --entrypoint git bike-checks:local clone --no-hardlinks /source.git /cache/worktrees/bike
docker compose -p bike-checks -f compose.checks.yaml up -d checks
docker compose -p bike-checks -f compose.checks.yaml exec checks bash
```

Run the normal commands inside that shell, without changing their task definitions:

```sh
mise install
git remote set-url origin https://github.com/ericbutera/bike.git
mise run hooks:install
mise run lint:ui
mise run test:rs
mise run hooks:check
```

The standalone `checks` service uses the image built from `Dockerfile.checks`;
its entry command requires no host mise environment or host cache metadata.
The checks container has no host source mounts and a read-only root filesystem.
Its only writable filesystems are the shared disk and the temporary directory on
that disk. The disk also mounts at its Docker-engine path so nested Docker mounts
and Compose build contexts resolve to the same files. Dependencies, compiler
output, reports, scratch, and `/tmp` all count toward the quota. Prek and Git
commits run in the Docker checkout; host hooks must never start host tooling.
Do not wrap individual mise tasks or prek entries in dispatch scripts.
Rebuild the checks image from inside the existing container when its Dockerfile
or mise image pins change. Keep tool versions in mise.
The Alpine dev UI and Debian checks container use separate dependency directories
on the same quota disk, sharing the package-download store. Their native packages
require different C libraries.

For another task, use native `git worktree add` inside the container under
`$BIKE_DOCKER_STORAGE_ROOT/worktrees/bike-<task>`. Select it with `BIKE_WORKTREE=bike-<task>`
when starting another checks container. Never create host `.worktrees` or separate
cache volumes. Remove completed Docker worktrees with native Git after publishing
their work. Preserve active uncommitted source; Git source is not a disposable log.

The dev UI uses source copied into its image. Run `mise exec -- docker compose watch --no-up ui`
inside the checks container to sync edits using Compose's built-in watcher.
To preview the host editor checkout, mount only that source read-only into a
watcher and point `BIKE_UI_SOURCE` at its UI directory. Run the watcher in Docker.
Generated directories are excluded from sync. The UI does not mount the host
project directory, so deleting host dependencies and caches cannot remove its
live mount points.

## Git and GitHub authentication

The existing host `gh` login can supply authentication for a Docker session:

```sh
GH_TOKEN="$(gh auth token --hostname github.com)" docker compose -p bike-checks -f compose.checks.yaml run --rm --no-deps --env GH_TOKEN checks bash
```

Inside that shell, use `mise exec -- gh ...` and `mise exec -- git ...`.
The checks service routes GitHub HTTPS credentials through `gh` and disables
authentication prompts. The token is a temporary container environment variable;
it is not written to Compose files, images, source files, or the build-cache volume.
The session is removed on exit. Host Git/GitHub commands remain supported.

## Shared build storage

- `bike-build-storage` is the one external Docker volume for disposable Bike
  build storage. Provision its backing Btrfs subvolume with a 200 GiB quota
  (214748364800 bytes) before starting development. Compose volume declarations
  and BuildKit garbage-collection targets do not impose a filesystem quota.
- Rust targets, UI/renderer dependencies, package/tool caches, BuildKit state,
  agent clones/worktrees, `.artifacts`, `.scratch`, `.cache`, and temporary task
  output share this budget.
  Access them inside Docker. Do not expose the volume through workspace
  symlinks or put launcher state in host scratch/cache directories.
- Host `.cache`, `.scratch`, and `.artifacts` are disposable independently of
  Docker. Purging them must not remove Docker files, tool configuration, builder
  settings, or Git hook dependencies. Persistent local configuration belongs in
  versioned source files or ignored `compose.local.yaml` and `.mise.local.toml`.
- Remove completed task scratch, including merged PR scratch. A PR merge alone
  does not perform filesystem cleanup. Retain only explicitly requested reports
  and files belonging to active work.
- PostgreSQL and Temporal databases, uploads, and other user data remain on
  their existing storage outside this disposable volume. Quota exhaustion must
  fail a build without redirecting output onto the host.
- The machine's quota is already provisioned independently of mise. Inspect it
  with Btrfs administration tools inside the Docker engine: the subvolume's
  `max_rfer` must be 214748364800. Do not use an ordinary uncapped volume as a
  replacement, and do not delete or recreate the volume to clear build output.
- Provision the subdirectories referenced by Compose's `volume.subpath` mounts
  on that disk before starting containers. The checks directories are
  `checks/{next,ui-node_modules}`, `renderer/node_modules`, `docker`, and `tmp/checks`;
  temporary directories must be writable by the container users.
- Initialize `checks/ui-node_modules` and generated `checks/next-env.d.ts` and
  `ui/next-env.d.ts` files on the same disk. Seed each generated file from
  `bike-ui/next-env.d.ts` inside Docker. Next.js updates those Docker files
  instead of the tracked type-reference file in the host checkout.
- Local image builds use the `bike-local` remote builder and the
  `bike-buildkit` service in `compose.storage.yaml`. Its BuildKit state shares
  the quota; runtime images loaded into Docker remain outside the volume.
- Keep editor host builds and automatic dependency downloads disabled. During
  cache migration use ordinary copies (`cp --reflink=never`) so OrbStack's
  allocation-based quota accounts for the migrated data.
