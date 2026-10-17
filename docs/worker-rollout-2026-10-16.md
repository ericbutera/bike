# Bike worker fail-forward rollout, 2026-10-16

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

- The consolidated [TODO](TODO.md#prioritized-review-follow-ups) preserves all
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
