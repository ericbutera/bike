# Bike TODO

## Worker conversion

- [Background work and Strava integration](specs/background-work.md) owns behavior,
  identities, storage, cancellation, priority, and U01–U27 / P01–P04 acceptance.
  This backlog is the single owner of task status, findings, and open work.
  Keep dated evidence in [the rollout record](worker-rollout-2026-10-16.md).
- PR #61 supplied groundwork. PR #66 merged the reviewed reprocess baseline;
  focused review follow-ups remain below. #64 and #65 are superseded.
  Strava is already native. Single-endpoint
  wiring is not production cutover of all producers or shared outputs.
- Convert one bounded task at a time. The user authorized merging and deploying
  completed PRs continuously until this backlog is complete, with each task and
  its downstream outputs verified. Keep PRs and checks as the review record;
  do not pause after each delivery. Status advances
  `Pending` → `In progress` → `Ready to verify` → `Verified`; email becomes `Retired`.
  A failed gate keeps the task open. Do not rename registered tasks without user
  approval/explanation. Email retirement is independent of reprocessing.

### First deliverable: complete reprocessing

- [x] Record the user's 2026-10-16 coverage exception for the initial production
      worker rollout. The exception covered the first production slice; reports,
      tests, lint, images, and release checks remained required. Restore the normal
      percentage gate after real-data vetting below.
- [x] Replace removed database-backed unit setup with 39 focused PostgreSQL mock
      regressions and shared fixtures: import reads/state/artifacts, archive
      progress, preference decisions, task claim/retry/cancellation/recovery,
      integration history, metrics, and SQL processor adapters. Existing native
      reprocess/workflow/storage and timestamp units remain active. Retired DAG
      controls do not need replacement tests. No SQLite schema or compatibility code.
- [x] After the corrected real-data canary, restore the CI 80% gate. Keep ordinary
      coverage reports and existing PostgreSQL/CI persistence checks required;
      mock contracts do not prove isolation, database defaults, or crash recovery.
- [x] Finish the production follow-up from the 22:58 UTC canary. PR #70 merged as
      `16ed6018` with model-owned 500-row bulk writes. The 00:50 UTC production
      stop/start canary accepted work in 0.30 seconds while the worker was down;
      all ten output Activities published on attempt one, with six segment
      Activities overlapping at peak. API and both worker logs had no warnings
      or errors. Thresholds and the operator's CI limits remained unchanged.
- [x] Verify real retained-input reprocessing and eventual outputs in production:
      stable job/run on retry, one audit, preserved activity/import identity and
      business data, no SQL enqueue, fresh fitness, ready heatmap, and all eight
      segment publications. This does not replace the admin UI vetting below.
- [x] Implement the concrete retained-source adapter with model-owned conditional
      writes, preserved activity/import IDs, minimal committed refresh scope,
      removed-segment handoff, audit rollback, and lost-completion retry handling.
- [x] Wire direct single-activity admin submission with stable command identity,
      HTTP-to-domain input mappings, native job references, and lazy Temporal
      connection. Run Bike/Strava queue pollers in one existing `temporal-worker`.
- [x] Supply concrete fitness/segment/heatmap adapters and durable handoffs for
      this review slice; remove native attempt stages; use shared object storage
      and in-memory/SeaORM fixtures for primary and supplemental source reads.
- [ ] Vet the actual admin reprocess flow and eventual rider-visible outputs in
      the existing Docker stack and production. Continue the authorized rollout;
      extend the existing CI E2E suite after the working flow is verified.
- [ ] Repair checksumless legacy source metadata before accepting those imports;
      preserve retained keys/bytes and keep native Strava on sync/backfill.
- [ ] Route every legacy single/bulk producer and batch completion contract at
      the chosen boundary. Drain or fence competing SQL writers and retire its
      user-wide lock dependency; never enable two execution owners for one effect.
- [ ] Move all shared-output producers onto neutral keyed handoffs, including
      Strava, uploads, edits/deletes, training, reads, and archive entries. Remove
      eager per-child/full-user rebuilds, inline fitness-read bypasses, collection
      publication waits, and per-request readiness requirements. Preserve background
      priority for archives/backfills/deletes and prompt new-ride refresh.
- [ ] Close applicable F1–F6 findings below. Keep provider code/queue ownership,
      deletion/reconnection fences, shared activity history, and old-history replay.
- [ ] Complete applicable U01–U17/U23/U26/U27 and affected provider U18–U22/U25 cases;
      record each test/command/commit/result and unimplemented variants. Pass the
      simplicity gate without SQL orchestration, redundant bookkeeping, or a new
      generic recovery service.
