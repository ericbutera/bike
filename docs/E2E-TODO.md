# TODO: Containerized Playwright E2E

Status: implementation plan, 2026-10-07. E2E is not yet verified against the
complete disposable stack. Track completion under TEST11 in [the backlog](TODO.md).
Browser E2E is temporarily excluded from CI by request. Complete this checklist
before restoring the separate E2E gate; current CI runs tests, builds, and deployment.

## Required outcome

Run the full maintained browser suite in a container against the same immutable
application images that can be deployed. Load a coherent fixture dataset once,
freeze it as a PostgreSQL template, and restore a fresh working copy for each
test attempt. Include the API, worker, UI, and map renderer wherever the scenario
needs them. Keep business-rule and database-query coverage in their owning suites.

The shared PR/main pipeline must be:

```text
checkout -> preparation -> unit/lint/native tests -> all image builds -> Playwright E2E -> main-only deployment
```

Every required stage must succeed before its dependent stages run. PR revisions
use the same tasks and workflow as main; only main continues to `mise run deploy`.
The deleted `synthetic-smoke` and `test-contracts` pipeline steps stay removed.

## Current gaps

- The existing [browser Dockerfile](../bike-ui/Dockerfile.e2e) and root
  `e2e:run` task default to one `platform-synthetic.spec.mjs` journey. They do not
  establish a full-suite gate or provision a disposable stack.
- Existing [SQL fixtures](../bike-rs/api/tests/fixtures/platform) contain scenario
  mutations of the same records. For example, activity variants use `1109/1110`,
  segment variants use `5`, and task variants use `5`. Concatenating every SQL
  file does not create a compatible baseline.
- The published CI branch attempted to compile/start Rust from the browser test
  step. The subsequent unpublished draft replaced that with a shell fixture
  daemon and file signals. Neither is the intended implementation.
- Upload fixtures live outside PostgreSQL. A database clone alone cannot restore
  deleted or modified source files.
- Current CI uses Woodpecker's Kubernetes backend. Containerized steps do not
  automatically provide a Docker engine or a supported way for Playwright to
  stop and recreate application containers.
- Existing browser documentation describes the earlier single-journey flow and
  must be updated when the replacement is verified.

## Implementation checklist

### 1. Establish one supported container lifecycle

- [ ] Confirm a supported Docker engine path for the CI runner before choosing
      Docker Compose as the stack runner. Woodpecker's Kubernetes backend runs
      steps as Pods; engine access must be configured deliberately. Record the
      chosen runtime and its supported start/stop/recreate interface. Do not
      compensate for missing engine access with a custom controller.
- [ ] Reuse Docker Compose's service definitions, health checks, dependency
      conditions, and cleanup if the runner supports Compose. Keep one E2E stack
      definition used through the same owning mise task locally and in CI.
- [ ] Give each pipeline run a separate stack/network and disposable volumes.
      Define the registry address reachable from the selected runtime. Verify
      actual image digests and use the exact revision's already-built images.
- [ ] Configure readiness checks and propagate startup failures. A started
      container is not necessarily ready to accept requests.
- [ ] Keep any operational shell limited to thin calls to supported container
      and PostgreSQL tools, with ShellCheck enforcement. Native Playwright test
      fixtures belong in the existing UI test language.

