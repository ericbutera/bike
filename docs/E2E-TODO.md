# Plan: Containerized Playwright E2E

Design reviewed on 2026-10-08. Implementation and verification are tracked
under TEST11 in [the backlog](TODO.md). Browser E2E remains excluded from CI
until the checks below pass. The proposed `mise run e2e` task does not exist yet.

## Required outcome

- Run the maintained required browser suite locally and in CI through the same
  owning mise task, with an explicit spec filter for focused development.
- Exercise real Bike UI, API, PostgreSQL, worker, and renderer behavior where a
  scenario needs it. Seed deterministic synthetic or public data into real
  services; fixtures and database-backed testing serve complementary purposes.
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

## Current gaps

- The [browser Dockerfile](../bike-ui/Dockerfile.e2e) and root `e2e:run` task
  default to one `platform-synthetic.spec.mjs` journey against an already
  prepared environment. They do not provision a disposable stack or gate CI.
- The existing Compose configuration is a development stack: it builds dev
  images, mounts source, exposes fixed ports, and names a persistent PostgreSQL
  volume. Changing its project name alone does not isolate E2E.
- Existing [SQL fixtures](../bike-rs/api/tests/fixtures/platform) are deliberate
  variants of the same records: activities `1109/1110`, segment `5`, and task `5`.
  Concatenating them is incompatible. Independent working copies allow reuse
  of these IDs without constructing one giant dataset.
- Upload fixtures and renderer caches live outside PostgreSQL. Database cloning
  alone cannot restore deleted files or remove stale cached results.
