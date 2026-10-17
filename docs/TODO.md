# Bike TODO

## Worker conversion

- **2026-10-17:** process the retained file, save the activity, then call segments,
  achievements, heatmap, training, and fitness tasks. Temporal owns execution.
- [Background work](specs/background-work.md) defines the design. The superseded
  accounting/coordination checklist remains [historical](worker-rollout-2026-10-16.md#superseded-worker-conversion-checklist).

### Converted tasks

- [x] `email_notification`: retire unused Bike SMTP work; keep OAuth/SSO.
- [x] `process_activity_import`: parse/save one retained file, then derived tasks.
- [x] `reprocess_activity_import`: use the same sequence, preserving activity identity.
- [x] `rebuild_fitness_freshness`: recompute from stored activities.
- [x] `rebuild_segment_analytics`: rebuild results from current efforts.
- [x] `prepare_heatmap`: rebuild using current source/privacy state.
- [x] `regenerate_activity_segments`: rematch, rebuild affected results, then achievements.
- [x] `regenerate_segment_efforts`: run the existing segment matching operation.
- [x] `backfill_user_xc_training`: page activities and calculate training.
- [x] `reprocess_user_activity_imports`: page owned imports and await reprocessing children.
- [x] `reprocess_archive_fit_activity_imports`: use the same loop with archive FIT selection.
- [x] `regenerate_user_segments`: page cycling activities and await regeneration children.
- [x] `activity_archive_import`: retain ZIP bytes, extract a page, await imports, continue by offset.
- [x] Reconcile pending heatmaps through a 15-second native schedule.
- [x] Move upload, admin, bulk, archive, training, and edit/delete producers off SQL.
- [x] Remove the SQL worker executable, queue, batch/stage/recovery/ledger code,
      SQL task APIs/UI, unused SMTP, retention-inventory prototypes, and obsolete checks.
- [x] Preserve historical migrations; append schema retirement separately.
- [x] Use ordinary transactions/upserts and source ownership/deletion/privacy checks.
      Repeated computation, partial results, and an in-flight Activity finishing are acceptable.
- [x] Keep the source-time/serialization proposals in #80/#81 out of this conversion.
      No custom cancellation or commit protocol is required.
- Local validation covers workflow order, page continuation, child failures,
  replay after extraction/save, retained input retries, and PostgreSQL mock writes.
  Production evidence is recorded in the [cutover record](worker-rollout-2026-10-16.md#2026-10-17-simple-worker-cutover).

### Production cutover

- [x] Pause intake/scheduled sync and drain the old SQL/Bike work. Retired Bike
      coordinator histories cannot be replayed/reset under the new sequence.
- [x] Verify a fresh database/file backup on both disks: `20261017T140351Z`.
- [x] Apply schema retirement and remove the SQL worker, its scrape/dashboard,
      unused SMTP configuration, and obsolete release checks.
- [x] Verify all four deployments, immutable image identities, and public health.
- [x] Process retained activity `1653`; preserve its title and identity, current
      heatmap, efforts, achievements, training, and rebuilt fitness.
- [x] Replace the worker during a native workflow and verify the same run completes
      after the replacement starts, without duplicate activities.
- [x] Process a 17-entry archive across Continue-As-New. The canary repeats one
      real retained file; it retains one duplicate import and creates no activity.
- [x] Complete native segment regeneration, fitness/segment analytics, training,
      heatmap reconciliation, and bounded tail pages of both import batch types
      and user-segment regeneration.
- [x] Keep polling/health responsive during CPU-heavy domain work using Tokio's
      blocking pool and two native Activity slots. Give Bike work a separate
      30-minute Activity timeout; preserve native retries and ordinary domain writes.

## Other backlog

- [ ] Correct provider event time versus local sync time with a focused fixture
      regression. Keep existing ownership, deletion, and connection checks;
      choose the smallest fix for the observed bug.
- [ ] Repair checksumless legacy import metadata without replacing retained files.
- [ ] Remove recording/provider bodies from SQL error diagnostics; retain useful
      operation names and IDs.
- [ ] Investigate the recorded root-disk growth and slow metrics/feature-flag/handoff
      queries. Preserve warning thresholds and the operator's CI resource limits.
- [x] Verify deployment readiness/public health, public Temporal links, and
      retained metadata/download authorization during the worker cutover.
- [ ] Verify warm deployment Go-cache reuse in CI. Local/manual deployment reused
      the compiled infrastructure program; the CI resource limits stay unchanged.
- [x] Verify native standalone segment regeneration with the forward image and
      retain its focused regressions. The former SQL failure targeted deleted
      activity `1681`; current real-data regeneration succeeds through Temporal.

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
- [x] Record the former 14-day/90-day scratch-retention proposal. The 2026-10-17 simplification supersedes its reference/pin inventory requirements; keep canonical sources under the current background-work specification.
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