- [ ] Prove P01/P02/P04 in the existing stack/CI suite and P03
      where provider routing/history changes. Include acceptance while the worker
      is stopped, commit-before-lost-reply, partial handoff, cancellation races,
      and real eventual outputs. Keep mock, DB, runtime, and deployed evidence distinct.
- [ ] Finish shared command/actor/owner/request/correlation/trace propagation
      through all producers, Activities, children, fan-in, retries, replay, and
      continuation. Verify native visibility and live API-to-workflow traces;
      shared output logs must not attribute merged work solely to the last caller.

### Prioritized review follow-ups

- **Resolved live finding, P0:** the first native reprocess on 2026-10-16 exposed overlapping
  segment refreshes replacing the same activity achievement rows. Five of eight
  Activities retried after PostgreSQL duplicate-key errors; all eventually
  published. PR #67 includes transactional projection replacement and consistent
  activity-first locking alongside R01. Seven PostgreSQL mock cases in
  `bike-core/src/analytics_projection_tests.rs` cover U27's transaction contract;
  mocks do not establish concurrent database execution. After #67, all eight
  overlapping segment Activities completed on attempt one; three slow lock waits
  motivated #70's bulk writes. The final deployed canary published all eight on
  attempt one, with six overlapping at peak and no application warnings/errors.
- **Live finding, P1:** error spans can include bound recording JSON in SQL
  statement diagnostics. Remove recording/provider bodies from application error
  logs while retaining operation, parameterized SQL, and causal IDs. Keep
  diagnostic inspection bounded; no source bytes belong in workflow payloads.
- **Operational follow-up, P1:** investigate the transient root-filesystem growth
  that caused node DiskPressure during CI 481. Application, PostgreSQL, Temporal,
  and CI recovered with retained volumes intact. Reclaiming old Rust executables
  on the separate CI-cache disk does not establish the root-disk cause. Preserve
  the operator's reduced CI resource limits during further validation.
