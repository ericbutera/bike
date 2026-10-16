# Bike worker fail-forward rollout, 2026-10-16

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
  [the conversion plan](worker-conversion-plan.md).
