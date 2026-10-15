# Bike TODO

## Model boundaries and fixtures

- [x] Audit runtime query builders and repeated test-model construction; record
  scope, counts, and migration acceptance in [the spec](specs/model-boundaries.md).
- [x] Share import/activity/account/connection fixtures through test-only core
  support; retain scenario overrides and generated-ID insert helpers.
- [x] Require simple call sites, entity-owned queries, and fixture reuse in
  `AGENTS.md`.
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

- [x] Define the new Temporal design and correlation fields in [the spec](specs/strava-temporal.md).
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
- [x] Observe Strava-originated POSTs on the registered route. Real creates `20582557310` and `20582644055` reached `/webhooks/strava-gateway` on 2026-10-14; see the Strava spec for outcomes.
- [x] Retain all Strava summaries, descriptions, and gear in the feed; skip streams for manual rides and non-cycling summaries. Fixture checks and the live manual ride recovery passed on 2026-10-14.
- [x] Centralize sport/cycling policy and Training/Race in backend enums; generate frontend types and runtime options through OpenAPI. Preserve unknown provider sports without admitting them to cycling calculations.
- [x] Bound Strava Temporal JSON to 256 KiB, keeping large provider bodies and parsed data in object storage. Cover oversized callbacks/results, a 3 MiB provider body, and 150,000-point streams with isolated units.
- [x] Define event/source retention and protected reference roots in the Strava spec. Keep canonical activity sources; retain successful workflow scratch for at least 14 days and failed scratch/audit events for at least 90 days after the final execution closes.
- [ ] Implement reference-aware retention with dry-run reports, complete Temporal inventory, and deletion-race protection; retain inputs when any required inventory is unavailable.
- [x] Test restart reconstruction, populated large histories, and failures across scheduled-scan pages with isolated native workflow units. Cover child-start failures and continuation within a page.
- [x] Deploy the summary fix and rerun failed Create/Upsert for `20582644055`; Bike activity `1679` retains distance, elevation, description, and gear. A subsequent scheduled scan completed without duplicates.
- [ ] Observe a live non-cycling summary that remains present in Strava. Walk `20582557310` received a real delete webhook at `14:40:21 UTC`, so it correctly remains absent after the rollout; fixture coverage verifies cycling exclusions.
- [x] Verify a live native-worker stop/restart with recorded-history replay. Queued reset of retained ride `20424619693` resumed after four recorded completions and finished in 1.66 seconds; Bike activity `1676` remained unchanged and unique.
- [ ] Verify an interruption while a provider or persistence activity is actively executing. The controlled stop/reset/restart drill queued its partial history while the worker was stopped.
- [ ] Resolve the release-tool warning that reports a successfully completed migration container as a warning. CI 403 applied revision `535cdf3e`; Kubernetes confirmed Job `Complete`, exit 0, and no pending migrations. Do not describe this release log as warning-free.
- [x] Measure observed production resource use through Prometheus. Over the inspected six hours, native worker memory peaked at 15.6 MiB of 512 MiB and shared Temporal at 130.4 MiB of 1 GiB; maximum five-minute mean CPU was 2.7m and 38.3m respectively. This records the actual small workload, not a capacity benchmark.
- [ ] Qualify capacity for large backfills and sustained customer concurrency, including shared PostgreSQL resource use.
- [ ] Resolve Temporal's initial `Frontend is not healthy yet` startup warning; keep diagnostics enabled.
