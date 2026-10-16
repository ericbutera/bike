# Worker conversion plan

- Goal: resolve all 13 SQL task types: convert 12 to native Bike tasks and retire legacy email. Keep Strava separately owned.
- `reprocess_activity_import` is the first conversion in progress. Native
  coordination groundwork is implemented; production command routing and domain
  adapters are not switched. Existing Strava workflows stay native.
- Execution contract: [background execution](specs/project-overview.md#background-execution).
- Shared unit of work: [durable-work contract](specs/durable-work-contract.md).
  It owns identity, minimal persistence, review findings F1–F6, and acceptance
  scenarios U01–U26 / P01–P04 for both Bike and Strava.
- Workflow composition and proof-of-concept contract:
  [named Bike task workflows](specs/bike-task-workflows.md). Keep registered task
  names exactly as listed below; renaming requires user approval and explanation.

## One task at a time

- PR #61 established workflow composition; PR #64 adds submission groundwork.
  Neither marks a production conversion complete. The current unit of work is
  the complete `reprocess_activity_import` slice below, including correction of
  relevant review findings. Task 0 remains retirement, not a new workflow.
- Start with reprocessing and its shared coordination dependencies, then work
  through the remaining inventory with one task conversion per PR. Email
  retirement is independent and does not block the first conversion.
- Status: `Pending` → `In progress` → `Ready to verify` → `Verified` (or `Retired` for email).
- Complete the verification gate and stop for the user's review before starting the next task.
- Keep the SQL worker for unconverted tasks. Each converted type has one execution owner.
- A failed gate keeps that task open. Fix it before proceeding.

## Separate concerns

| Boundary                                       | Strava            | Bike tasks (target) |
| ---------------------------------------------- | ----------------- | ------------------- |
| Workflow/Activity code and worker registration | `strava-temporal` | `bike-temporal`     |
| Task queue                                     | `bike-strava`     | `bike-tasks`        |
| Workflow ID prefix                             | `strava-`         | `bike-task-`        |
| Logical object-storage prefix                  | `strava/`         | `bike/tasks/`       |

- Run separate worker processes; each registers only its own workflows/Activities. They may share the Temporal service/namespace, filesystem volume, neutral infrastructure, and domain operations.
- Keep callbacks, OAuth, provider captures, and Strava-specific manifests with Strava. Neither workflow layer imports the other's workflow/Activity types; provider Activities submit typed change intents through a neutral domain scheduling interface.

## Email retirement: independent of reprocessing

- Source audit, 2026-10-15: [login](../bike-ui/app/login/page.tsx) uses OAuth; [auth routes](../bike-rs/bike-core/src/auth/controllers/auth.rs) provide sessions, with separate [OAuth routes](../bike-rs/bike-core/src/auth/controllers/oauth.rs). No application caller enqueues email; only the unused adapter, processor, and test references remain.
- Account verification/recovery mail belongs to the SSO provider. Remove Bike's email processor, templates, unused mail dependencies, UI descriptions, and SMTP wiring; keep provider-owned email configuration.
- Verify deployed SSO login/session behavior and resolve any pending legacy email tasks before marking retirement complete. This audit did not test live provider email delivery.

## First conversion: shared groundwork

- Build the dedicated Bike task worker using the existing typed SDK and workflow-mock patterns; preserve Strava's worker registration, queue, and histories.
- Use the [shared object-storage service](specs/project-overview.md#shared-object-storage)
  for bytes, descriptors, references, and retention policy. Inject the backend;
  the filesystem is the initial adapter. Preserve Strava's existing logical keys
  and serialized references.
- Retain exact input/large-result bytes using content hashes and atomic durable
  publication. Test storage and TTL contracts once with shared in-memory fixtures
  and a focused filesystem restart check; each task tests its domain behavior.
- Establish command idempotency, shared output workflows, and conditional
  domain-write fencing with reprocessing. Temporal owns orchestration; do not
  introduce a SQL scheduling bridge, custom queue, or SQL lock/barrier dependency.
- Task 1 must address the [recalculation policy](specs/project-overview.md#derived-work-priority-and-debounce) across uploads, sync, deletes, and fitness reads. Converting the queue alone does not fix repeated computation.
- For each conversion, find every producer, including direct SQL inserts, child tasks, and batch completion checks. Route them through the selected task's native submission/result contract.
- Drain existing SQL work for that type before switching ownership. Replace all
  command producers and batch result contracts at the native boundary together.
  If that boundary is incomplete, keep the conversion in progress.
- Native Activities call domain operations directly. Do not wrap SQL task
  submission or use a SQL batch to coordinate native results.
- API producers start Temporal directly and return its acknowledged job reference.
  Domain Activities use natural keys, model versions, and transactions for safe
  effects; preserve enough refresh scope without a universal outcome ledger.
  Direct starts may fail the API request. An inbox is optional, not a new Bike
  prerequisite. Follow the shared
  [handoff contract](specs/bike-task-workflows.md#handoff-and-completion).

### First deliverable: complete reprocessing

- [ ] Implement the real import adapter using model-owned idempotency. Preserve
      activity/import identity and eventual refresh of removed dependencies.
      Choose existing keys/versions, a retained plan, or a broader rebuild before
      adding an outcome record.
- [ ] Wire the API to direct acknowledged submission, with stable command identity
      and optional Bike activity identity at intake. Register and configure the
      dedicated Bike worker; prove acceptance while that worker is stopped.
- [ ] Finish fitness, segment analytics, and heatmap dependencies needed by this
      slice. All producers sharing those outputs use the same neutral handoff
      and keyed coordination, including Strava. Preserve provider ownership and
      retained-history compatibility.
- [ ] Correct relevant F1–F4 behavior at the changed boundary. Test cancellation
      before apply and a too-late/deferred request during apply/handoff. Keep
      exhausted failures visible with an operator retry or existing repair path.
- [ ] Replace/drain or fence every competing legacy producer at the selected
      boundary. Do not switch both SQL and native execution on for the same work.
- [ ] Implement and record U01–U17, U23, U26 and affected Strava scenarios, plus
      P01, P02, P04 from the shared contract. Record P03 where provider routing or
      history changes. Mock-only tests do not establish database/runtime recovery.
- [ ] Pass the simplicity gate: one execution owner, reused domain/storage/fixture
      paths, typed small inputs/results, no SQL scheduling bridge or redundant
      attempt-stage lifecycle. Record each retained record's distinct purpose.
- [ ] Verify the real admin reprocess flow and eventual outputs, then stop for
      user review before broader rollout. This document does not authorize a
      deployment or merge.

## Task inventory and conversion order

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

## Native debounce and priority

- `process_activity_import` and `reprocess_activity_import` share an explicit
  `activity_import` Activity. Each workflow then hands off to the named fitness,
  segment analytics, and heatmap workflows; import completion means durable
  acceptance, not derived publication. Other workflows reuse these same steps.
- The derived workflow owns debounce for its output, regardless of the calling
  parent. Five quick requests from orchestration must not trigger five rebuilds.
- Use Temporal's [Event Accumulator](https://docs.temporal.io/design-patterns/event-accumulator) and [Updatable / Debounced Timer](https://docs.temporal.io/design-patterns/updatable-timer) patterns: Signal-With-Start, deduplication, and durable timers.
- Use native [Task Queue Priority and Fairness](https://docs.temporal.io/develop/task-queue-priority-fairness), including Activity priority overrides; verify the pinned SDK/server configuration during task 1.
- Initial configurable windows: new activity **2s quiet / 10s cap**, priority **1**; delete/background recalculation **60s quiet / 5m cap**, priority **5**. Caps bound debounce, not total execution time.
- Coalesce by user/output or segment; retain the earliest dirty date, affected-ID union, latest revision, and highest pending priority. Promote an existing low-priority batch when new activity arrives.
- Source persistence does not wait for a full user rebuild per sync child.
  Collections finish after source children and accepted handoffs; projections
  may remain stale afterward. No final aggregate barrier or per-request receipt.
- Archive/backfill entries keep background refresh priority. A new ride promotes
  pending fitness/segment/achievement work; deletion may finish slowly too.
- Verify no inline fitness-read bypass, no duplicate recomputation for unchanged inputs, one follow-up for changes arriving during execution, fair service across users, and eventual service for aged low-priority work.
- Preserve new-activity high priority and deletion/background low priority in
  the proof of concept. Revisit aging and final timing policy after it works;
  these remain conversion gates rather than a custom scheduler to build first.

## Filesystem object TTLs

- Implement a native scheduled sweeper with [object TTLs](specs/project-overview.md#object-ttls): abandoned submissions **24h**, successful scratch **14d**, failed/canceled scratch **90d**; canonical sources remain while referenced.
- Verify expired objects are reclaimed, active/retry/history/database references stay protected, and each concern's sweeper stays inside its own prefix. Record reclaimed files/bytes and cleanup failures.

## Large archive durability: required for task 12

- Follow the [archive recovery contract](specs/project-overview.md#large-archive-durability). Keep the immutable archive and paged manifest on the persistent filesystem; replacement workers reuse the same references.
- Stream download/extraction and process bounded entry batches. Workflow history records completed batch/child results; Continue-As-New carries the next settled-batch cursor, counts, dirty revisions, and object references.
- Long download/extraction Activities use native [heartbeat details](https://docs.temporal.io/encyclopedia/detecting-activity-failures#activity-heartbeat) for the stage, entry cursor, and last durably committed byte/chunk offset. A retry reads and validates the last received checkpoint.
- Heartbeats may be throttled or lost with a crash: expect a retry to repeat work after an older checkpoint. Stable entry IDs and idempotent writes/counting prevent duplicates. A compressed archive uses manifest-entry cursors, not arbitrary compressed-byte seeks.
- Verify worker loss during download, extraction, after entry commit/before completion acknowledgement, between batches, and across Continue-As-New. Include restart after a missed heartbeat and TTL sweeps while the import is paused.
- Use mocked time/large manifests for focused units, then a controlled multi-gigabyte archive with thousands of entries for runtime proof. Measure bounded memory, resumed offsets, entry/result counts, and coalesced rebuild counts; keep this task open until the user verifies the result.

## Verification gate: repeat for every task

- [ ] Update the owning behavior spec and cover the happy path with shared fixtures/mocks.
- [ ] Link every applicable shared-contract scenario ID to its test, command,
      commit, result, and evidence scope. Explain any inapplicable scenario;
      keep unimplemented scenarios and unresolved findings visibly open.
- [ ] Test duplicate submission, conflicting work, stale completion, cancellation, and a successful write followed by lost Activity completion. Cover shared-resource overlap with Strava and remaining SQL work where applicable.
- [ ] For derived-work tasks, use time-skipping fixtures to verify burst counts, both debounce windows, high-priority promotion, maximum waits, events during execution, and unchanged inputs. Measure actual rebuild counts on a deployed mixed sync/upload burst.
- [ ] Run the affected Docker/mise checks, required hooks, and CI; resolve every warning/error.
- [ ] Deploy this task and run its real user/admin flow. Check the database/output and user-visible result, then retry the same request.
- [ ] Stop the worker during an executing Activity; restart and verify correct recovery without duplicate effects. Long Activities must resume from native heartbeat details; archive task 12 also passes its dedicated recovery gate.
- [ ] Confirm the owning worker/queue and object prefix, status/history links, retry/cancel behavior, and task-specific verification. Strava must still run on its own queue and read its retained objects.
- [ ] Record the evidence below, mark `Ready to verify`, and stop. Mark `Verified` only after the user accepts the result.

## Verification record

Keep one short record per task as it is converted:

### Reprocessing groundwork (PR #61)

- Initial `bike-temporal` workflows used the exact task names for #1–4 and #8.
  Both imports share `activity_import` and explicitly hand off through typed
  Signal-With-Start Activities. Each derived workflow owns its resource ID,
  debounce timer, native Activity priority/fairness, and Continue-As-New state.
  The former generic coordinator and core `task_coordination/debounce.rs`
  state machine have been removed.
- The graph review now includes #5–7 and #9–12 with the same canonical names.
  Shared typed steps remove repeated import handoffs and SDK builders; one bounded
  manifest page loop and child caller serve the collection workflows. Segment
  batches reuse #5, reprocessing batches reuse #1, and archive entries reuse #8.
  Domain Activity adapters are grouped by imports, derived data, matching,
  training, and archives. These are proof-of-concept definitions, not completed
  database/producer migrations.
- Shared object storage uses the supported `object_store` backend interface.
  The local adapter enables durable writes; shared in-memory fixtures cover
  immutable objects, descriptors, references, ownership, and retention. Strava's
  wire format and logical keys remain compatible. Retention supplies candidate
  decisions and a serializable lifetime contract, without automatic deletion.
- The SDK workflow mock is shared infrastructure; Strava retains its provider
  assertions. Bike and Strava workflow packages do not import each other.
- The initial generic coordinator passed its fixture/mock checks; its tests are
  replaced by named workflow, shared Activity, native handoff, and output-level
  debounce checks. These remain unit proof, not deployed execution evidence.
- Docker `mise run checks:docker test:tasks:unit`: **169 tests passed**, including
  **30** named Bike workflow/Activity/client tests, **70** Strava workflow
  regressions, and **69** shared storage/TTL and Strava domain checks. Five quick
  fitness or segment requests coalesce; replay retains one active operation and
  one follow-up. Import workflow tests complete after accepted handoffs.
- Complete graph/refactoring follow-up: the same Docker task passed **197 tests**,
  including **58** Bike workflow/Activity/client checks and the same **139**
  shared-storage/Strava regressions. The new checks cover all twelve names,
  reused children, manifest-bound selections, a synthetic 273-entry archive
  across continuation, and cancellation racing with successful completion.
  The latter two regressions failed before the shared cancellation guard and
  passed after it. These tests do not prove real archive streaming or live
  domain publication.
- Review correction: the historical cancellation assertion does not establish a
  safe cutoff before apply/handoff. F4 and U12–U14 require that cutoff and stopping
  owned work after accepted cancellation, not post-cancellation finalization.
- Storage follow-up: Docker `mise run test:storage` passed **18 tests** covering
  the shared storage/TTL contract and Strava byte/manifest/exchange compatibility.
- Required Docker prek checks passed, including workspace Clippy with warnings
  denied. Full CI coverage and deployed behavior remain verification gates.
- Pending before cutover: concrete idempotent domain Activities, eventual refresh
  and safe cancellation, all producer/batch replacements,
  persisted retention/reference inventory and scheduled cleanup, dedicated worker
  deployment/configuration, and live duplicate/recovery/visibility verification.
- This groundwork has no SQL scheduling bridge and is not a released conversion.

- Task / PR / commit / deployed image / environment:
- Workflow ID / run ID / retained input reference:
- Unit and CI results:
- Applicable U/P scenario IDs, test paths, commands, evidence scope, open findings:
- User-visible result / duplicate, concurrency, and restart results:
- Remaining issues / user acceptance:

## Worker-owned background work

| Responsibility                                                                       | Disposition                                                                                                                    |
| ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------ |
| Heatmap reconciliation every 15 seconds                                              | Convert with #4 using native scheduling and generation-aware domain operations.                                                |
| Import recovery on startup; expired task recovery                                    | Replace with native retries/timeouts and idempotent import Activities; retire when import conversions are verified.            |
| SQL batch reconciliation, child barriers, and archive cleanup                        | Replace with awaited native children in #9–12; retain original sources and clean only disposable scratch.                      |
| Filesystem object TTL cleanup                                                        | Native scheduled cleanup with separate prefix ownership and reference-aware expiry; verify before retaining new task scratch.  |
| Processor registry, queue sampling, attempt history, and hourly diagnostic retention | Retire SQL execution diagnostics after the last task; retain useful domain status and provide native workflow metrics/history. |

## Final retirement

- After the Bike task conversions finish, complete the Strava shared-storage and
  retention integration recorded in [the backlog](TODO.md#worker-conversion).
  Keep provider workflows separate while reusing storage contracts and fixtures.

- [ ] All 12 converted tasks are `Verified`, email is `Retired`, and existing Strava workflows still work.
- [ ] No SQL producers, pending/running tasks, open batches, or stranded import locks remain. Resolve retained failed work explicitly.
- [ ] Remove the SQL worker executable/deployment/service, Compose wiring, obsolete build targets, recovery loops, scrapes, dashboards, and task-management paths. Keep the separate Bike task and Strava Temporal workers.
- [ ] Preserve historical migrations, retained input objects, and required audit history. Schema retirement uses new migrations.
- [ ] Verify deployed image identities, migrations/readiness, native metrics, and representative user results after removal.
- Rollback must stop new dispatch and drain or fence current executions before changing ownership; never activate both executors for the same work.
