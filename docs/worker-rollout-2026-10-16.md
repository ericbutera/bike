# Bike worker fail-forward rollout, 2026-10-16

## Output lifecycle deployed and verified, 2026-10-17 02:57 UTC

- PR #75 merged as `8fb5d9ff9a4254804fd73f31acf53ac40f6f6e6e`.
  Main CI 505 passed tests, lint, PostgreSQL checks, the normal 80% coverage gate,
  image builds, and all **83 browser cases**, then deployed. Deployment diagnostics
  contained no warnings/notices. The matching migration Job was `Complete`.
- Verified all five deployment rollouts and healthy public `/api/health` using
  the new owning release check. Both worker templates report revision `8fb5d9ff`.
  Immutable image hashes are API `8e2e2c240b230451cab86bbca152917a332114ee21cd4689360a0f3c9370fa5c`,
  both workers `3d148919c289226883691304c7e7a1c3db4765b994556a3f9dc6d0fe25e03ecb`,
  UI `a090c2e5f16fdc3c5bbd3b32b7cb6283b28c64d44f38718676358ed925098714`,
  and maps `2fc551cc71370c4d7bb7d845f17cc99a24a03e108d3abcebb16af14773d562ef`.
- After checking the live worker revision/readiness, sent native `drain` to the
  ten original runs from **17:58 EDT**: fitness user 1, heatmap activity 1588, and
  segments 8/9/13/17/21/22/26/28. Queried each original Run ID independently;
  all ten are **Completed**. No cancellation, termination, or reset was used.