References: [Woodpecker Kubernetes backend](https://woodpecker-ci.org/docs/administration/configuration/backends/kubernetes),
[Compose startup order](https://docs.docker.com/compose/how-tos/startup-order/).

### 2. Finish the standalone Playwright image

- [ ] Reuse `bike-ui/Dockerfile.e2e` and the existing UI lockfile. Package the
      tests, configuration, fixture SQL, upload fixtures, and required test
      dependencies into the image.
- [ ] Use the official Playwright image reference selected by mise, and match
      its browser version to the locked `@playwright/test` version. Install
      dependencies during image construction rather than during the E2E run.
- [ ] Build and publish this test image in the build stage with an immutable
      revision reference. Run the full maintained suite by default; preserve an
      explicit spec argument for focused development checks.
- [ ] Configure process reaping and sufficient Chromium shared memory using the
      chosen runtime's supported options.
- [ ] Make application image selection and container URLs explicit. E2E must
      not run Cargo, `next dev`, package installation, or application builds.

Reference: [Playwright Docker guidance](https://playwright.dev/docs/docker).

### 3. Make one coherent fixture baseline

- [ ] Inventory the maintained specs and list their required data and services.
      Include connected scenarios, not just tests with mocked application APIs.
- [ ] Reuse the existing SQL and upload fixtures. Separate fixture data from
      verification queries and scripts that intentionally mutate scenario state.
- [ ] Give incompatible scenarios distinct records, users, and identifiers.
      Update specs to select their own fixture records and expected counts.
      Remove dependence on a previous test's mutations or a spec filename that
      secretly selects a dataset.
- [ ] Keep background-task fixtures stable until deliberately exercised. Check
      that an active worker cannot consume a fixture before its UI assertion.
      Use existing scheduling/lease semantics or isolated scenario state; avoid
      production-only test switches and sleeps intended to win a race.
- [ ] Use deterministic fixture timestamps and public/synthetic source files.
      Keep secrets and private production datasets out of the test image.

### 4. Create and restore the PostgreSQL snapshot

- [ ] Start an isolated PostgreSQL instance using the owning mise image pin.
      Run migrations once using the already-built migration executable from
      the release API image, then load and verify the complete baseline.
- [ ] Use that seeded database as a PostgreSQL template. Close all seeding
      connections and prevent application connections to the baseline. PostgreSQL
      cannot clone a template with active sessions attached to it.
- [ ] Clone a working database from the template for each test attempt. Before
      replacing a working database, stop its API and worker, close their pools,
      and dispose of the old copy through PostgreSQL's native commands.
- [ ] Restore a private working copy of the upload fixture directory and clear
      mutable service caches. Start fresh API/worker processes from the same
      immutable images, then wait for readiness before browser actions.
- [ ] Ensure clones retain the required ownership, permissions, migrations,
      sequences, and data. Database-level grants need explicit handling because
      PostgreSQL template cloning does not copy them.
- [ ] Scope database administration and reset operations to the disposable
      instance. Application processes receive only the working database URL.
      Never snapshot or reset production, or copy a live PostgreSQL data directory.

Reference: [PostgreSQL template databases](https://www.postgresql.org/docs/17/manage-ag-templatedbs.html).

### 5. Let Playwright own setup, isolation, and cleanup

- [ ] Use a Playwright setup project for once-per-run baseline preparation and
      verification, with dependent test projects and a teardown project.
- [ ] Use reusable test-scoped fixtures for working-copy restoration, service
      readiness, and cleanup. Each retry gets a clean copy. Cleanup must run
      after failed assertions, and cleanup failures must remain visible.
- [ ] Keep the existing single-worker execution while one mutable service stack
      is shared. Add parallelism only when each worker has its own database,
      services, and file storage. Browser-context isolation alone does not isolate
      backend data; a browser-side transaction cannot roll back API/worker writes.
- [ ] Prefer Playwright's built-in `page`, `context`, and `request` fixtures.
      Use explicit fixture options for meaningful scenario differences rather
      than a custom fixture framework, browser proxy, or file-based protocol.
- [ ] Use a supported PostgreSQL client/native SQL for baseline cloning and
      seeding. Use the application's real APIs for the behavior under test.

References: [Playwright fixtures](https://playwright.dev/docs/test-fixtures),
[setup projects and teardown](https://playwright.dev/docs/test-global-setup-teardown).

### 6. Make browser coverage deterministic and observable

- [ ] Use real owned UI/API/worker/renderer behavior for connected scenarios.
      Keep self-contained UI/browser regressions in the maintained suite too,
      including the Mermaid/KaTeX browser regression.
- [ ] Stub external identity, Strava, tile, and archive providers at their
      existing boundaries. Browser routing does not intercept server-side
      renderer or worker requests; configure those boundaries separately.
      Replace the archive worker test's live `https://example.com` dependency
      with a controlled response that exercises the intended failure.
- [ ] Replace fixed sleeps with web-first assertions, response waits, and
      bounded readiness/result polling supported by Playwright.
- [ ] Assert browser warnings and errors as failures across every test context.
      Fix causes rather than ignoring diagnostics or increasing timeouts/retries.
- [ ] Inventory skips and environment flags, including the optional visual
      suite. Define the required CI set explicitly; missing connected fixtures
      must fail setup rather than silently skipping tests. Report optional
      visual coverage separately until its deterministic baselines are ready.
- [ ] Retain failure traces, reports, test results, and service logs before
      cleanup. Use Playwright's `forbidOnly` and `failOnFlakyTests` CI options so
      focused tests or a successful retry cannot conceal missing or unstable
      coverage.

Reference: [Playwright best practices](https://playwright.dev/docs/best-practices).
CI options: [TestConfig](https://playwright.dev/docs/api/class-testconfig#test-config-fail-on-flaky-tests).

### 7. Wire the shared pipeline and remove obsolete machinery

- [ ] Keep `test-ui-unit` and `test-ui-e2e` separate. All unit/lint/native gates
      must pass before image builds; all builds must pass before connected E2E;
      E2E must pass before main deployment.
- [ ] Call the same owning mise tasks for PR and main. A force-pushed PR revision
      must run the complete tests/builds/E2E path for that revision. PR runs stop
      after E2E, without deployment credentials or deployment actions.
- [ ] Propagate failures from image pulls, migration, fixture loading, snapshot
      restoration, services, browser tests, and cleanup to the pipeline. Avoid
      ignored failures, unconditional deployment, and success-only log checks.
- [ ] Remove the draft `e2e:prepare`, `e2e:fixtures`, and `e2e:service` daemon
      tasks, `scripts/e2e-*.sh` file-signalling loops, their guard-only test, and
      helper code that uses marker files. Retire the test-only Rust fixture
      binary/source-compilation path from the branch as well.
- [ ] Update affected mise tasks, Docker inputs, package locks, and prek filters
      together. Replace stale instructions in `docs/deployment.md` and the E2E
      README with the verified container lifecycle and focused-run command.

### 8. Verify completion in CI

- [ ] Inspect the final diff for duplicate orchestration, diagnostic suppression,
      and unrelated edits. Run the owning formatter, linter, applicable type
      checks, and hooks for the changed files.
- [ ] Verify baseline creation, one representative connected browser flow, and
      database/upload restoration after both a successful and a failed attempt.
      Confirm the sealed baseline is unchanged after tests.
- [ ] Run the complete required suite on a PR revision and record selected,
      passed, failed, skipped, and retried tests, plus tested image digests.
- [ ] Deliberately fail a browser assertion in a temporary PR revision. Confirm
      the E2E step and pipeline fail, deployment is absent, artifacts are retained,
      and disposable resources are removed. Restore the assertion and rerun.
- [ ] Force-push a revised PR commit and confirm CI uses its images and fresh
      fixtures, with no data or artifacts leaking from the previous revision.
- [ ] After the reviewed change is authorized for publication, verify the same
      path on main and confirm deployment uses the images that passed E2E.

This plan changes documentation only. Implementation and CI verification remain
unchecked. Follow the user's instruction to perform application verification in
CI rather than running broad suites locally.