- **CI reliability:** formatting CI 487 failed because the ORM tracing test
  expected one sample but recorded two. The sampler intentionally retains extra
  slow queries at 100 ms; that wiring test uses wall-clock durations under CPU
  limits. [PR #74](https://github.com/ericbutera/bike/pull/74) uses deterministic
  span fixtures for exact fast/slow counts and the inclusive threshold, and keeps
  ORM wiring assertions valid under scheduling delays. All eight focused tracing
  tests and required Rust hooks pass locally. Keep open for review; production
  sampling caps and CI resource limits are unchanged.
- **Formatting integration:** PR #68 merged as `e401699a` with the user's
  explicit one-time coverage exception. CI 489 passed tests/lint/PostgreSQL
  checks, but counted rewrapped existing code below 80%: API 54.3%, worker 25%,
  core 55.54%. Images/E2E were skipped; this is not a new deployment verification.
  The normal threshold remains unchanged. PR #74 owns the tracing test correction.
- **Release readiness, P1:** CI 486 reported deployment success before the API
  rollout finished; an initial public health request returned 503 before recovery.
  Review API rollout availability, and make the owning release gate wait for all
  five deployments and healthy public HTTP before declaring success. Use shell
  mocks/fixtures to prove that an unready workload or HTTP 503 blocks success,
  and that ready workloads plus healthy HTTP pass. Keep this separate from
  Temporal's durable job acceptance and eventual output completion.
- PR #66 merged as `7fa4f451`; main CI 471 deployed it after 83 browser checks.
  The real FIT canary returned 202 in 0.30 seconds and preserved activity/import
  IDs. The dated rollout record owns the observed outputs and retry limitation.
  Formatting is isolated in #68, and custom DAG retirement is isolated in #69.

- Review baseline: PR #66 at `c00ad6f9`, 2026-10-16. R01–R11 preserve the
  numbering of the eleven review recommendations. Execute in the order below;
  the existing task inventory and rollout gates remain in force.
- P1 protects accepted work, source correctness, or the cutover boundary. P2
  simplifies implementation or completes diagnosis before broader rollout.
  Implement one bounded item per review. No new execution/admission ledgers,
  SQL orchestration, SQLite-specific tests, or E2E framework.
- Historical baseline check: 97 focused Docker units passed. CI 464/465 exposed
  SQLite fixture failures. Remove those suites, their schema/timestamp helpers,
  and SQLite dev-dependency features; retain pure tests and Postgres mocks.
  The retained activity-update/audit mock and five core model cases pass locally;
  all required hooks pass. CI 466 at `7a920cbb` passes tests/lint but fails the
  80% changed-line coverage gate: API 58.41%, worker 53.04%, core 62.29%.
  Images and E2E did not run after that gate. The user subsequently authorized
  the initial-production exception above; defer new mock coverage until real-data
  vetting, then restore the CI 80% gate. Do not restore SQLite tests or compatibility
  logic. R01 is implemented in PR #67 and remains separate from the first release.

| Order | Review | Priority | Work                                                    | Status          |
| ----- | ------ | -------- | ------------------------------------------------------- | --------------- |
| 1     | R01    | P1       | Atomic fitness invalidation                             | Ready to verify |
| 2     | R05    | P1       | Stable command identity through UI/API retries          | Ready to verify |
| 3     | R03    | P1       | Strava duplicate outcomes and removed segment scope     | Ready to verify |
| 4     | R02    | P1       | Source ordering and reconciliation                      | Pending         |
| 5     | R04    | P1       | Cancellation boundary across provider parents/children  | Pending         |
| 6     | R06    | P1       | Bounded webhook acceptance                              | In progress     |
| 7     | R09    | P2       | Separate legacy SQL bookkeeping from domain effects     | Pending         |
| 8     | R08    | P2       | Simplify debounce state and return domain outcomes      | In progress     |
| 9     | R11    | P2       | Permanent retained-input failures                       | Pending         |
| 10    | R10    | P2       | Native job diagnosis and causal context                 | Pending         |
| 11    | R07    | P1       | One owner for shared outputs and all selected producers | Pending         |

- **R01 — atomic fitness invalidation** (U10/U15/P02).
  Replace read/modify/write with one model-owned upsert that preserves the
  earliest dirty day and a monotonic source timestamp. Touch the existing
  publication fence on every invalidation, including an old timestamp arriving
  during a rebuild. Use current stored dirty state for incremental rebuilding.
  Mock cases: first invalidation, earlier/later dates, out-of-order source time,
  duplicate requests, stale rebuild rejection, and database failure. Preserve
  last rebuild/creation metadata. Implementation and four Postgres SeaORM mock
  contract tests are complete; database isolation proof remains P02. Three new
  tests failed against the original read/modify/write path, then all four passed.
- **R05 — stable command identity** (U02–U04/U25).
  Retain one typed UI command across uncertain retries; allocate a new UUID only
  for intentional new work. A duplicate start returns its canonical target or
  rejects changed work; never attach another activity's metadata to the old job.
  Mock a lost response, retry, closed job, and same-key/different-target request.
  Reuse Temporal's retained command information without an admission table.
  Implementation retains per-activity typed UI commands until acknowledgement
  and verifies immutable work against the exact native job/run's first history
  event. Changed work returns HTTP 409; revision/context changes on retry retain
  the original execution. No new persistence or orchestration layer. Focused
  Docker checks cover 76 native workflow/client tests, six API reprocessing tests,
  and seven UI hook/component tests. Two UI regressions, the changed-target
  native regression, and API conflict mapping failed against the old behavior.
  Review before rollout: exercise the actual admin flow and lost-response retry.
  UI state survives rerenders/target switches, not page reload; broader producer
  cutover and live retention/crash behavior remain separate work.
- **R03 — Strava retry outcomes and removed scope** (F1/F2; U07–U09/P01/P02).
  Recheck the original outcome immediately after acquiring the existing lock,
  before unchanged/superseded classification. Preserve minimal old/new refresh
  dependencies with the domain mutation, or use a justified broader rebuild.
  Mock a first lookup miss followed by another commit, and `{A,B}` → `{B,C}`
  followed by lost completion: retain the original outcome and refresh A/B/C.
  Implemented the locked outcome recheck before classification. Existing Strava
  mutation results now capture old memberships under the activity lock, including
  unchanged sources; rematching unions them with current dependencies. No new
  table, stage marker, or execution version. All 56 Strava domain tests pass in
  Docker, including source-scope persistence and lost-rematch-completion scope.
  The query-order contract and two source-update scope cases failed against the
  original implementation. PostgreSQL isolation and live lost-reply drills remain
  P01/P02; other legacy producer conversions remain R07.
- **R02 — source ordering** (F3; U18–U21/P03).
  Separate provider event time from Bike sync/backfill/receive time. Route
  owned source reconciliation, including fetching/applying, through per-source
  Temporal serialization/coalescing with existing tombstone/generation fences.
  Mock clock skew, distinct equal-second changes, delayed fetched content,
  delete/upsert overlap, and reconnect; preserve canonical webhook deduplication.
- **R04 — cancellation boundary** (F4; U12–U14/P04).
  Keep provider preparation cancellable and close cancellation before mutation
  and small durable handoffs, consistently through request-owned parents and
  children. Accepted cancellation stops new work; a late request settles the
  current item and reports the actual outcome. Shared outputs remain independent.
  Mock before-apply, commit-before-handoff, partial handoff, and batch boundaries.
- **R06 — bounded webhook acceptance** (U25/P03).
  Bound the complete durable acceptance operation below Strava's two-second
  callback budget. The existing inbox is acceptable; no additional inbox is
  required. Return failure promptly on timeout/uncertain acceptance and preserve
  deduplication after a lost reply. Use delayed/success/failure store mocks.
  The existing intake store seam now has one 1.5-second deadline around the
  complete acceptance operation. Controlled Tokio clock/store fixtures cover
  per-stage delays, the cumulative budget, prompt success/failure, and commit
  before a timed-out reply followed by canonical retry. All 15 intake units pass;
  the cumulative-budget regression fails without the deadline. Required hooks,
  CI, and deployed callback verification remain open.
- **R09 — legacy bookkeeping** (U05/U17; simplicity gate).
  Move SQL pipeline compatibility recording into legacy callers so native domain
  operations need no SQL execution/readiness records. Preserve old consumers
  until their cutover and retire unused attempt-correlation fields only through
  new migrations. Mock native effects without pipeline writes and legacy adapters
  with their required recording; preserve historical migrations.
- **R08 — debounce and domain outcomes** (U10/U15/U16).
  Live inspection found ten idle output coordinators remaining `Running` after
  successful publication, with no pending Activities, Workflow Tasks, timers, or
  failures. Finish a drained coordinator and let native Signal-With-Start create
  the next run for its stable resource key. Keep completion/reuse atomic across
  a racing notification; mock drain, completion/signal race, and a later new run.
  The user prioritizes this correction before resuming source ordering. Lifecycle
  implementation passes 78 native units and SDK replay of all ten real histories;
  deployment, graceful retirement, and a later live publication remain required.
  Return actual publication outcomes from domain operations instead of inferring
  them with surrounding reads. Remove unused `published` state and unsupported
  blocked/superseded behavior; retain resource, merged pending work, priority,
  timers, and compatible continuation input. Mock concrete domain adapters,
  late lower revisions, active work plus one follow-up, and retained history.
- **R11 — permanent retained-input failure** (U06).
  Distinguish immutable checksum/size/parse failures from transient backend
  failures with typed errors. Mock corrupt primary/supplemental bytes as terminal
  and temporary storage faults as retryable; do not repeatedly parse corrupt input.
- **R10 — native job diagnosis** (U14/U23/P04).
  Link returned native jobs to authenticated history or a thin authorized status
  endpoint. Display acceptance separately from completion/failure; preserve root
  actor/owner/correlation/trace context through Activities, children and handoffs.
  Mock failure before a domain audit exists, ownership, retry, and merged causes.
  Reuse Temporal history and existing activity audit rather than a new job table.
- **R07 — shared-output ownership and producer cutover** (F5/F6; U15–U17/U22/U26/P03/P04).
  After the preceding corrections and verification, route Strava, legacy single/
  bulk imports, edits/deletes, training, reads, and archive producers through
  neutral keyed refresh handoffs. Drain/fence legacy writers at the selected
  boundary; source completion waits only for handoff acceptance. Preserve deployed
  Strava history compatibility. Mock mixed producer bursts, prompt/new versus
  background/archive priority, pending projections, and replay before activation.

### Task inventory and conversion order

Sources: [registered processors](../bike-rs/worker/src/tasks/mod.rs),
[task names and payloads](../bike-rs/bike-core/src/jobs/adapter.rs), and
[processor implementations](../bike-rs/worker/src/tasks/processors/).
Dependencies below describe the intended conversion order.

| #   | Task                                     | Work / dependencies                                                                    | Task-specific verification                                                                                                                  | Status      |
| --- | ---------------------------------------- | -------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- | ----------- |
| 0   | `email_notification`                     | Retire the unused legacy-auth task; do not convert it.                                 | SSO works without Bike SMTP; no pending legacy mail remains.                                                                                | Pending     |
| 1   | `reprocess_activity_import`              | Establish native resource/output coordination and replay one retained source.          | Activity identity survives replay; duplicates, competing writes, and late attempts cannot publish stale results.                            | In progress |
| 2   | `rebuild_fitness_freshness`              | Move every fitness producer onto its independently keyed workflow established with #1. | A mixed sync/upload burst triggers one rebuild per window, with correct data, priority promotion, and bounded waits.                        | Pending     |
| 3   | `rebuild_segment_analytics`              | Rebuild affected segments through independently keyed analytics workflows.             | Results match fixture efforts; an older rebuild cannot overwrite newer data.                                                                | Pending     |
| 4   | `prepare_heatmap`                        | Prepare activity generations; move the reconciliation loop with this task.             | Only the current generation publishes; privacy changes and deletion reject stale output.                                                    | Pending     |
| 5   | `regenerate_activity_segments`           | Rematch one activity; refresh affected segment/activity analytics. Reuse #3.           | Repeated matching creates no duplicate efforts; affected caches are current.                                                                | Pending     |
| 6   | `regenerate_segment_efforts`             | Rematch one segment across activities; refresh affected caches. Reuse #3.              | One current effort set remains after overlap with #5 or a segment edit.                                                                     | Pending     |
| 7   | `backfill_user_xc_training`              | Rebuild a user's activity training analysis and goal-backfill status.                  | A competing import does not strand the backfill; retry/cancel leaves accurate status and releases coordination.                             | Pending     |
| 8   | `process_activity_import`                | Parse a retained file, persist an activity, and publish derived results. Reuse #2–4.   | FIT/TCX/GPX imports finish; duplicate submission/restart leaves one activity and current derived outputs.                                   | Pending     |
| 9   | `reprocess_user_activity_imports`        | Page through one user's eligible imports; run #1 children.                             | More than 16 activities resume without skips; child failures prevent a successful parent result.                                            | Pending     |
| 10  | `reprocess_archive_fit_activity_imports` | Run #1 for a user's archive FIT sources only.                                          | Selection excludes other sources; final counts remain correct after page retry.                                                             | Pending     |
| 11  | `regenerate_user_segments`               | Page through a user's cycling activities; run #5, then #3.                             | More than 16 activities finish matching and handoff; analytics converge afterward.                                                          | Pending     |
| 12  | `activity_archive_import`                | Retain archive/manifest; process bounded entry batches through #8.                     | Resume a multi-gigabyte, thousand-entry archive after mid-Activity crashes and Continue-As-New, without skips, duplicates, or reset counts. | Pending     |

- Batch conversions use bounded children, await their results, and carry progress/failures through continuation. Keep sources available until dependent work finishes.

### Findings requiring closure

Reviewed on 2026-10-16 at PR #64 head `93b93db9`. Findings describe required
behavior, not a new table design. Partial coverage in the reprocess review does
not close other paths; 68 Bike/168 focused Strava mock passes at review were not
database race or live recovery proof.

| ID  | Finding / current scope                                                                                                                                                                      | Remaining correction and acceptance                                                                                                                    |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| F1  | [Strava persistence](../bike-rs/bike-core/src/strava/native/persistence.rs) now rechecks stored outcome immediately after the existing lock. R03 implementation ready for verification.      | Preserve original `Applied` outcome on overlapping retry. U07/U08 mocks pass; P01 remains. No new ledger.                                              |
| F2  | [Rematching](../bike-rs/bike-core/src/activity_lifecycle.rs): native reprocess retains audit scope; Strava now retains source mutation scope before rematching. Legacy producers remain R07. | Union captured and current dependencies so removed segments survive retry. U09 units pass; P02 remains.                                                |
| F3  | [Sync event construction](../bike-rs/bike-core/src/strava/events.rs) mixes Bike time with [provider state ordering](../bike-rs/bike-core/src/entities/strava_activity_states.rs). Open.      | Separate clocks/provenance; serialize/coalesce remote-object updates; reconcile equal-second and out-of-order fetches. U18–U21/P03.                    |
| F4  | Original cancellation fixture suppressed handoffs after a late source commit without a safe cutoff. Reprocess now tests before-apply vs too-late cancellation.                               | Verify other converted paths accept cancellation only before apply/handoff or at safe batch boundaries; stop owned work after acceptance. U12–U14/P04. |
| F5  | Original attempt wrapper lacked concrete source/API/worker adapters. The concrete review slice replaces it.                                                                                  | Finish producer-wide cutover, deployment, recovery, and user acceptance before declaring conversion complete.                                          |
| F6  | [Strava upsert](../bike-rs/strava-temporal/src/workflows/activity_upsert.rs) rebuilds per item. Open.                                                                                        | Neutral asynchronous shared outputs, coalescing/priority, source-only collection completion, and compatible history evolution. U15–U17/U22/P03.        |

### Coordination, storage, and archive follow-ups

- [ ] Measure mixed new-activity/deletion/archive bursts after the proof of concept.
      Tune quiet windows/caps, bounded concurrency, fairness across users, and
      low-priority backlog aging. Preserve native priority 1/5 and verify deployed
      `matching.enableFairness`; investigate native Schedule-To-Start promotion
      if needed rather than building a separate scheduler.
- [ ] Implement native reference-aware scheduled cleanup for owned namespaces:
      persist lifetime/reference/pin state independently of history retention,
      collect complete paginated Temporal/DB/storage inventories, coordinate
      deletion with writers, and dry-run protected/candidate/reclaimed bytes.
      Missing inventory retains inputs; canonical sources have no scratch TTL.
      Cover active/resettable history, shared refs, pins, inventory errors, and races.
- [ ] After Bike conversions, finish Strava's remaining storage composition,
      source/reference pins, retention inventory, and cleanup on the shared service.
      Preserve raw captures, encryption, logical keys, serialized refs, separate
      queues, and retained-history replay. Do not defer F1–F4/F6 to that cleanup.
- [ ] For task 12, implement real streamed download/extraction, retained immutable
      archives/manifests, stable entry IDs/counts, bounded batches/continuation,
      and safe resume from older heartbeat checkpoints. Prove multi-gigabyte,
      thousand-entry recovery during download/extraction, after entry commit,
      between batches, across continuation, after missed heartbeats, and during
      paused-import TTL sweeps. Measure bounded memory/offsets/counts/rebuilds.
- [ ] Retire legacy email processor/templates/mail dependencies/UI descriptions/
      SMTP wiring. SSO owns account verification/recovery mail; source audit found
      no application enqueue caller. Verify live SSO login/session and resolve
      pending legacy email; the audit did not prove provider email delivery.
- [ ] Verify the forward image and retry the standalone segment-regeneration
      failure recorded in the existing SQL canary ledger. Preserve its three
      focused regression checks; do not describe SQL canaries as native proof.

### Worker loops and final retirement

- [ ] Move 15-second heatmap reconciliation with task 4 using native scheduling
      and generation-aware operations.
- [ ] Replace import startup/expired-task recovery with native retry/timeouts and
      idempotent import effects as their conversions are verified.
- [ ] Replace SQL batch reconciliation, child barriers, and archive scratch
      cleanup with awaited bounded children in tasks 9–12 and shared retention.
- [ ] After all 12 tasks are user-verified and email retired, resolve every
      pending/running/failed SQL task, open batch, and stranded import lock.
- [ ] Remove the SQL worker executable/deployment/Compose/build targets,
      processor registry, queue sampling, attempt/diagnostic retention, recovery
      loops, scrapes/dashboards, and task-management paths. Keep useful domain
      status/audit and native metrics/history in the combined `temporal-worker`.
- [ ] Preserve historical migrations and retained input/audit records. Retire
      schema through new migrations. Verify image identity, migrations/readiness,
      metrics, retained Strava histories, and representative user results.
- [ ] For each conversion, record focused checks/hooks/CI, owning queue/prefix,
      status/history, retry/cancel behavior, actual outputs, same-request retry,
      and interruption during an executing Activity. Mark `Ready to verify` only
      after its gate; mark `Verified` only after observed downstream results. Rollback drains
      or fences dispatch before switching ownership.

## Model boundaries and fixtures

- [x] Declare SeaORM insert defaults for the existing
      account, activity/import, analytics, segment, preference, flag, and task models.
      Preserve explicit values and append the missing task defaults migration.
- [x] Share the ordinary SeaORM update hook beside each owning model; remove the
      central seed registry and keep queue claim identities explicit.
- [x] Reuse shared task, preference, and feature-flag insert fixtures with typed
      struct-update overrides and one generic insert helper. Keep migration files
      focused on schema changes without the new migration unit-test block.
- [x] Apply shared insert/update fixture helpers throughout Rust unit tests;
      verify partial-update and explicit-clear behavior and affected test modules.
- [x] Verify in Docker on 2026-10-15: 220 affected unit tests and required Rust
      formatting/Clippy hooks pass; two existing large real-archive tests stay ignored.
- [ ] Verify full workspace and changed-line coverage CI for the insert-defaults PR.

- [x] Audit runtime query builders and repeated test-model construction; record
      scope, counts, and migration acceptance in [the spec](specs/model-boundaries.md).
- [x] Share import/activity/account/connection fixtures through test-only core
      support; retain scenario overrides and generated-ID insert helpers.
- [x] Require simple call sites, entity-owned queries, fixture factories, and
      shared test helpers in `AGENTS.md` and the model/fixture specification.
- [x] Move the audited activity/import controller queries onto owning models;
      preserve owned pagination, import hydration, source selection, and list limits.
- [x] Move segment access, dedupe, leaderboard reads, and deletion persistence
      onto entities and cohesive domain operations.
- [x] Consolidate preferences, fitness, report, and goal queries on their owners.
- [x] Move import recovery/lifecycle, batch, and worker persistence queries onto
      owning entities; preserve cancellation, retry, locks, and revision fences.
- [x] Move diagnostic/retention, heatmap, auth, feature-flag, and Strava query
      construction behind owning methods; preserve transaction and privacy policy.
- [x] Consolidate remaining repeated model families from the fixture audit,
      excluding intentional partial updates and scenario-specific overrides.

- [x] Complete mutation boundaries for the 24 constructor sites and conversion-based
      writes; preserve the baseline and acceptance evidence in the model-boundary spec.

## Strava

- [x] Define the new Temporal design and correlation fields in [the spec](specs/background-work.md#strava-protocol-and-workflows).
- [x] Delete gateway code and designs; preserve registered external URLs as native handler aliases.
- [x] Verify the registered URL and TLS live; preserve native callback aliases.
- [x] Accept unsigned Strava webhook POSTs as a provider protocol limitation; keep subscription, ownership, generation, and deduplication checks.
- [x] Bind OAuth state to the initiating browser session, consume it once, and verify its HMAC with the library's constant-time check.
- [x] Unit-test webhook validation/deduplication and OAuth state protections with mocked dependencies; use no database or running services.
- [x] Retire obsolete tables and jobs with new migrations; preserve existing migrations.
- [x] Delete old Strava SQL processors and checkpoint orchestration; exclude Strava from generic recovery and replay.
- [x] Store indexed Strava events and put customer, remote, event, and Temporal IDs in structured logs.
- [x] Use typed mutation and attempt statuses; store mutation status on the event row and count children without inspecting JSON.
- [x] Centralize queue names, use typed Temporal definitions, share the recovery timeout, and split connection validation and deletion into named functions.
- [x] Centralize required activity IDs on Strava events and hide state lookup/presence checks on the model; cover matching and supersession with inline mocked units.
- [x] Split scheduled sync into request, page, child-start, child-completion, and continuation functions; centralize workflow error construction.
- [x] Apply the two-expression conditional rule, share typed activity execution and handoff log context, and use ORM create/update with isolated unit coverage.
- [x] Call activities as `run(ctx, RevokeConnection, event)` using native SDK markers; derive display summaries centrally.
- [x] Correlate collection and scheduled child workflows with parent/child Temporal run IDs, root request/trace/customer/activity IDs, and replay-safe lifecycle logs.
- [x] Share fixture response writes; unit-test child trace propagation, failure/cancellation/continuation logs, and replay log suppression without running services.
- [x] Implement separate native create, update, upsert, delete, sync, backfill, disconnect, and scheduled-sync workflows.
- [x] Implement the Temporal schedule and durable API handoff.
- [x] Run the new worker with Bike's local Docker stack.
- [x] Remove the new PostgreSQL workflow test and Temporal test-server CI setup.
- [x] Unit-test all eight workflows with fixture-backed activity and child mocks; require no running services.
- [x] Unit-test activity handlers with mocked provider, persistence, and storage dependencies.
- [x] Link customer/activity integration history to Temporal job history; document processors and typed inputs/outputs.
- [x] Update UI unit fixtures for accepted Strava events and enum-owned SQL processor names.
- [x] Inventory infrastructure worker, Strava, system-health, and Temporal dashboards, metrics, alerts, and data sources.
- [x] Replace retired panels, alerts, and trace sampling; retain metrics for other Bike workers.
- [ ] Verify Grafana values, time ranges, missing data, and alert delivery against running services.
- [x] Publish the Bike and infrastructure PRs for review.
- [x] Restore blocking 80% changed-line coverage after the 2026-10-14 release; the approved exception covered one release only. Coverage reports remain required.
- [x] Verify isolated adapter/lifecycle/handler coverage with the normal 80% gate. CI 394: API 89.74%, worker 94.44%, Temporal 96.58%, core 100% on changed lines; 72 focused units pass.
- [ ] Cover remaining process bootstrap and SDK metrics-exporter startup with mocked dependencies. Socket/database startup and OS signal wiring remain outside the isolated tests.
- [x] Deploy the native worker, both metrics scrapes, and six search attributes to Nibelheim.
- [ ] Provide authenticated Temporal history access.
- [x] Define native workflow-failure, event-handoff backlog/freshness, and both scrape-endpoint alerts with fixtures.
- [ ] Deploy the native handoff exporter before its alerts; verify real metric samples and alert delivery.
- [ ] Repair retained alert runbook links to retired documentation.
- [x] Verify the deployed image, migrations, registered URL, and a real customer activity exchange. Scheduled sync upserted Strava activity `20424619693` as Bike activity `1676` on 2026-10-14.
- [x] Coordinate cutover and backups for the native writer, Temporal history, and retained activity files; review storage protection before retirement. Both disks validated backup `20261014T110418Z` with all three writers paused and resumed.
- [x] Verify shared trace continuity after enabling the native Temporal crate targets. A repeated real sync returned one unchanged activity and zero failures; list, activity, and streams exchanges shared trace `8d61988be672f15ffda11303fee1fe77`.
- [x] Observe Strava-originated POSTs on the registered route. Real creates `20582557310` and `20582644055` reached `/webhooks/strava-gateway` on 2026-10-14; the dated rollout record preserves their outcomes.
- [x] Retain all Strava summaries, descriptions, and gear in the feed; skip streams for manual rides and non-cycling summaries. Fixture checks and the live manual ride recovery passed on 2026-10-14.
- [x] Centralize sport/cycling policy and Training/Race in backend enums; generate frontend types and runtime options through OpenAPI. Preserve unknown provider sports without admitting them to cycling calculations.
- [x] Bound Strava Temporal JSON to 256 KiB, keeping large provider bodies and parsed data in object storage. Cover oversized callbacks/results, a 3 MiB provider body, and 150,000-point streams with isolated units.
- [x] Define event/source retention and protected reference roots in the Strava spec. Keep canonical activity sources; retain successful workflow scratch for at least 14 days and failed scratch/audit events for at least 90 days after the final execution closes.
- [x] Test restart reconstruction, populated large histories, and failures across scheduled-scan pages with isolated native workflow units. Cover child-start failures and continuation within a page.
- [x] Deploy the summary fix and rerun failed Create/Upsert for `20582644055`; Bike activity `1679` retains distance, elevation, description, and gear. A subsequent scheduled scan completed without duplicates.
- [ ] Observe a live non-cycling summary that remains present in Strava. Walk `20582557310` received a real delete webhook at `14:40:21 UTC`, so it correctly remains absent after the rollout; fixture coverage verifies cycling exclusions.
- [x] Verify a live native-worker stop/restart with recorded-history replay. Queued reset of retained ride `20424619693` resumed after four recorded completions and finished in 1.66 seconds; Bike activity `1676` remained unchanged and unique.
- [ ] Verify an interruption while a provider or persistence activity is actively executing. The controlled stop/reset/restart drill queued its partial history while the worker was stopped.
- [ ] Support Temporal CLI camelCase JSON exports in the read-only native replay
      tool; file mode currently reports `missing field event_id`. Preserve the
      verified `--workflow` fetch mode and execute no Activities during replay.
- [ ] Verify live backfill and disconnect behavior; retain the observed create,
      update/sync, and delete evidence without treating it as proof of every operation.
- [ ] Resolve the release-tool warning that reports a successfully completed migration container as a warning. CI 403 applied revision `535cdf3e`; Kubernetes confirmed Job `Complete`, exit 0, and no pending migrations. Do not describe this release log as warning-free.
- [x] Measure observed production resource use through Prometheus. Over the inspected six hours, native worker memory peaked at 15.6 MiB of 512 MiB and shared Temporal at 130.4 MiB of 1 GiB; maximum five-minute mean CPU was 2.7m and 38.3m respectively. This records the actual small workload, not a capacity benchmark.
- [ ] Qualify capacity for large backfills and sustained customer concurrency, including shared PostgreSQL resource use.
- [ ] Resolve Temporal's initial `Frontend is not healthy yet` startup warning; keep diagnostics enabled.