- Started a new native reprocess through the Temporal CLI using the real retained
  FIT for activity/import 1588: artifact 1190, 352,993 bytes. This checks worker
  execution; it is not a fresh HTTP/admin UI acceptance test. Command
  `6f7fd399-1252-4149-81af-0f15674a2f49` carries system actor
  `worker-rollout-verification` and has exactly one domain audit.
  [Native job](https://temporal.nibelheim.dev/namespaces/bike/workflows/bike-task-reprocess_activity_import-user-1-6f7fd399-1252-4149-81af-0f15674a2f49)
  run `01a147cb-4fd5-7c71-9376-c07e9ac61b43` completed in **1.85 seconds**.
- All ten stable output Workflow IDs started new Run IDs and **Completed** in
  approximately **2–3 seconds**. Each performed one Activity, returned `Published`,
  and completed on attempt 1. All eleven histories have no failed/timed-out events.
  A subsequent visibility query found **zero Running Bike task workflows**.
- Verified stored outputs: one activity linked to the same import, unchanged
  title/sport/derived-data hash and **47 efforts**, unchanged fitness-day values,
  cleared fitness dirty day, and a new rebuild timestamp. All eight segment
  summaries have newer publication timestamps and effort counts equal their
  stored efforts. The unchanged heatmap correctly reuses ready generation **7**.
  SQL task count remains **1,996**. Full producer cutover and the rest of R08 remain.
- The UI Deployment now receives the public Temporal UI origin. The owning CI
  and manual deployment tasks have a native five-rollout/public-health gate;
  focused shell mocks cover unready, HTTP 503, unhealthy-body, and healthy cases.
  The check also passed read-only against this live release. Custom DAG retirement
  remains PR #69; these configuration/readiness changes do not establish its rollout.
- Runtime qualification remains open: subsequent SQLx warnings came from metrics
  COUNT queries, a feature-flag lookup, and a handoff poll (about 0.27–1.2 seconds,
  unchanged 250 ms threshold). The eight-core node's load was **35.06** with no
  memory/disk/PID pressure. This is a contention observation, not a proven root
  cause. The domain results above passed; do not describe the runtime as warning-free
  or change the operator's CI limits to hide the issue.

## Output lifecycle verification before deployment, 2026-10-17 02:04 UTC

- The lifecycle correction passes **78 native workflow/client/Activity units**.
  Six new regressions failed against the old implementation before the fix.
  Coverage includes all three output types completing, a later lower revision in
  a new run, same-activation completion/signal orderings, pending/active graceful
  drain, continuation, and replay of old multi-publication histories.
- Read and replayed all ten real running coordinator histories with the pinned
  SDK's native `WorkflowReplayer`: heatmap activity 1588, fitness user 1, and
  segments 8/9/13/17/21/22/26/28. All replayed successfully with the new code.
  The replayer fetches history directly through the SDK; it executes no domain
  Activities and makes no application writes.
- This is local and retained-history evidence. Deployment, graceful drain of
  those ten runs, and new-run publication/reuse are still pending.

## Live rename and running-workflow inspection, 2026-10-17 01:17 UTC

- A user-initiated title change for real Strava activity `20608722796` reached
  Bike at 01:17:05.599 UTC and was submitted to Temporal at 01:17:06.159 UTC.
  [UpdateActivity](https://temporal.nibelheim.dev/namespaces/bike/workflows/strava-52e5b689-1d46-479f-aff5-7d085ef8219c)
  completed in **1.560244050 seconds**; Bike activity `1683` stored the new title.
- Its Upsert child completed all nine Activities with zero failed/timed-out
  events. Fitness rebuilt at 01:17:07.505 UTC and cleared its dirty day. All five
  application deployments were ready; the worker's inspected 20-minute log window
  contained 33 JSON records and zero warnings/errors.
- Inspected all ten running `bike-task-*` executions: one fitness, one heatmap,
  and eight segment coordinators from the retained-input canaries. Each had three
  completed Activities, latest result `Published`, zero failures/timeouts, zero
  pending Activities/Workflow Tasks, and zero open timers. Their last publications
  were at 00:50:23–24 UTC. They wait indefinitely for another signal, so `Running`
  does not mean work is outstanding. R08 now explicitly tracks drained completion
  and safe native reuse of the stable resource Workflow ID.
- The UI's five failed executions are retained October 14 runs: one scheduled
  scan, its customer sync and Upsert child, plus an earlier Create/Upsert pair.
  Their failure histories report `Resource Not Found` or the enclosing child
  failure. None belongs to the October 16 rename. This was a read-only history
  check; do not replay historical creation work over a newer deletion.
- Strava still publishes derived outputs through its provider path. R07 owns
  moving those producers to neutral shared handoffs; these ten coordinators were
  not awaiting the renamed activity. This inspection changed no application data
  and canceled no executions.

## Read-only Strava confirmation, 2026-10-17 01:15 UTC

- Inspected production SQL identity records and native Temporal descriptions;
  submitted no new work and changed no application data.
- The scheduled 00:00 UTC sync (October 16, 8:00 PM EDT),
  `strava-d62bec3f-6e51-5997-a3ea-be225558c974`, completed in **3.270873047 seconds**
  with zero imports and one unchanged activity. Its native status was `Completed`.
- Child `strava-24124b0d-6276-50af-98bf-f2158014c975` completed in
  **1.679757269 seconds**. Real Strava activity `20608722796` retained Bike activity
  `1683` / import `1682`, sport `indoor_trainer_ride`. The provider create/update
  records from October 16 were `Applied`; the scheduled repeat was `Unchanged`.
- The SQL collection row still said `pending` although the native sync completed.
  Temporal is authoritative for execution status; R09/R10 track retiring or
  separating ambiguous legacy execution bookkeeping. This inspection confirms
  recent base sync behavior, not R03's new race/retry fixes or another crash drill.

## Verified batching release and worker restart, 2026-10-17 00:50 UTC

- PR #70 merged as `16ed60188310c69b3d90f72f30b66fa39f03c2fa` after
  [CI 484](https://ci.nibelheim.dev/repos/3/pipeline/484) passed all checks,
  verified release images, and passed 83 browser cases in 6.3 minutes.
  [Main CI 486](https://ci.nibelheim.dev/repos/3/pipeline/486) passed another
  83 browser cases in 6.4 minutes and deployed the release.
- The normal CI 80% changed-line gate is restored. All 160 changed covered core
  lines passed at 100%. Focused Docker validation passed 39 replacement/batching
  cases, seven projection transaction cases, 14 native reprocessing cases, and
  54 Strava domain cases. Unit databases and SQLite compatibility logic did not
  return; the replacements use PostgreSQL SeaORM mocks and shared fixtures.
- Projection persistence now inserts at most 500 summaries per statement inside
  the existing transaction. Ordered activity locks, fresh reads, rollback, field
  mappings, and database-owned insert timestamps remain intact. Production
  `activity_analytics` has `CURRENT_TIMESTAMP` defaults for both timestamps.
- All five application Pods were ready with zero restarts. API, both workers,
  and maps reported the merged revision. Migration Job
  `schema-migration-16ed60188310c69b3d90f72f30b66fa39f03c2fa` completed with
  `No pending migrations`. Verified configured image digests:
  - API: `f2e462dd2fc6bae91344d49ac549d7d0f0d08ea573baf89641a1e964af6eeca6`.
  - Both workers: `58bbe53aaf4dc27bfec8acbb920cee981ea2a6e5c390910159a6dcc33d697ddb`.
  - UI: `1eee0cb68d25a693c43cca3b50c24a1c793916ab4d65d1a9a63fd1ab0b4bcd4f`.
  - Maps: `543f34e66f85ce15d081cbc9563dce090f11d3790557bf448d22b6dfd66b4451`.
    Its unchanged running platform digest remains
    `1d4f73e3c70df60a8dde120d1f4d62ef05de12614b244325f92f35dc8b924b6a`.
- The infrastructure CLI update removed the earlier deployment update notice;
  the actual deployment output contained no warnings/errors. Existing CI limits
  were preserved, including the deployment step's 0.5-CPU limit. The cold Go SDK
  build advanced under that cap without OOM events.
- **Readiness gap:** CI reported success before the API rollout finished. The
  first public health request returned 503; after rollout, public health was
  healthy and all five deployments were ready. This is an explicit release-gate
  follow-up in the TODO, not evidence of uninterrupted API availability.
- With no other active source workflows, stopped only the combined Temporal
  worker and submitted activity/import 1588 using command
  `2b341217-b03f-4ef4-956a-91f9cd9e149e`. HTTP 202 returned in **0.297617 seconds**.
  Its waiting history contained only workflow-start and workflow-task-scheduled
  events. Activity/import timestamps, business data, and SQL queue counts remained
  unchanged while the worker was stopped.
- Restarting the worker completed the
  [native job](https://temporal.nibelheim.dev/namespaces/bike/workflows/bike-task-reprocess_activity_import-user-1-2b341217-b03f-4ef4-956a-91f9cd9e149e),
  run `01a14756-8f1b-7d48-8907-1c75d3e3be49`, in **4.368158272 seconds** including
  its queued wait. Root history had 29 events. The worker returned to one ready
  replica.
- Fitness, heatmap, and all eight segment Activities returned `Published` on
  attempt **1**, with zero failed/timed-out Activity events. Segment execution
  overlapped at a peak of **six**, with the last completing at
  00:50:24.480525459 UTC. Faster completions meant all eight did not overlap at once.
- Fitness rebuilt at 00:50:23.428736 UTC and cleared the dirty day before the
  fitness API was read. That read returned 200 in **0.176439 seconds**, preserving
  training load 122.3, fitness 62.6, fatigue 76.5, and form -13.9. The unchanged
  heatmap reused ready generation 7. Activity reads returned 200; activity/import
  IDs, title, sport, 47 efforts, one linked activity, and derived-content hash
  `1eabe440455f27f29e77448eb0a9b716` were preserved.
- Retrying the identical command returned 202 in **0.277101 seconds** with the
  same job/run and exactly one audit. SQL task counts stayed 1,991 completed and
  five pre-existing failures. API, SQL-worker, and replacement Temporal-worker
  logs captured since the canary started contained zero warnings/errors.
- This verifies the deployed single-activity reprocess slice, stopped-worker
  acceptance, restart completion, stable retry identity, and eventual outputs.
  It does not prove cancellation, a crash during an executing Activity,
  commit-before-lost-reply recovery, every legacy producer, checksumless-import
  repair, or the actual admin UI interaction. Those remain explicit TODOs.

## Approved first native reprocess release

- On 2026-10-16 the user authorized staying below 80% changed-line coverage for
  this feature to prioritize a working production deployment and real-data vetting.
  CI 466 passed tests/lint/PostgreSQL checks but measured API 58.41%, worker 53.04%,
  and core 62.29%; images and E2E did not run after that gate failed.
- Temporarily measure CI coverage with a zero percentage minimum. Required
  reports, collection/retention, tests, lint, image builds, and release checks remain
  blocking. The ordinary `coverage:check` task retains its 80% minimum.
- Restore the CI threshold and add focused mock/fixture coverage after real-data
  vetting. Do not reintroduce SQLite tests, schema substitutes, timestamp massaging,
  or persistence compatibility logic. Existing PostgreSQL/Docker suites own database
  semantics. Keep new tests deferred until the working flow is verified.
- Release scope is the reviewed single-activity admin reprocess path and its
  eventual output handoffs in the combined worker. Other producer conversions,
  cutover, recovery drills, and PR #67's atomic fitness follow-up remain separate.

## Storage incident during validation, 23:07 UTC

- On 2026-10-16 the node's 98 GiB root filesystem filled during CI 481.
  Kubernetes evicted application, PostgreSQL, Temporal, ingress, and CI Pods;
  Bike and CI were temporarily unavailable. This was not an application or
  workflow acceptance pass.
- Emergency recovery paused CI replicas while its API was unavailable, then
  restored the normal replicas and released the recovery operation's maintenance
  Lease. This emergency action does not replace the normal scheduler-pause
  protocol for cache maintenance. Both temporary recovery Pods were removed.
- Old unreferenced Rust executables reclaimed about 9 GiB on the separate
  CI-cache filesystem. The root filesystem later returned to approximately
  34 GiB used and 60 GiB available. These are different filesystems: the root
  exhaustion cause remains unconfirmed and is an explicit operational follow-up.
- PostgreSQL, all five Bike deployments, Temporal, ingress, and CI recovered.
  Retained application volumes and all ten existing resource workflows survived.
  Activity 1588 still had 47 efforts, one linked activity, and the same derived
  content hash. Activity and fitness API reads returned 200 after recovery.
- CI 482 passed backend checks and the restored 80% gate with all 160 changed
  core lines covered, then was canceled during image compilation. CI 483 was
  canceled while scheduling was paused. The operator reduced CI resource limits
  for concurrent Plex use and explicitly authorized resuming under those limits.
  CI 484 reused the identical backend results, verified release images, and
  passed all 83 browser cases in 6.3 minutes. Resource limits were preserved.

## Concurrent projection fix: production canary, 22:58 UTC

- PR #67 merged as `97857523e69693f2e2880043448460ecc1465f70` after
  CI 478 passed all 83 browser cases. Main CI 480 passed another 83 cases and
  deployed the fix. All five application Pods were ready with zero restarts;
  `schema-migration-97857523e69693f2e2880043448460ecc1465f70` completed.
- Verified immutable images: API
  `8d4c28744b40ae647e84e69f2949069a5ff3f420b85649e72f55bc816a258871`,
  both workers `2a4f7663adb170c370b5257dd842acb48dadf21cfb0664c2f4ca184c8799f9d5`,
  UI `ebcc21b88723b4aac4107a7ed208e7dbb899b93d1ce83edb38227efd2e96f620`,
  and maps `9457a623de24c8b86093af52a2c9ca315e5345580adf12ceccf30abadaee8ede`.
  API, workers, and maps reported the merged revision. UI identity uses its
  immutable image rather than a missing `BIKE_VERSION` environment variable.
- Reprocessed the same real retained activity/import 1588 using command
  `4a787518-dc7b-43e3-8c5a-d66f273ee090`. HTTP 202 took **0.495768 seconds**;
  run `01a146f0-7816-7a76-b5ae-b1e9f676ed2e` completed in **1.510499988 seconds**.
- All eight segment Activities started between 22:58:50.266177 and
  22:58:50.421450 UTC, before the first completed at 22:58:50.497501 UTC.
  All eight therefore overlapped. Every Activity completed on attempt **1**,
  with no failed/timed-out events; the last completed at 22:58:51.846219 UTC.
  This repeats the previously failing real concurrency case with the row locks.
- Fitness rebuilt at 22:58:50.225509 UTC and cleared the dirty day. The activity
  retained 47 efforts, its IDs/title/sport, and the same derived-content hash.
  The unchanged heatmap reused ready generation 7; a new generation was unnecessary.
  The fitness API returned 200 with the same load/fitness/fatigue/form values.
- Retrying the identical command took **0.246614 seconds** and returned the same
  job/run. Exactly one command audit and one linked activity remained. SQL task
  totals stayed 1,991 completed and five pre-existing failures.
- Diagnostics exposed three row-lock waits of 288, 330, and 443 ms. The duplicate-key
  defect is corrected, but this is not a warning-free performance pass. Projection
  insertion still made one request per activity while retaining shared locks.
  The follow-up uses model-owned 500-row bulk inserts and tests 501 rows as two
  writes, retaining ordered locks, fresh reads, and atomic rollback.
- CI 480 also emitted a Pulumi update notice after applying successfully. Update
  the owning infrastructure CLI pin before the final release and inspect the
  actual deployment diagnostics. Do not suppress notices or raise SQL thresholds.
- Restore the ordinary CI 80% changed-line gate after this real-data vetting.
  The follow-up restores 37 removed-test scenarios with PostgreSQL mocks/shared
  fixtures and adds two projection batching regressions; no SQLite behavior returns.

## First native production reprocess, 21:58 UTC

- PR #66 merged as `7fa4f45181e3235a5b62f309e2b09573ac4beba3` after
  [CI 469](https://ci.nibelheim.dev/repos/3/pipeline/469) passed, including 83
  browser checks. [Main CI 471](https://ci.nibelheim.dev/repos/3/pipeline/471)
  passed its backend/build checks, another 83 browser checks, and deployment.
- All five live deployments were ready with zero container restarts. API, SQL
  worker, combined Temporal worker, and maps reported the new revision.
  Migration Job `schema-migration-7fa4f45181e3235a5b62f309e2b09573ac4beba3`
  completed both `native_import_correlation` and `activity_event_history`
  migrations. Public health returned healthy. The completed migration Pod is
  expected to be unready; the five application Pods were ready.
- Configured immutable release images under `localhost:32500/`:
  - API: `bike-api@sha256:111073abb2315f12d46fdefe6800a2c2d0facab0b1e2a6cc43f900f6649ecc9b`.
  - Both workers: `bike-worker@sha256:cbdf2e683503fa7557767c5b42f82482609c3124a37a7d9f74ada3d2c39b5bab`.
  - UI: `bike-ui@sha256:3868de5b4e21d53fdab45e235406ce2d37472fd89517e31c017d81dc185230c0`.
  - Maps: `bike-maps@sha256:f1f605b3caab0da088483eda0b0511d5cedd352acae1542cc63918d6cd37f944`.
    Its running platform digest remained
    `sha256:1d4f73e3c70df60a8dde120d1f4d62ef05de12614b244325f92f35dc8b924b6a`;
    record that separately from the configured image manifest.
- Both workflow and Activity pollers were present on `bike-tasks`.
  The existing SDK client identity is still named `bike-strava-temporal`;
  the combined binary polls both queues.
- Used an existing real FIT: user 1, activity/import 1588, artifact 1190,
  352,993 bytes. Verified retained bytes against SHA-256
  `f591a1a4845efdd5d7943eb1e7d51456506741ef415f628ddcee4746e9d8b42f`
  and filled only that artifact's previously empty checksum metadata. No source
  bytes or storage keys changed. Broader checksumless-import repair remains open.
- Command `ec95cef9-f4cb-4ab2-b4c3-e0c1561c00cb` returned HTTP 202 in
  **0.300533 seconds**. Its
  [Temporal job](https://temporal.nibelheim.dev/namespaces/bike/workflows/bike-task-reprocess_activity_import-user-1-ec95cef9-f4cb-4ab2-b4c3-e0c1561c00cb)
  completed in **2.719543358 seconds**, run
  `01a146b8-dd67-7e1b-ae4e-c889467db6af`, with 29 history events and
  6,860 history bytes. The recording stayed outside workflow history.
- Activity/import IDs and the title were preserved. Sport changed from legacy
  `ride` to `mountain_bike`. The source-derived hash changed from
  `171b476db612a8c46f56b2aa289c481e` to
  `1eabe440455f27f29e77448eb0a9b716`.
- Fitness rebuilt at 21:58:07.130465 UTC and cleared the dirty day. Heatmap
  generation 7 published ready at 21:58:07.785307 UTC. All eight segment summaries
  published by 21:58:17.389013 UTC. Activity and fitness API reads returned 200.
  The activity retained 47 efforts and exactly one linked activity.
- Repeating the identical command returned HTTP 202 in **0.199813 seconds**,
  with the same workflow and run IDs. Exactly one reprocess audit event remained;
  no additional SQL task was queued. SQL task totals stayed 1,991 completed and
  five pre-existing failed records.
- **Concerning result:** segment Activities 8/17/21/22/28 needed 2/3/3/2/4
  attempts respectively. Their transactions overlap on activity analytics even
  when each segment workflow is independently serialized. PostgreSQL rejected
  duplicate activity-analytics inserts; Temporal retries recovered all eight
  publications. Root completion and eventual output recovery are proven, but
  this is not an error-free concurrency pass.
- PR #67 now addresses shared projection replacement with ordinary model-owned
  transactions, ordered activity row locks, fresh reads after locking, and
  segment commit before projection locking. Keep the fix open for review and
  repeat this real concurrent case after integration. Also retire bound recording
  payloads from SQL error spans; the observed diagnostics were unnecessarily large.
- Formatting was removed from the worker baseline and moved to independent PR
  #68. Custom DAG views/APIs and graph reconstruction are removed in PR #69.
  Both remain separate reviews. This canary does not prove cancellation, worker
  interruption, lost-completion recovery, or producer-wide conversion.

## Review follow-up R01: atomic fitness invalidation

- The consolidated [archived checklist](#superseded-worker-conversion-checklist) preserves all
  eleven review items in execution order. R01 is ready for review; rollout and
  PostgreSQL isolation proof remain open.
- `analytics_user_states::Model::mark_dirty` now merges the earliest dirty day
  and greatest source timestamp in one upsert against the existing user row.
  Every invalidation touches its existing write fence; rebuild completion retains
  both source-time and write-time conditions. No new table, column, or ledger.
- Four PostgreSQL SeaORM mock contracts exercise first/duplicate/out-of-order
  invalidations, preservation of rebuild metadata, old source timestamps advancing
  the write fence, stale completion rejection, and database failures. Three failed
  against the original read/modify/write implementation; all four pass afterward.
  Mock SQL assertions verify the generated contract, not database interleavings.
- Focused Docker commands through owning mise configuration passed 83 cases:
  `cargo test -p bike-core --locked --lib fitness` (15),
  `cargo test -p bike-core --locked --lib activity_reprocessing` (14), and
  `cargo test -p bike-core --locked --lib strava_native_unit` (54).
- The combined run exposed a trace-fixture failure that did not occur in
  isolation. Refresh tracing callsite interest after installing the scoped
  subscriber; retain SQL-span assertions. Three focused runs with 16 test threads,
  all 54 Strava domain cases, and six database-span cases pass after this change.
- SQLite-backed suites, schema/timestamp helpers, unused SQL fixtures, and SQLite
  dev-dependency features have been removed from the baseline as requested.
  Pure tests, PostgreSQL mocks, shared synthetic browser uploads, and the existing
  PostgreSQL/Docker suites remain. Historical test counts elsewhere predate this
  removal. No new E2E suite or database substitute has been introduced.
- CI 466 on baseline `7a920cbb` passed tests/lint but failed changed-line coverage:
  API 58.41%, worker 53.04%, core 62.29% against 80% per project. Images and E2E
  were skipped. The approved first-release exception above supersedes this block;
  R01 remains a separate review in PR #67.
- These checks do not prove real concurrent upserts, commit-before-lost-reply
  recovery, or rider-visible convergence. P01–P04 and user vetting remain gates
  before the broader worker conversion.

## Recovery baseline captured before PR #61

- Observed at 2026-10-16 12:20 UTC, before integration or production task runs.
- Source revision: `c68c3d3914290233d3f4148ebadee9801c7517cf`.
- All five deployments were ready (one replica each), with zero container
  restarts. Public `/api/health` returned `{"status":"healthy"}`.
- Record configured release digests separately from running platform image IDs.
  UI/maps use multi-platform images; their platform digests differ legitimately.

| Deployment        | Configured image (`localhost:32500/`)                                                 | Running image digest                                                      |
| ----------------- | ------------------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| `api`             | `bike-api@sha256:4101a3e25e6d191328849dba1d5e44120d8624305331628b829ddce20689e058`    | `sha256:4101a3e25e6d191328849dba1d5e44120d8624305331628b829ddce20689e058` |
| `worker`          | `bike-worker@sha256:71f03457b6a6cfeac93c5ad8d0c808f7d703e44561166ca645b74ad8dd5c27a2` | `sha256:71f03457b6a6cfeac93c5ad8d0c808f7d703e44561166ca645b74ad8dd5c27a2` |
| `temporal-worker` | Same worker image                                                                     | Same worker digest                                                        |
| `ui`              | `bike-ui@sha256:aefd7bba0dc9266af2230139220191f1a706b7b99480926d8a2a56a5e1ec5b36`     | `sha256:003a01a9af07171394e4fe8668e20f9f0749776f0f415f2ac41c6fef4b49ab50` |
| `bike-maps`       | `bike-maps@sha256:8d8f1a2bb35e96399d6e94d717f83be7423ddf6115db40cbcb874ab307b764e0`   | `sha256:1d4f73e3c70df60a8dde120d1f4d62ef05de12614b244325f92f35dc8b924b6a` |

- Migration Job
  `schema-migration-c68c3d3914290233d3f4148ebadee9801c7517cf` was Complete;
  logs said `No pending migrations`. Latest applied migration:
  `m20261015_000001_background_task_insert_defaults`.
- Upload storage: PVC `bike-uploads`, volume
  `pvc-2db207f9-8f52-4401-bf9e-f9f4985dd8d3`, mounted at `/data/uploads`
  in the API and both workers. Keep its bytes and references intact.
- Map cache: PVC `map-image-cache`, volume
  `pvc-c670b5e2-6b5e-4a1e-a446-1886a742154b`. Keep the claim intact.
- Latest combined database/files backup Job: `pg-backup-29868840`, succeeded
  at 06:01:18 UTC. Independent database backup: `pg-database-backup-29868870`,
  succeeded at 06:30:47 UTC. Job success is not restore verification.
- Native Strava polls namespace `bike`, queue `bike-strava`, through
  `/app/temporal-worker`. Shared server is Temporal 1.32.1, ready.
- SQL worker owns the existing Bike processors. There were no pending or
  processing SQL tasks. Four retained `prepare_heatmap` failures predate this
  rollout: one obsolete payload decoder error and three recording-policy
  constraint errors. Current projections are all version 5: 737 ready and
  455 skipped, with none pending or failed.

## Integration and recovery procedure

- PR #61 head `01ac73a77f2211514313a4305a330d0b062723ac` passed both
  Woodpecker PR checks (pipeline 449). Focused Docker unit checks passed 197
  tests; these use fake domain adapters, not production database effects.
- Integrate the checked groundwork, then let the existing main CI pipeline
  build, test and promote its immutable images. Verify the new migration Job,
  every deployment's image and readiness, and public health before task runs.
- PR #61 merged as `f0a335b27cabcdce19e4f42933717b682ba8447f`. Main pipeline
  450 passed its test stages but stopped at changed-line coverage: report reuse
  omitted `bike-temporal` from a hard-coded cache archive. No images were built
  or deployed. A fresh-checkout cache regression reproduced the missing report;
  the forward fix derives the retained Rust projects from `coverage-projects.json`.
- The twelve named Bike workflows are definitions only at this boundary.
  `bike-tasks` has no deployed worker or concrete database adapters. Do not send
  production commands there or describe SQL processor runs as native proof.
- Prefer a small forward fix, with a reproducing unit test, for a diagnosed
  problem. Use a PR and the owning checks/deployment pipeline.
- If rollback becomes necessary, refresh the production stack configuration,
  then use its owning deployment tasks to set `appTag`, `uiTag`, `mapImageTag`
  and `imageDigestRevision` to the baseline source revision and the four
  `apiImageDigest`, `workerImageDigest`, `uiImageDigest`, `mapImageDigest` pins
  to the configured digests above. Keep both worker images on the same release.
- Check that the registry still serves those digests before applying. If a
  digest is gone, rebuild and validate the baseline source through CI instead
  of assuming a tag still identifies the old bytes.
- PR #61 introduces no database migration or destructive TTL sweeper. Retained
  Strava object keys and reference JSON stay compatible. Do not reverse schema
  history, remove upload bytes, reset workflows, or restore a database merely
  to roll back application images.
- After any recovery, verify migration completion, deployment readiness, exact
  images, health, queue ownership, and one bounded domain operation. Preserve
  existing Strava workflow histories and retained objects throughout.

## Production exercise ledger

- Baseline processor canaries use one temporary disabled account, two synthetic
  activities, three retained imports, and one synthetic segment. Existing riders
  are outside this test scope. Inputs are the tiny invented FIT/TCX fixtures;
  the manual TCX moves its invented date forward by one day.
- These are SQL processor checks on the baseline image, not native Temporal,
  API handoff/trace, large archive, or HTTP download proof. The archive test
  resumes an injected retained two-entry ZIP through the actual worker.
- Archive task `2266` and child imports `2267`/`2268` finished: one imported,
  one duplicate, zero failures; its batch closed and released its lock. Manual
  import `2271`, reprocessing `2272`, fitness `2279`, segment efforts `2277`,
  segment analytics `2278`, training `2280`, heatmap `2281`, user reprocessing
  `2282`, and archive-FIT reprocessing `2287` completed on their first attempts.
- Observed domain output: two activities with 16/3 route points, two training
  analyses, completed training-backfill state, 2,481 fitness-day rows accounting
  for both activities, ready version-5 projections with eight heatmap chunks,
  and a matched effort reflected in its segment summary.
- Individual `regenerate_activity_segments` task `2273` failed with
  `Segment batch context is missing`. Its implementation unconditionally used
  batch aggregation. A focused regression reproduced that error; another caught
  missing per-activity analytics. The forward fix handles standalone cache
  publication and refreshes activity analytics in both standalone/batch paths,
  including removal of the last match. All three focused regressions pass.
- User regeneration `2290`, both matching children, and segment cache `2293`
  completed; its final cache barrier closed and released the import lock.
- Forward-image verification and retry of the standalone failure remain pending.
  Record final image/task outcomes in the rollout PR description after deployment
  so continuing rollout evidence does not create additional commits.
- Native task conversion still requires concrete domain effects, retry-stable
  outcomes and write fences, producer routing, worker registration/deployment,
  shared request/trace/actor context across API and workflow handoffs, and live
  debounce/restart checks. Follow the task order in
  [the conversion backlog](TODO.md#worker-conversion).

## Native Bike migration unit evidence, 2026-10-16

- Historical review evidence below is distinct from the production SQL canaries
  above. Current behavior belongs to [the background-work spec](specs/background-work.md)
  and outstanding work to [the backlog](TODO.md#worker-conversion).
- PR #61 groundwork passed 169 focused Docker tests, then 197 after completing
  the twelve named workflow definitions: 58 Bike workflow/Activity/client checks
  and 139 storage/TTL/Strava regressions. The synthetic 273-entry archive tested
  continuation, not real streaming/disk/crash durability. The original cancellation
  fixture did not prove the later apply/handoff cutoff; F4/U12–U14 retained that gap.
- Storage follow-up passed 18 shared storage/TTL/Strava compatibility units.
  Retention implementation supplied serializable lifetimes/candidate decisions,
  without enabling automatic deletion. Required Docker hooks and Clippy passed.
- The first concrete reprocess slice passed 92 Rust units (11 core, 71 Bike
  workflows/Activities/submission, 3 API, 7 worker lifecycle/metrics) and two admin
  UI units. It wired direct single-activity submission, concrete retained-source
  replacement, minimal audit refresh scope, and derived handoffs in one combined
  `temporal-worker`. This was local review evidence, not production activation.
- Request-boundary refinement passed six focused API units covering submission,
  command UUID preservation/defaulting, acceptance mapping, and replay preconditions.
  OpenAPI regeneration left the public wire contract unchanged.
- Shared-storage refinement passed 33 focused units: 14 reprocess, 16 shared
  storage/retention, two Strava object compatibility, one transport. Sources used
  the shared in-memory backend/SeaORM mocks; corrupt primary/supplemental bytes
  failed before mutation and supplemental evidence preserved primary geometry.
  The existing OIDC regression also passed after a formatter-related helper split.
- The shared worker compiled with warnings denied; required formatting/Clippy
  hooks passed with Rust's 120-column limit and default heuristics. PR #66 retained
  one commit targeting `main`; #64 and #65 closed as superseded. No merge/deployment was
  authorized. The default-stack browser assertion expects unavailable Temporal;
  its edit is not live server evidence.
- Scenario mapping for this review: U02–U04/U23 API/submission; U05–U09 domain
  reprocessing; U12–U14 cancellation/replay; U16/U17/U26 debounce/priority/late
  notifications. Cross-producer variants, P01–P04, user vetting, and cutover remain
  open in the backlog rather than being implied by these unit counts.
- Documentation/task consolidation then passed **283 native units** through the
  composed `test:tasks:unit` task, **64 local documentation links**, scenario/task
  preservation comparisons, and Markdown formatting. Required coverage-policy,
  Rust compiler-cache, CI reuse, Rust formatting, and Clippy hooks passed.
  This is local validation of the cleanup, not a new production exercise.

## Strava observations, 2026-10-13 and 2026-10-14

These observations were moved from the former Strava spec. Revisions, resources,
coverage, and gaps describe those inspection dates, not a fresh production check.

- 2026-10-14: observed a real Strava create for activity `20582557310` on
  `POST /webhooks/strava-gateway` at `14:26:41 UTC`. Subscription `374570`;
  event `89f2d159-4723-47d2-a3a4-ef2f525f24f7`; trace
  `83126ef1c10370831a84c3d0cc816701`. Create run
  `01a13ace-eea9-77db-9e34-607e551eaec5` and Upsert run
  `01a13ace-eef6-7016-9076-665f1cfc28a7` completed. The deployed version excluded
  its manual Walk summary (`already_absent`) before the summary fix was deployed.
  A real delete webhook arrived at `14:40:21 UTC`, event
  `9b4365ff-d180-4fdb-9f2c-b51b351cc5ad`; the deleted summary must remain absent.
- 2026-10-14: a second real create for `20582644055` reached the same POST route
  at `14:34:05 UTC`, event `507437e9-5cbd-4623-996e-1710b3129bdd`, trace
  `43669f123df376c8c0965be1a2379fe5`. Its retained activity response contained a
  manual MountainBikeRide, 10,718.2 m distance, 407.5 m elevation gain, description,
  and gear ID/name. The deployed workflow failed on streams HTTP 404. The fix
  skips that request for manual activities; successful recovery is recorded below.
- 2026-10-14: [CI 403](https://ci.nibelheim.dev/repos/3/pipeline/403) deployed
  revision `535cdf3e37d00fadd5609b41b7dfaf28e07f9b1e` after unit, integration,
  lint, blocking changed-line coverage, and all 83 browser checks passed.
  All five workloads were ready, the matching migration Job was `Complete`
  with no pending migrations, and public `/api/health` returned healthy.
  The live native worker image was
  `bike-worker@sha256:58bd6161cdfb61eaaf6141b95377ca9cf8307d012b223f1cdad7beae4da36e80`.
  The release log emitted a Pulumi warning for the migration container's
  successful exit 0; retain this diagnostic as a release-tool gap, not a clean pass.
- Reset the failed Create for `20582644055` at its first workflow task after
  the updated worker was ready. New Create run
  `af733b44-882a-4cda-ac9f-e134ea6f97f5` and its recreated Upsert run
  `01a13ba7-94fd-79a7-bbdb-226a9a748ce0` completed at `18:23:21 UTC`.
  Event `507437e9-5cbd-4623-996e-1710b3129bdd` is `applied`; Bike activity
  `1679` stores MountainBikeRide, 10,718.2 m distance, 407.5 m elevation gain,
  the supplied description, and gear ID/name. No independent child reset was needed.
- A subsequent real delete for `20582644055` arrived at `19:31:24 UTC`, event
  `8399473f-84e7-4a30-a803-817113c08dd0`; its terminal mutation is `deleted`.
  Preserve that newer deletion rather than retrying its earlier create again.
- Triggered the existing native schedule once after recovery. Scan
  `bike-strava-scan-2026-10-14T18:23:48Z`, run
  `01a13ba8-007b-7b35-b26c-6985e16536a4`, and its customer Sync completed
  with two unchanged activities and no duplicate ride. The deleted Walk
  remained absent; a live retained non-cycling summary still needs observation.
- Live production samples over an inspected six-hour range showed native worker
  memory peaking at 15.6 MiB of its 512 MiB limit and shared Temporal at 130.4 MiB
  of 1 GiB. Maximum five-minute mean CPU was 2.7m and 38.3m respectively.
  Shared PostgreSQL used 10,636,435 bytes for `temporal` and 11,685,011 bytes for
  `temporal_visibility` at the storage observation. These are observed small-load
  values; large-backfill capacity and shared database resource load remain unqualified.
- 2026-10-14: verified a real native-worker stop/restart using retained ride
  `20424619693`. With no other Strava workflows running, stopped the worker,
  queued a reset of Upsert `strava-04182920-01d9-5566-a554-a69e09f43364` at
  workflow-task event 28, and restored one worker replica. This retained the four
  completed fetch/streams/normalize/persist activities and their object references.
  Replacement pod `temporal-worker-b54fb764c-jmkrv` replayed those completions;
  new run `4d1bf832-c9e5-453d-875c-5d8a615ff90f` completed from
  `20:15:48.259 UTC` to `20:15:49.915 UTC` (1.66 seconds). Historical provider
  completions kept their original timestamps; only the remaining publication
  activities ran again. Bike activity `1676` stayed unique with identical sport,
  distance, elevation, and source metadata. New worker diagnostics were clean.
  This controlled stop/reset/restart does not establish recovery from killing
  a process during an executing provider or persistence activity.
- The real replayed fetch/streams/normalize activity results were 953, 1,193,
  and 1,212 serialized JSON bytes. They carried object references; the corresponding
  stored normalized source was 97,651 bytes. Oversized-source proof remains the
  separate 3 MiB provider-body and 150,000-point stream fixtures, not this small
  live activity. Live namespace retention was confirmed as `604800s` (seven days).
- The new native SDK replay tool successfully replayed the real Walk Create
  and Upsert histories, plus completed Sync `strava-ff834edc-cfbd-5f7a-bf2a-e910f1c26926`,
  using fresh workers and current workflow code. It fetched history
  read-only and executed no provider or database activities. This proves history
  compatibility separately from a live process-interruption drill.
- Read-only live SDK replay also passed the failed manual ride Create/Upsert
  histories before reset. The example's file mode rejects Temporal CLI camelCase
  JSON exports with `missing field event_id`; use its verified `--workflow`
  history-fetch mode until export-format compatibility is addressed.
- 2026-10-14: 127 focused Rust units and five UI units passed in Docker. These
  include large stored provider bodies/streams, restart reconstruction, 5,100
  activity scans, 60 customer scans with ten failures, summary metadata, and
  non-cycling create/update/delete boundaries. UI lint, types, and formatting
  passed. Old histories retain pre-patch continuation/start-failure/input-budget
  decisions; a restarted patched scan retains its failed-start count. Full CI
  and post-rollout verification are separate gates.

- 2026-10-14: the release has an explicitly approved one-release exception to
  the 80% changed-line coverage gate. Tests, lint, coverage report retention,
  image builds, and existing release checks remained blocking. The follow-up
  restores blocking coverage after rollout; isolated adapter/startup coverage
  remains on the TODO.
- 2026-10-13: read-only Strava API confirmed subscription `374570` uses
  `https://bike.nibelheim.dev/webhooks/strava-gateway`. Live GET challenge passed;
  OAuth rejection returned HTTP 303. Ingress routes both aliases to `api:3000`.
- The current unit suite executes all eight workflows with mocked dependencies
  and fixture responses. It requires no running services. The earlier database
  workflow test and its CI setup have been removed from this PR.
- The suite also covers connection validation, mutation status/counts, deletion
  idempotency and rollback with SeaORM mocks, shared recovery configuration, and
  stable queue payloads. No PostgreSQL or SQLite database is opened by these tests.
- 2026-10-13: 92 isolated units passed: 38 domain/persistence, 14 OAuth, nine
  webhook, 28 Temporal workflow/handler/logging, and three configuration/queue tests.
- 2026-10-14: revision `1a7b33e6` deployed with all five Bike workloads ready
  and all three new migrations applied. Both native worker exporters are
  scraped successfully. All six correlation search attributes are registered.
  Public GET verification passed on both webhook routes; both OAuth aliases
  rejected invalid state with HTTP 303. The existing release suite passed
  83 browser checks. A real scheduled sync fetched activity `20424619693` and
  its streams with HTTP 200, upserted Bike activity `1676`, and completed all
  three parent/child workflows with zero failures. Root correlation ID:
  `01a13a17-43ff-7af6-999c-e5f409989648`.
- The live job exposed a trace filter gap: new native crate targets were omitted.
  Include them as application trace roots. An isolated in-memory tracer unit
  verifies that a filtered scan produces a carrier and its child retains the
  same trace ID. Revision `bd300cf8` passed the restored coverage gate and
  deployed successfully. A repeated real sync returned one unchanged activity
  and zero failures. Its customer and activity events retained the same carrier;
  list, activity, and streams exchanges shared trace
  `8d61988be672f15ffda11303fee1fe77`.
- Both backup disks validated generation `20261014T110418Z` after pausing and
  resuming API, SQL worker, and native Temporal worker.
- Unit guarantees remain bounded; deployment readiness and callback checks do
  not establish production behavior for every operation. Natural webhook POST,
  create, and delete observations are recorded above; live backfill/disconnect
  and interruption during an executing Activity still need evidence.
- [TODO](TODO.md) tracks production worker/scrapes/UI, cutover, coordinated

### Alert delivery and coverage observations

- On 2026-10-14 the real manual-ride Create/Upsert failures triggered
  `BikeStravaWorkflowFailed` at 16:13:18 UTC. Alertmanager routed both to ntfy;
  notification `nELxsbEcvAda` arrived at 16:13:48 UTC. No synthetic workflow or
  injected alert was used. Handoff alerts still awaited exporter rollout.
- CI 394 on `1496503a` passed the 80% changed-line gate: API 89.74%, worker 94.44%,
  Temporal 96.58%, core 100%. All 72 focused isolated units, formatting, and Clippy
  passed. This does not establish whole-integration coverage or bootstrap/OS
  signal/runtime behavior. Remaining coverage and deployment work stays in TODO.

## Superseded worker conversion checklist

- Archived on 2026-10-17 after the user rejected the migration's accounting and
  coordination complexity. This section preserves prior findings and reported
  evidence; unchecked items, acceptance IDs, and rollout instructions below are
  historical, not current requirements or authorization.
- The [current TODO](TODO.md#worker-conversion) and
  [background-work specification](specs/background-work.md) own the replacement
  file → processing → child-task design.

<details>
<summary>Previous checklist and implementation evidence</summary>

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

### Execution UI simplification

- [x] Retire custom activity/import/task/integration DAG screens, graph APIs,
      reconstruction models, and stage replay controls; use Temporal UI instead.
- [x] Preserve simple source metadata, domain audit/event history, and the legacy
      task list until its producers are converted. Replace the fixed import DAG
      runner with an ordered stage list without changing its SQL retry behavior.
- [x] Share Temporal links using the recorded namespace/workflow ID and runtime
      `TEMPORAL_UI_URL`. Do not invent workflow identity from activity IDs.
- [ ] Configure the public Temporal origin in deployment before enabling the links.
- [ ] Verify retained metadata/errors/downloads and access controls with existing
      mocks/fixtures and CI. Remove tests exclusive to the retired DAG controls.

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
  tests and required Rust hooks pass locally. PR #74 merged as `1eef1aa9` after
  CI 503 passed checks and all 83 browser cases. Production sampling caps and
  CI resource limits are unchanged.
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
  The owning CI/manual deployment tasks now run this native rollout/public-health
  gate; all four shell fixture cases pass. It passed against live revision
  `8fb5d9ff`, with the matching migration complete and all tested images ready.
- **Runtime qualification, P1:** the 02:57 UTC lifecycle canary published all
  outputs on attempt one, but later logs contain 250 ms slow-statement warnings
  from metrics COUNTs, feature flags, and the handoff poll (0.27–1.2 seconds).
  The eight-core node's load was 35.06 without memory/disk/PID pressure. Diagnose
  the queries and contention before marking the runtime clean; preserve warning
  thresholds and the operator's CI settings. Domain completion is already verified.
  Deployment also lacked the shared Go cache setting: Pulumi dependencies were
  repeatedly downloaded/compiled in the temporary checkout. Wire the deploy
  workflow to the existing Go cache, verify native configuration checks and a
  later warm deployment, and retain the broader contention/disk investigation.
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
| 9     | R11    | P2       | Permanent retained-input failures                       | In progress     |
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
  PR #75 deployed as `8fb5d9ff`. All ten original runs gracefully completed, then
  a real retained-source reprocess started fresh runs on the same resource IDs.
  All ten published once and completed on attempt one; no Bike tasks remain
  Running. Stored fitness, eight segment summaries, activity identity/efforts,
  and the ready heatmap were verified. The remaining simplification below stays open.
  Return actual publication outcomes from domain operations instead of inferring
  them with surrounding reads. Remove unused `published` state and unsupported
  blocked/superseded behavior; retain resource, merged pending work, priority,
  timers, and compatible continuation input. Mock concrete domain adapters,
  late lower revisions, active work plus one follow-up, and retained history.
- **R11 — permanent retained-input failure** (U06).
  Distinguish immutable checksum/size/parse failures from transient backend
  failures with typed errors. Mock corrupt primary/supplemental bytes as terminal
  and temporary storage faults as retryable; do not repeatedly parse corrupt input.
  Implementation uses the existing typed domain status and native error adapter:
  integrity/retained JSON errors are terminal 422s, parser/missing-input client
  errors remain terminal, and temporary backend failures remain retryable.
  All 35 focused mock/fixture checks pass, including malformed primary/supplemental
  sources with valid integrity, corrupt bytes/size, malformed retained JSON, and
  native terminal/retry adaptation. The checksum regression fails with the old
  server-error classification. Required hooks, CI, and live rollout remain open.
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

</details>
