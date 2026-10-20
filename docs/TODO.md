# Bike TODO

- [ ] Impersonate user (from admin user list screen)
  - [ ] admin see's UI exactly like end user
  - [ ] add "return to admin" link on navbar when impersonating

## Worker

- The 12 Bike tasks and earlier Strava processors are accounted for in the single
  [worker specification](specs/background-work.md#task-inventory-and-behavior-parity).
  Email and obsolete SQL dispatcher/polling/retention processors are explicitly
  retired there. There is no remaining SQL-to-Temporal task conversion checklist.
- Clients enqueue native workflows; Activities reuse domain logic and parents
  await downstream children. Temporal owns durable execution.
- Completed cutover and polling-removal evidence is consolidated in
  [the specification](specs/background-work.md#cutover-evidence).
- [ ] Verify the direct-submission release: new API before relay-free worker,
      both native queues available, real webhook/upload/manual edit start their
      expected workflows, rejected starts fail requests, and redelivery retries
      the original workflow ID without duplicate activities. Confirm an idle
      worker creates no reconciliation jobs or SQL dispatch scans. Schedule/history
      deletion alone did not verify this request path.
      Include the Activities and Users gear menus: confirm task-specific inputs,
      native job links, unchanged-command retry, and integration-event history.
- [ ] Deploy and verify the updated Worker, Overview, System Health, Strava, and
      Traces Grafana dashboards plus retired inbox-alert removal through their
      owning infrastructure resources. Check both worker endpoints, both queues,
      selected ranges, missing series, and real logs/traces.

## Activities

- [x] Show accessible sport icons before feed titles, with original mountain/fat
      bike artwork and baseline-aligned location/date metadata; see the
      [activity-list specification](specs/activity-list.md).
- [x] Show icons in the activity filter, preserving URL selection and keyboard
      access, and a location pin in the activity-detail header.
- [x] Match the feed's upload action to the shared compact Refresh icon button.
- [x] Simplify the activity-detail header, split summary/data cards, hide empty
      matches/signals, combine signal toggles with summaries, and use a compact
      lap table; see the [detail layout](specs/activity-details.md#detail-layout).
- [x] Clarify selected-climb numbers and activity-distance ranges, reduce metric
      text, and fit elevation profiles to their sample range and phone viewport.
- [x] Remove import details, workflow links, and regeneration from the activity
      detail actions menu, including for administrators.
- [x] Try shared Segments/Climbs tabs with counts, populated-data defaults,
      preserved detail state, keyboard access, and independent map layers.
- [x] Make the header sport and Training/Race tags editable, put Edit activity
      first, and support title/location/description plus recorded gear names and
      private notes. Preserve partial writes, manual history, and sport eligibility.
- [x] Compact Activity data with a visible elapsed-time/metadata grid and no
      provenance fields or Show more button; show a clock beside the header time.
      Place measurements and details in two columns, stacking on mobile.
      Tighten card gaps, soften map-control borders, allow starred segments to
      collapse, and show twenty climb rows before scrolling.
- [ ] Decide how detected climbs should be named and related to matched segments;
      see [climb identification](specs/activity-details.md#climb-identification).

## Performance

Workers prepare derived data; reads stay bounded by the page or selection.
The [performance contract](specs/performance.md) owns read shapes, availability,
and comparison requirements. Each checkbox is a separate PR.

### Activity reads

- [x] **PERF08 — Small dataset and API baseline.** Use existing fixtures/tools for
      1,000 activities and authenticated list/detail/segment comparisons. Deliver
      See the [baseline report](performance/PERF08.md); PR 102 remains under review.
      a reproducible report; exclude capacity, browser, and worker-load testing.
- [ ] **PERF01a — Reliable stored activity ranks.** Verify rank/PR and achievement
      producers across imports, rematching, removals, retries, and completed-empty
      results. Add only missing availability data and a bounded backfill.
- [ ] **PERF01b — Remove activity-detail ranking fallback.** After PERF01a, remove
      history-wide ranking from `activity_segment_effort_rank_maps`. Return stored
      ranks and missing/updating section state. Verify ready, stale, missing,
      zero-effort, repeated attempts, and bounded reads as history grows.
      Exclude new ranking algorithms and synchronous repair.
- [ ] **PERF10a — Prepare activity-card data.** Store resolved location and compact
      previews alongside existing metrics/achievements. Cover edits, replacement,
      and backfill; preserve manual location. Exclude a full schema rewrite.
- [ ] **PERF10b — Read compact activity cards.** After PERF10a, replace full-model
      feed queries and recording-derived cards with narrow page reads. Preserve
      sport filtering and pagination; compare pages of long recordings.
- [ ] **PERF11 — Cheap map cache hits.** Check ownership and prepared route identity
      before loading recording data. Verify route edits invalidate and title edits
      reuse cached geometry. Exclude renderer/style changes.

### Segment detail and playback

- [ ] **PERF03 — Prepared effort playback.** Store reusable effort slices during
      matching, preparing each activity once. Cover repeated attempts, telemetry,
      replacement/deletion, and backfill. Exclude API/UI cutover and downsampling.
- [ ] **PERF02a — Bounded segment API.** After PERF03/PERF01a, provide paginated,
      filtered effort summaries and capped selected-effort data. Authorize every
      ID, return the route once, and generate contracts. Exclude UI migration.
- [ ] **PERF02b / PERF04 — Switch detail/race consumers.** After PERF02a, share the
      new queries and remove the old unbounded comparison endpoint. Preserve
      cross-page selections and race URL order. Exclude interpolation changes.
- [ ] **PERF05 — Isolate playback updates.** After PERF02b, keep filtering, ranking,
      and history-wide setup outside animation renders. Verify fixed rider
      selection stays bounded as history grows; exclude interpolation changes.
- [ ] **PERF06 — Indexed playback lookups.** After PERF05, prepare ranges once and
      reuse interpolation with indexed seeks/forward cursors. Test seeks, ties,
      missing distance, finishes, and gaps; measure 2/10 riders. Exclude camera work.

### Worker and analysis costs

- [ ] **PERF12 — Worker-built segment-edit analytics.** Move rename/delete
      achievement rebuilds into existing worker operations. Enforce deletion
      immediately and preserve start-failure behavior. Exclude other mutations.
- [ ] **PERF13 — Batch rank writes.** Use bounded bulk updates while preserving
      order, ties, transactions, and concurrency protections. Measure one fixed
      segment; exclude matching scope and workflow changes.
- [ ] **PERF14 — Narrow worker rebuilds.** After PERF13, refresh affected old/new
      segments and avoid duplicate achievement rebuilds. Preserve retry recovery
      when removed memberships are lost; exclude a new coordinator.
- [ ] **PERF07a — Prepared analysis data.** After PERF03, store per-effort splits
      and complete-history section bests for supported split counts. Verify
      semantics, backfill, and storage cost; exclude pairwise comparisons and UI.
- [ ] **PERF07b — Paged analysis reads.** After PERF07a, replace full-history
      recording loads with a page, reference, and prepared aggregates. Preserve
      reference/theoretical-best results; exclude new report features.

### Capacity qualification

- [ ] **PERF09a — Large dataset.** After PERF08, extend its manifest/generator to
      one million activities, a heavy owner, and a bounded long-recording cohort.
      Verify storage and expected results; exclude concurrent ingestion.
- [ ] **PERF09b — Concurrent qualification.** After PERF09a and reader cutovers,
      compare 1/10/50 sessions with a real worker processing a fixed batch.
      Report latency, errors, memory, rows/bytes, playback cost, and worker progress.
      Exclude an always-on load service and per-commit million-row gates.

## Other backlog

- [ ] Repair checksumless legacy import metadata without replacing retained files.
- [ ] Remove recording/provider bodies from SQL error diagnostics; retain useful
      operation names and IDs.
- [ ] Investigate the recorded root-disk growth and slow metrics/feature-flag
      queries. Preserve warning thresholds and the operator's CI resource limits.
- [ ] Verify warm deployment Go-cache reuse in CI. Local/manual deployment reused
      the compiled infrastructure program; the CI resource limits stay unchanged.

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

## Strava operations

These are provider verification and operational follow-ups, not worker conversion
gates. Completed migration evidence is in the single worker specification.

- [ ] Verify native alert delivery after the observability rollout; dashboard
      release checks are tracked once under Worker.
- [ ] Cover remaining process bootstrap and SDK metrics-exporter startup with mocked dependencies. Socket/database startup and OS signal wiring remain outside the isolated tests.
- [ ] Verify a fresh authenticated Temporal history session and linked job access.
- [ ] Repair retained alert runbook links to retired documentation.
- [x] Preserve available Strava GPS for maps across all recorded sports, superseding
      the non-cycling stream skip. Keep heatmap and cycling-analysis admission
      independent; cover GPS-free/manual entries and unavailable streams.
- [ ] Release the GPS-map update and resync activity `1684`; verify its feed map
      appears while the hike remains excluded from heatmap and cycling outputs.
- [ ] Observe a live non-cycling summary that remains present in Strava. Walk `20582557310` received a real delete webhook at `14:40:21 UTC`, so it correctly remains absent after the rollout; fixture coverage verifies cycling exclusions.
- [ ] Verify an interruption while a provider or persistence activity is actively executing. The controlled stop/reset/restart drill queued its partial history while the worker was stopped.
- [ ] Support Temporal CLI camelCase JSON exports in the read-only native replay
      tool; file mode currently reports `missing field event_id`. Preserve the
      verified `--workflow` fetch mode and execute no Activities during replay.
- [ ] Verify live backfill and disconnect behavior; retain the observed create,
      update/sync, and delete evidence without treating it as proof of every operation.
- [ ] Resolve the release-tool warning that reports a successfully completed migration container as a warning. CI 403 applied revision `535cdf3e`; Kubernetes confirmed Job `Complete`, exit 0, and no pending migrations. Do not describe this release log as warning-free.
- [ ] Qualify capacity for large backfills and sustained customer concurrency, including shared PostgreSQL resource use.
- [ ] Resolve Temporal's initial `Frontend is not healthy yet` startup warning; keep diagnostics enabled.
