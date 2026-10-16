# Bike TODO

## Worker conversion

- [Background work and Strava integration](specs/background-work.md) owns behavior,
  identities, storage, cancellation, priority, and U01–U26 / P01–P04 acceptance.
  This backlog is the single owner of task status, findings, and open work.
  Keep dated evidence in [the rollout record](worker-rollout-2026-10-16.md).
- PR #61 supplied groundwork. PR #66 consolidates the concrete reprocess review
  slice; #64 and #65 are superseded. Strava is already native. Single-endpoint
  wiring is not production cutover of all producers or shared outputs.
- Convert one task at a time and stop for user review after each. Status advances
  `Pending` → `In progress` → `Ready to verify` → `Verified`; email becomes `Retired`.
  A failed gate keeps the task open. Do not rename registered tasks without user
  approval/explanation. Email retirement is independent of reprocessing.

### First deliverable: complete reprocessing

- [x] Record the user's 2026-10-16 coverage exception for the initial production
      worker rollout. CI measures and retains coverage with no percentage minimum;
      normal `coverage:check` remains 80%. Tests, lint, images, and release checks
      remain blocking. Prioritize a working deployment and real retained data.
- [ ] After real-data vetting, restore the CI 80% gate and fill meaningful unit
      coverage using PostgreSQL mocks/shared fixtures. Do not restore SQLite tests,
      database emulation, timestamp massaging, or compatibility behavior.
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
      the existing Docker stack. Stop for user review before broader rollout;
      extend the existing CI E2E suite only after the working flow is accepted.
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
- [ ] Complete applicable U01–U17/U23/U26 and affected provider U18–U22/U25 cases;
      record each test/command/commit/result and unimplemented variants. Pass the
      simplicity gate without SQL orchestration, redundant bookkeeping, or a new
      generic recovery service.
- [ ] After user vetting, prove P01/P02/P04 in the existing stack/CI suite and P03
      where provider routing/history changes. Include acceptance while the worker
      is stopped, commit-before-lost-reply, partial handoff, cancellation races,
      and real eventual outputs. Keep mock, DB, runtime, and deployed evidence distinct.
- [ ] Finish shared command/actor/owner/request/correlation/trace propagation
      through all producers, Activities, children, fan-in, retries, replay, and
      continuation. Verify native visibility and live API-to-workflow traces;
      shared output logs must not attribute merged work solely to the last caller.

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

| ID  | Finding / current scope                                                                                                                                                                 | Remaining correction and acceptance                                                                                                                    |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| F1  | [Strava persistence](../bike-rs/bike-core/src/strava/native/persistence.rs) rechecks stored outcome after content/supersession branches. Open in provider path.                         | Recheck under lock first; preserve original `Applied` outcome on overlapping retry. U07/U08/P01; no new ledger.                                        |
| F2  | [Rematching](../bike-rs/bike-core/src/activity_lifecycle.rs) can lose removed segment scope on retry. Reprocess now retains minimal audit scope; other rematch paths remain open.       | Preserve/reconstruct old/new dependencies with a plan/version/scope or broader rebuild. U09/P02.                                                       |
| F3  | [Sync event construction](../bike-rs/bike-core/src/strava/events.rs) mixes Bike time with [provider state ordering](../bike-rs/bike-core/src/entities/strava_activity_states.rs). Open. | Separate clocks/provenance; serialize/coalesce remote-object updates; reconcile equal-second and out-of-order fetches. U18–U21/P03.                    |
| F4  | Original cancellation fixture suppressed handoffs after a late source commit without a safe cutoff. Reprocess now tests before-apply vs too-late cancellation.                          | Verify other converted paths accept cancellation only before apply/handoff or at safe batch boundaries; stop owned work after acceptance. U12–U14/P04. |
| F5  | Original attempt wrapper lacked concrete source/API/worker adapters. The concrete review slice replaces it.                                                                             | Finish producer-wide cutover, deployment, recovery, and user acceptance before declaring conversion complete.                                          |
| F6  | [Strava upsert](../bike-rs/strava-temporal/src/workflows/activity_upsert.rs) rebuilds per item. Open.                                                                                   | Neutral asynchronous shared outputs, coalescing/priority, source-only collection completion, and compatible history evolution. U15–U17/U22/P03.        |

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
      after its gate; mark `Verified` only after user acceptance. Rollback drains
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