- Woodpecker uses its Kubernetes backend. Its documented Docker-in-Docker path
  needs deliberate permissions; the existing
  [Woodpecker IaC](https://github.com/ericbutera/pulumi-iac/blob/main/nibelheim/woodpecker/repo_secrets.go)
  sets `trusted-security=false`. Runtime feasibility is not yet verified.
- Browser specs contain conditional skips, fixture-specific counts, inconsistent
  API defaults, fixed waits, and live archive URLs. A broad invocation does not
  establish complete, deterministic coverage.
- Earlier source-compilation and file-signalling daemon drafts are absent from
  the current checkout. Their cleanup is not an implementation prerequisite;
  the replacement must use supported tools and native Playwright fixtures.

## Implementation sequence

### 1. Prove the CI container runtime

- [ ] Use a disposable Docker-in-Docker daemon for the E2E job on Woodpecker's
      Kubernetes backend, and the developer's Docker engine locally. Verify the
      installed runner's support before building the rest of the harness.
- [ ] Prepare the required permission change in the existing Woodpecker IaC,
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

- [ ] Extract image-based Compose runtime definitions shared by development and
      E2E, with small explicit overrides for their differences. Reuse service
      environment contracts, dependencies, and health checks; avoid copying the
      existing development stack or maintaining a second full infrastructure.
- [ ] Keep PostgreSQL, migration, API, UI, renderer, and optional worker as the
      core test services. Include other owned services only when a maintained
      scenario exercises them. External providers use controlled fixtures.
- [ ] Give every run a unique project/network and project-owned disposable
      volumes. Do not inherit development volume names, source bind mounts,
      database URLs, or credentials. Containers use service DNS; any host ports
      needed for inspection are allocated dynamically.
- [ ] Implement the proposed root `mise run e2e` entry point. Its default runs
      the complete required suite; an explicit spec filter selects a focused
      slice with the same lifecycle and prerequisite setup.
- [ ] Separate preparation from execution. Local preparation uses existing
      cached image builds or explicitly supplied images. CI supplies its built
      images through the same task's documented inputs and never rebuilds them.
- [ ] Resolve images once per run and record their identities, source revision,
      and platform. Local worktree builds must be identified as local artifacts;
      deployment evidence requires the release images on the production platform.
- [ ] Keep the outer mise/Bash lifecycle responsible for run resources and
      artifact export. Playwright owns scenario setup/reset. Use thin calls to
      Compose/PostgreSQL tools, checked by ShellCheck; avoid duplicate lifecycle
      implementations or marker-file protocols.

References: [Compose reuse and overrides](https://docs.docker.com/compose/how-tos/multiple-compose-files/),
[Compose readiness and dependencies](https://docs.docker.com/compose/how-tos/startup-order/).

### 3. Package the standalone test runner

- [ ] Reuse `bike-ui/Dockerfile.e2e` and the UI lockfile. Package the maintained
      tests, configuration, fixture SQL, uploads, and lifecycle dependencies.
- [ ] Use the official Playwright image selected by mise, matching its browser
      version to locked `@playwright/test`. Install dependencies at image-build
      time and build the test image alongside application images in CI.
- [ ] Run the complete required suite by default and preserve explicit filtering.
      Configure process reaping and sufficient Chromium shared memory through
      the chosen runtime's supported options.
- [ ] Keep fixture transfer and artifact export compatible with a remote Docker
      daemon. Use packaged files, named volumes, and supported copy commands;
      do not assume a runner filesystem path exists on the daemon's host.

Reference: [Playwright Docker guidance](https://playwright.dev/docs/docker).

### 4. Build a shared baseline and explicit scenarios

- [ ] Inventory every retained browser spec's required fixtures, authentication,
      services, mutations, and optional flags. Classify connected journeys,
      browser tests with mocked application APIs, and optional visual comparisons.
- [ ] Run migrations once from the release API image's migration executable.
      Load and verify a small shared baseline using the existing SQL/uploads,
      then seal it as a PostgreSQL template with no application connections.
- [ ] Use named scenario options in native Playwright fixtures to apply the
      required SQL overlays to a fresh working copy. Reuse existing record IDs
      across independent copies. Separate seed/variant data from verification
      queries; do not select a dataset implicitly from the spec filename.
- [ ] Make fixture dates, counts, sequences, ownership, and expected values
      deterministic. Generate expiring session state for the current attempt;
      fixed historical data must not imply a permanently valid session token.
- [ ] Use normal session/token authentication for connected writes and admin
      actions, with local-admin bypass disabled. The production synthetic
      credential is read-only and cannot authenticate those journeys.
- [ ] Run queue-display scenarios without a worker. Processing scenarios start
      the real worker and wait for persisted results. Keep pre-existing task
      fixtures stable through existing scheduling/lease semantics, without
      production test switches or sleeps intended to win a race.
- [ ] Use synthetic/public source files and the canonical fixture owners. Keep
      private exports, production dumps, and credentials out of the test image.

### 5. Restore all mutable state per connected attempt

- [ ] Use a Playwright setup project for baseline preparation/verification and
      dependent test projects. Use reusable test-scoped fixtures for scenario
      resets; every connected attempt and retry starts from a fresh copy.
- [ ] Stop the working API and worker and close their pools before disposing of
      the old database. Clone through PostgreSQL's native commands, apply the
      scenario overlays, and explicitly establish database ownership/grants.
- [ ] Restore a private copy of upload files and clear mutable service caches.
      Recreate the affected application processes from the same selected images
      and wait for bounded readiness before browser actions.
- [ ] Keep the sealed baseline inaccessible to application processes and verify
      it remains unchanged. Database administration and resets must target only
      the run-owned disposable PostgreSQL instance.
- [ ] Start with one Playwright worker for connected tests. Add parallelism only
      when each worker owns its database, services, and file storage.
- [ ] Use built-in `page`, `context`, and `request` fixtures. Browser tests with
      mocked application APIs need fresh browser state and only their required
      services; standalone diagrams need no Bike stack or database reset.
- [ ] Make teardown visible after failed assertions and failed setup. Export
      artifacts before removing run resources; use outer lifecycle cleanup for
      interruption and partial startup, plus verified runner cleanup for forced
      cancellation. Cleanup failures must fail the run.

References: [PostgreSQL template databases](https://www.postgresql.org/docs/17/manage-ag-templatedbs.html),
[Playwright fixtures](https://playwright.dev/docs/test-fixtures),
[setup projects and teardown](https://playwright.dev/docs/test-global-setup-teardown).

### 6. Make maintained coverage deterministic and observable

- [ ] Preserve connected activity, segment, race, heatmap, and admin journeys,
      plus useful auth, map-control, and diagram browser regressions. Map each
      spec to its scenario and evidence; keep business-rule matrices in their
      existing owning suites.
- [ ] Verify a successful upload through the real API and worker to a persisted
      activity visible in the UI, in addition to the activity/segment/race read
      journey. Do not infer worker processing from an accepted queue response.
- [ ] Control external identity, Strava, tiles, fonts, and archive downloads at
      their actual boundaries. Browser interception does not cover renderer or
      worker traffic; verify those server-side seams separately. Reuse the
      existing local-archive configuration for controlled archive responses.
- [ ] Replace live archive URLs, including `https://example.com`, with a
      controlled source that exercises the intended success/failure. Do not
      substitute mocked Bike responses for connected behavior.
- [ ] Replace fixed sleeps with web-first assertions, response waits, and
      bounded readiness/result polling. Collect browser warnings, errors, and
      page errors across every context and fail on diagnostics without hiding
      them or increasing timeouts/retries to obtain a pass.
- [ ] Define required test selection explicitly. Provision feature flags and
      fixtures for required connected coverage; missing prerequisites fail setup
      rather than silently skipping tests. Report optional screenshot coverage
      separately until its deterministic baselines are reviewed.
- [ ] Use `forbidOnly` and `failOnFlakyTests` in CI. Preserve reports, traces,
      failure screenshots, test results, and service logs before cleanup. Record
      selected/passed/failed/skipped/retried counts and tested image identities.

References: [Playwright best practices](https://playwright.dev/docs/best-practices),
[flaky-test enforcement](https://playwright.dev/docs/api/class-testconfig#test-config-fail-on-flaky-tests).

### 7. Enable the shared release gate

- [ ] Keep `test-ui-unit` and `test-ui-e2e` separate. Existing unit/native checks
      remain independent of database servers; PostgreSQL belongs to the E2E
      environment and separately selected server-specific checks.
- [ ] Require all unit/lint/native checks before builds, all image builds before
      E2E, and E2E before main deployment. Invoke the same owning E2E task on PRs
      and main, without deployment credentials or actions on PR runs.
- [ ] Propagate image-pull, migration, fixture-load, reset, readiness, browser,
      artifact-export, and cleanup failures. Deploy only the images selected and
      verified by that revision's E2E gate.
- [ ] Keep mise tasks, Docker inputs, locks, and affected prek/CI enforcement
      aligned during implementation. Update the deployment guide and browser
      README with verified commands, prerequisites, and artifact locations.
- [ ] Preserve production availability monitoring and its read-only browser
      checks separately from disposable, mutating E2E. TEST11 does not restore
      the removed synthetic smoke stage.

### 8. Verify completion locally and in CI

- [ ] Run the affected formatter, linter, applicable type checks, and hooks.
      Inspect the diff for duplicated orchestration, diagnostic suppression,
      unrelated changes, and whitespace errors.
- [ ] Verify migrations/baseline, the read journey, the upload/worker journey,
      and database/upload/cache restoration after success and failure. Confirm
      the baseline is unchanged and scenario order does not affect results.
- [ ] Run the complete required suite and a focused spec locally, including
      repeat runs. Demonstrate coexistence with development and two simultaneous
      E2E runs without sharing resources or changing development data.
- [ ] Run the same required suite on a PR revision and record test counts,
      source revision, platform, and tested image digests.
- [ ] Deliberately fail a browser assertion in an authorized test revision;
      verify pipeline failure, absence of deployment, retained artifacts, fresh
      state on retry, and cleanup. Also verify partial-startup failure and
      cancellation cleanup. Restore the assertion before final review.
- [ ] Verify a revised PR run uses its own images and fresh state without leaks
      from a superseded run. Publish test revisions only under the repository's
      explicit publication rules.
- [ ] After reviewed implementation is authorized for publication, verify the
      same path on main and confirm deployment uses the images that passed E2E.

This change finalizes the plan only. Runtime provisioning, the proposed command,
browser-suite migration, and local/CI/deployment verification remain unimplemented.
