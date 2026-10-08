# TEST11: Containerized Playwright E2E

Design revised on 2026-10-08 under TEST11 in [the backlog](TODO.md), continuing
[Bike PR #8](https://github.com/ericbutera/bike/pull/8). The initial restart-based
gate passed CI pipeline 215, but its 19-minute E2E stage was unacceptable.
The replacement keeps services running and excludes the worker. The revised
required suite passes locally in 3.4 minutes; publication and revised CI
acceptance remain pending. Commands and the scenario
inventory live in the [browser README](../bike-ui/tests/e2e/README.md).

## Implementation evidence

- Shared image-based Compose runtime, separate development/test overlays, root
  `e2e:prepare`/`e2e` tasks, Playwright-owned ORM-validated seed builders, native
  data snapshots, normal authentication, upload/cache restoration, and artifact export
  are implemented locally. Required projects include connected, API-mocked, and
  standalone diagram coverage; visual comparisons are explicit opt-in.
- Repeated default runs passed all 74 tests on Linux arm64 in 213 and 206 seconds with
  zero skips, failures, retries, or flakiness. Initial migration/seed/snapshot
  setup in the final run took 10 seconds. Across 61 resets, database restoration had a 90 ms
  median and 126 ms p95; complete restore/session/files/API setup had a 268 ms
  median and 316 ms p95. Every attempt verified unchanged container IDs/start
  times/restart counts, database object identities, and snapshot hashes. No
  worker container existed, final service logs were clean, and cleanup left no
  run-owned resources. Native reports and timing/runtime attachments are retained
  in `.artifacts/e2e/bike-e2e-20261008202317-3106-20242/`.
- A repeated focused run passed all six setup/upload queue/seeded result/warm
  reset/heatmap checks in 26 seconds. The warm-reset regression deletes a real
  activity through the API, restores the baseline while services remain alive,
  and reads the restored data through the live pool. Rust workspace tests passed
  287 tests, including the fixture CLI's owning-model insert/update and stale
  field/type checks; 26 existing opt-in tests remained ignored. Clippy, rustfmt,
  ShellCheck, ESLint, TypeScript, Prettier, OpenAPI freshness, Dockerfile checks,
  and relevant prek hooks passed without diagnostics.
- Local failure acceptance deliberately failed a mutating browser assertion
  on its first attempt and retry. Both attempts started with restored activities
  `1109/1110`, the next test passed, and service/database identities remained
  unchanged. The run returned failure, retained HTML/JSON reports, both traces
  and screenshots, and removed its resources. A separate migration-startup
  failure also returned failure with retained reports/logs and no surviving
  resources. Cancellation after warm setup returned 143, exported the interrupted
  report and logs, and removed its resources. These disposable local probes are not required browser specs or
  revised CI failure/deployment evidence.
- The CI workflow builds the prepared browser image and job-local daemon from
  mise pins, gates E2E on every owning check/image, and gates deployment on E2E.
  CI reports use the existing cache PVC rather than disappearing with checkout.
  The dependent E2E workflow declares Docker as a native service with TLS port
  2376, as required by the installed Kubernetes backend and strict workflow schema.
- [Companion IaC PR #6](https://github.com/ericbutera/pulumi-iac/pull/6) permits
  privileged steps only for Bike and deploys validated tested-image digests.
  Go formatting, lint, vet, and tests passed. Woodpecker preview/apply changed only
  the Bike sync ConfigMap/Job; the synchronization Job completed successfully.
- Historical evidence for the superseded reset: local release images and the
  prepared browser image built. The required
  invocation passed all 74 tests on Linux arm64 in 15.4 minutes: 59 connected,
  13 mocked, one setup, and one standalone diagram check. There were zero skipped,
  failed, flaky, or retried tests, and no service diagnostics in either per-test
  attachments or final logs. All run-owned resources were removed. Reports,
  image identities, source revision/patch, and logs are retained in
  `.artifacts/e2e/bike-e2e-20261008165139-93738-20784/`.
- Historical focused runs verified GPX upload through the real worker to a persisted activity
  visible in the UI, the connected activity/segment/race journey, and both real
  heatmap checks. An independently selected run passed all 14 setup/mocked checks
  with clean service logs on the final images. These runs coexisted with other
  disposable runs and the existing development stack.
- PostgreSQL uses native password authentication and probes its permanent TCP
  server and maintenance database, avoiding initialization/reset probe errors.
  The release UI image owns its Next.js runtime files so its non-root user can
  write the image cache. Setup and connected/mocked attempts enforce service
  diagnostics as failures, alongside the browser diagnostic gate.
- Connected browser execution exposed incomplete basemap TileJSON, chart sizing
  warnings, and refetches of deleted records before navigation. The fixes reuse
  canonical style layers, a shared Recharts container with initial dimensions,
  and React Query's supported invalidation without immediate refetch. Chromium
  uses Xvfb and Mesa OpenGL in both disposable and external containerized runs.
  Negative browser cases assert their complete, exact expected API errors;
  unexpected diagnostics always fail. UI unit tests passed all 193 tests, native
  Dockerfile checks reported no warnings, and all 13 prek hooks passed.
- Docker Buildx state lives in the ignored project cache, so normal image tasks
  run within the workspace sandbox without permission to write `~/.docker`.
  Each run records complete native engine metadata for reproducibility. The host
  OrbStack configuration reports `DOCKER_INSECURE_NO_IPTABLES_RAW is set`; global
  host settings are not a new prerequisite for running the isolated test stack.
  Cancellation retained HTML/JSON reports and service/browser logs, returned 143,
  and removed every run-owned resource. An intentional startup failure also
  returned failure and removed its partially started resources. These runs
  coexisted with the broader E2E run and the existing development stack.
  Initial CI pipeline 215 passed all 74 tests on Linux amd64, retained reports
  after pod exit, and removed run resources. Revised-gate CI and main deployment
  proof remain pending. The standalone diagram check also passed in
  the retained external containerized runner.

## Template and tmpfs exploration

- A disposable prototype reused the shared runtime and mise-pinned PostgreSQL
  17 image, replacing only test database storage with a 1 GiB tmpfs. It did not
  introduce another managed infrastructure definition or change development.
  Both volume and tmpfs focused runs passed seven checks, including upload
  queue/result states, template cloning, warm data reset, and heatmaps. They
  ran simultaneously without a worker, unexpected diagnostics, or leaked resources.
- A complete `DROP DATABASE` plus `CREATE DATABASE ... TEMPLATE` cycle had a
  120 ms median on the volume and 78 ms on tmpfs across ten samples each.
  Data-only restore medians in the same runs were 129 ms and 100 ms across four
  resets each. These small local samples include command overhead, run alongside
  other disposable work, and do not predict CI performance. Tmpfs usage stayed
  below 80 MiB in the prototype; the 1 GiB value is a cap, not reserved memory.
- A template can be seeded through the same Playwright/ORM boundary, then
  marked `IS_TEMPLATE = true ALLOW_CONNECTIONS = false`. Native cloning needs
  no sessions connected to its source. Replacing the application's target
  database also requires closing its active connections; cloning a separate
  database does not reset the one the API already uses. Templates fit future
  isolated parallel lanes. The current data-only restore preserves the live
  database and pooled connections, and remains the selected default.
- Tmpfs can be adopted independently of templates. Clear the inherited
  PostgreSQL data-volume mount in the E2E overlay before mounting tmpfs at
  `/var/lib/postgresql/data`; keep the existing image pin and normal durability
  settings. Revised Woodpecker memory/runtime evidence is required before
  claiming this prototype as the shared default. Prototype files and reports
  remain local under `.artifacts/template-probe/` and `.artifacts/e2e/`.
- The first tmpfs trial exposed a heatmap assertion reading a tile body after
  Chromium had discarded it. The test now reads that same real tile through
  Playwright's authenticated request context; both repeated focused runs pass
  without retries or weaker diagnostics.

References: [PostgreSQL database templates](https://www.postgresql.org/docs/17/sql-createdatabase.html),
[active connections during database replacement](https://www.postgresql.org/docs/17/sql-dropdatabase.html),
[Docker tmpfs storage and memory accounting](https://docs.docker.com/engine/storage/tmpfs/).

## Required outcome

- Run the maintained required browser suite locally and in CI through the same
  owning mise task, with an explicit spec filter for focused development.
- Exercise real Bike UI, API, PostgreSQL, and renderer behavior. Seed
  deterministic synthetic or public data into real
  services; fixtures and database-backed testing serve complementary purposes.
- Start services once per run and restore only mutable data between attempts.
  Keep the database/schema, connection pools, and service processes alive.
- Exclude the job worker from the default browser gate. Verify queue submission
  and seeded persisted result states. Worker E2E is deferred to a separately
  selected, opt-in suite; default tests must incur no worker startup/polling cost.
- Preserve useful browser regressions that mock application APIs, including
  auth, map controls, and standalone Mermaid/KaTeX rendering. Identify their
  evidence separately from connected behavior.
- Run against prepared, immutable application images. Local preparation can
  build the current worktree through existing image tasks; CI supplies the
  revision's already-built release images. No application builds, package
  installation, Cargo, or `next dev` belong in the actual test run.
- Provision a dedicated disposable environment for each run. It must coexist
  with development and other E2E runs without sharing databases, uploads,
  caches, ports, or globally named volumes.
- Reuse image recipes, migration executables, mise pins, and common runtime
  definitions. Keep the test environment limited to services its scenarios need;
  production infrastructure continues to belong to Pulumi.
- Keep business rules, provider contracts, and database-query regressions in
  their owning suites. Browser E2E establishes connected user journeys, without
  requiring private production data or a live SSO-to-Strava authorization chain.

The shared PR/main pipeline must be:

```text
checkout -> preparation -> unit/lint/native tests -> all image builds -> Playwright E2E -> main-only deployment
```

- Every required stage must succeed before its dependent stages run. PR and
  main revisions use the same tasks; only main continues to `mise run ci:deploy`.
- Deploy the application image digests that passed E2E. Keep the deleted
  `synthetic-smoke` and `test-contracts` pipeline steps removed.

## Constraints that motivated the redesign

- The retained `e2e:run` command is an explicit external read-only monitor.
  Disposable connected work uses `e2e`; development mounts, ports, and persistent
  volume names belong only to the development overlay.
- [Playwright seed builders](../bike-ui/tests/e2e/helpers/seeds.mjs) own explicit
  variants of activities `1109/1110`, segment `5`, and task `5`. Current ORM
  models validate record fields/types; database defaults remain authoritative.
  Native snapshots are generated after current migrations and seeding each run.
  Do not maintain browser seed SQL or checked-in database dumps.
- Upload fixtures and image caches live outside PostgreSQL. A data restore
  alone cannot restore deleted files or remove stale cached results.
- Woodpecker uses its Kubernetes backend. Its documented Docker-in-Docker path
  needs deliberate permissions; the existing
  [Woodpecker IaC](https://github.com/ericbutera/pulumi-iac/blob/main/nibelheim/woodpecker/repo_secrets.go)
  now has an authorized Bike-only permission application. The installed runner's
  registry, TLS/networking, and cancellation behavior still need CI verification.
- Required spec skips, fixed waits, and live archive URLs have source replacements;
  a broad invocation alone still does not establish deterministic coverage.
- Earlier source-compilation and file-signalling daemon drafts are absent from
  the current checkout. Their cleanup is not an implementation prerequisite;
  the replacement must use supported tools and native Playwright fixtures.

## Implementation sequence

Checked items below describe implemented behavior with local verification where
applicable. CI acceptance remains explicit in steps 1 and 8; a configured gate
does not establish a successful CI run.

### 1. Prove the CI container runtime

- [ ] Use a disposable Docker-in-Docker daemon for the E2E job on Woodpecker's
      Kubernetes backend, and the developer's Docker engine locally. Verify the
      installed runner's support before building the rest of the harness.
- [x] Prepare the required permission change in the existing Woodpecker IaC,
      with pinned daemon/client inputs and authenticated engine access. Document
      its actual privilege scope; do not grant the test workload production
      Kubernetes or deployment credentials.
- [ ] Demonstrate that the job can pull the revision's application images from
      the existing registry, create a Compose network and disposable storage,
      reach services, and remove its resources after success and failure.
- [ ] Verify the registry hostname and protocol from both the job and nested
      containers. Do not assume the cluster-only registry address works locally.
- [ ] Verify runner cancellation removes the disposable daemon and its storage.
      If the proof fails, revise this runtime decision before proceeding; do not
      create a custom controller to compensate for missing engine access.

References: [Woodpecker Kubernetes backend and Docker-in-Docker example](https://woodpecker-ci.org/docs/administration/configuration/backends/kubernetes#headless-services).

### 2. Share a minimal runtime and one entry point

- [x] Extract image-based Compose runtime definitions shared by development and
      E2E, with small explicit overrides for their differences. Reuse service
      environment contracts, dependencies, and health checks; avoid copying the
      existing development stack or maintaining a second full infrastructure.
- [x] Keep PostgreSQL, migration, API, UI, and renderer as the
      core test services. Include other owned services only when a maintained
      scenario exercises them. External providers use controlled fixtures.
- [x] Give every run a unique project/network and project-owned disposable
      volumes. Do not inherit development volume names, source bind mounts,
      database URLs, or credentials. Containers use service DNS; any host ports
      needed for inspection are allocated dynamically.
- [x] Implement the proposed root `mise run e2e` entry point. Its default runs
      the complete required suite; an explicit spec filter selects a focused
      slice with the same lifecycle and prerequisite setup.
- [x] Separate preparation from execution. Local preparation uses existing
      cached image builds or explicitly supplied images. CI supplies its built
      images through the same task's documented inputs and never rebuilds them.
- [x] Resolve images once per run and record their identities, source revision,
      and platform. Local worktree builds must be identified as local artifacts;
      deployment evidence requires the release images on the production platform.
- [x] Keep the outer mise/Bash lifecycle responsible for run resources and
      artifact export. Playwright owns scenario setup/reset. Use thin calls to
      Compose/PostgreSQL tools, checked by ShellCheck; avoid duplicate lifecycle
      implementations or marker-file protocols.

References: [Compose reuse and overrides](https://docs.docker.com/compose/how-tos/multiple-compose-files/),
[Compose readiness and dependencies](https://docs.docker.com/compose/how-tos/startup-order/).

### 3. Package the standalone test runner

- [x] Reuse `bike-ui/Dockerfile.e2e` and the UI lockfile. Package maintained
      tests, seed builders, uploads, and lifecycle dependencies. Build the
      model-backed fixture CLI alongside the existing API/migration executables.
- [x] Use the official Playwright image selected by mise, matching its browser
      version to locked `@playwright/test`. Install dependencies at image-build
      time and build the test image alongside application images in CI.
- [x] Run the complete required suite by default and preserve explicit filtering.
      Configure process reaping and sufficient Chromium shared memory through
      the chosen runtime's supported options.
- [x] Keep fixture transfer and artifact export compatible with a remote Docker
      daemon. Use packaged files, named volumes, and supported copy commands;
      do not assume a runner filesystem path exists on the daemon's host.

Reference: [Playwright Docker guidance](https://playwright.dev/docs/docker).

### 4. Build a shared baseline and explicit scenarios

- [x] Inventory every retained browser spec's required fixtures, authentication,
      services, mutations, and optional flags. Classify connected journeys,
      browser tests with mocked application APIs, and optional visual comparisons.
- [x] Run migrations once from the release API image's migration executable.
      Load a small Playwright baseline through the current owning ORM models.
      Build each named variant once and capture native data-only snapshots.
- [x] Use named scenario options in native Playwright fixtures to apply the
      required snapshot before each attempt. Reuse record IDs across independent
      attempts. Keep scenarios explicit rather than inferred from spec filenames.
- [x] Make fixture dates, counts, sequences, ownership, and expected values
      deterministic. Generate expiring session state for the current attempt;
      fixed historical data must not imply a permanently valid session token.
- [x] Use normal session/token authentication for connected writes and admin
      actions, with local-admin bypass disabled. The production synthetic
      credential is read-only and cannot authenticate those journeys.
- [x] Run all browser scenarios without a worker. Seed queued/running/completed/
      failed states explicitly and prepare heatmap projections once through the
      owning builder. Browser assertions never wait for asynchronous job execution.
- Worker E2E is deferred to a future, separately selected opt-in suite and is
  not a completion requirement for TEST11.
- [x] Use synthetic/public source files and the canonical fixture owners. Keep
      private exports, production dumps, and credentials out of the test image.

### 5. Restore all mutable state per connected attempt

- [x] Use a Playwright setup project for baseline preparation/verification and
      dependent test projects. Use reusable test-scoped fixtures for scenario
      resets; every connected attempt and retry starts from a fresh copy.
- [x] Restore rows/sequences in the existing schema with transactional TRUNCATE
      and native data-only COPY output. Preserve database/table identities,
      application connection pools, and service processes across attempts.
- [x] Restore a private copy of upload files and clear mutable service caches.
      Mount the run's volumes on the browser runner, preserving service processes.
      Use normal flag-update APIs and distinct heatmap revisions for cache coherence.
- [x] Keep snapshots private to the runner and verify their hashes. Assert
      unchanged container IDs/start times/restart counts and database object IDs
      after every attempt. Reset only the run-owned PostgreSQL instance.
- [x] Start with one Playwright worker for connected tests. Add parallelism only
      when each worker owns its database, services, and file storage.
- [x] Use built-in `page`, `context`, and `request` fixtures. Browser tests with
      mocked application APIs need fresh browser state and only their required
      services; standalone diagrams need no Bike stack or database reset.
- [ ] Make teardown visible after failed assertions and failed setup. Export
      artifacts before removing run resources; use outer lifecycle cleanup for
      interruption and partial startup, plus verified runner cleanup for forced
      cancellation. Cleanup failures must fail the run.

References: [PostgreSQL TRUNCATE](https://www.postgresql.org/docs/17/sql-truncate.html),
[native data snapshots](https://www.postgresql.org/docs/17/app-pgdump.html),
[Playwright fixtures](https://playwright.dev/docs/test-fixtures),
[setup projects and teardown](https://playwright.dev/docs/test-global-setup-teardown).

### 6. Make maintained coverage deterministic and observable

- [x] Preserve connected activity, segment, race, heatmap, and admin journeys,
      plus useful auth, map-control, and diagram browser regressions. Map each
      spec to its scenario and evidence; keep business-rule matrices in their
      existing owning suites.
- [x] Verify uploads accepted through the real API and queue display, plus
      seeded processed imports linked to visible activities. Identify seeded
      results honestly; queue acceptance does not establish worker processing.
- [x] Control external identity, Strava, tiles, fonts, and archive downloads at
      their actual boundaries. Browser interception does not cover renderer or
      worker traffic; verify those server-side seams separately. Reuse the
      existing local-archive configuration for controlled archive responses.
- [x] Replace live archive URLs, including `https://example.com`, with a
      controlled source that exercises the intended success/failure. Do not
      substitute mocked Bike responses for connected behavior.
- [x] Replace fixed sleeps with web-first assertions, response waits, and
      bounded readiness/result polling. Collect browser warnings, errors, and
      page errors across every context and fail on diagnostics without hiding
      them or increasing timeouts/retries to obtain a pass.
- [x] Define required test selection explicitly. Provision feature flags and
      fixtures for required connected coverage; missing prerequisites fail setup
      rather than silently skipping tests. Report optional screenshot coverage
      separately until its deterministic baselines are reviewed.
- [x] Use `forbidOnly` and `failOnFlakyTests` in CI. Preserve reports, traces,
      failure screenshots, test results, and service logs before cleanup. Record
      selected/passed/failed/skipped/retried counts and tested image identities.

References: [Playwright best practices](https://playwright.dev/docs/best-practices),
[flaky-test enforcement](https://playwright.dev/docs/api/class-testconfig#test-config-fail-on-flaky-tests).

### 7. Enable the shared release gate

- [x] Keep `test-ui-unit` and `test-ui-e2e` separate. Existing unit/native checks
      remain independent of database servers; PostgreSQL belongs to the E2E
      environment and separately selected server-specific checks.
- [x] Require all unit/lint/native checks before builds, all image builds before
      E2E, and E2E before main deployment. Invoke the same owning E2E task on PRs
      and main, without deployment credentials or actions on PR runs.
- [x] Propagate image-pull, migration, fixture-load, reset, readiness, browser,
      artifact-export, and cleanup failures. Deploy only the images selected and
      verified by that revision's E2E gate.
- [x] Keep mise tasks, Docker inputs, locks, and affected prek/CI enforcement
      aligned during implementation. Update the deployment guide and browser
      README with verified commands, prerequisites, and artifact locations.
- [x] Preserve production availability monitoring and its read-only browser
      checks separately from disposable, mutating E2E. TEST11 does not restore
      the removed synthetic smoke stage.

### 8. Verify completion locally and in CI

- [x] Run the affected formatter, linter, applicable type checks, and hooks.
      Inspect the diff for duplicated orchestration, diagnostic suppression,
      unrelated changes, and whitespace errors.
- [x] Verify current migrations/baseline, read journeys, upload queue/result states,
      and database/upload/cache restoration after success and failure. Confirm
      the baseline is unchanged and scenario order does not affect results.
- [x] Run the revised required suite and a focused spec locally, including
      repeat runs. Demonstrate coexistence with development and two simultaneous
      E2E runs without sharing resources or changing development data.
- [x] Record initial seed/snapshot setup, per-attempt restore/session/files,
      browser execution, and total suite timings separately. Prepared full-suite
      execution should take a few minutes; service restarts are prohibited.
- [ ] Run the same required suite on a PR revision and record test counts,
      source revision, platform, and tested image digests.
- [ ] Deliberately fail a browser assertion in an authorized test revision;
      verify pipeline failure, absence of deployment, retained artifacts, fresh
      state on retry, and cleanup. Also verify partial-startup failure and
      cancellation cleanup. Restore the assertion before final review.
- [ ] Verify a revised PR run uses its own images and fresh state without leaks
      from a superseded run. Publish test revisions only under the repository's
      explicit publication rules.
- [ ] After review and merge, verify the same path on main and confirm deployment
      uses the images that passed E2E.

The implementation is not ready to merge until the remaining runtime acceptance
checks pass. Main deployment proof follows reviewed publication and merge; it is
not inferred from local builds, static checks, or the permission application.
