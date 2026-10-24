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

- [x] **P2 Metadata/laps:** Save editable metadata and ordered lap facts in typed
      rows during imports and edits, with owner-scoped reads and a focused
      stage-only backfill. B2 owns the API/UI cutover; see
      [metadata and laps](specs/activities.md#typed-metadata-and-laps).

- [x] **P9 Climb facts:** Save the existing report detector facts with typed artifact
      references and convert/backfill only climb output. See
      [saved climb report facts](specs/background-work.md#saved-climb-report-facts).

- [x] **LOAD09:** Read saved activity effort ranks and personal-best summaries,
      preserving nulls and stale saved values without history ranking or request
      repair. See the [activity detail contract](specs/activity-details.md#route-map-and-matched-segments).

- [x] Serve compact activity cards from typed metrics, saved location and ordered
      preview points; prepare them during imports/reprocessing and provide the
      focused `backfill_activity_cards` workflow. See
      [saved card reads](specs/activity-list.md#saved-card-reads).

- [x] Generate and persist ordered activity climbs in the upload/reprocessing DAG;
      include a one-off Temporal backfill for missing climb outputs. See the
      [stage rollout contract](specs/background-work.md#activity-climbs-and-stage-backfills).

- [x] **P1 Recording:** Persist normalized recording artifacts with typed references,
      integrate both import paths, and convert existing saved recordings with a bounded
      stage-only backfill. See [recording storage](specs/activities.md#normalized-recording-storage).

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

## Segments

- [x] **P6 Current results:** Retry changed standings input before awards; submit
      bounded segment batches and refresh the union of shared/removed activity
      summaries once. LOAD04 upload caller composition remains P13; see
      [publication](specs/background-work.md#current-segment-result-publication).

- [x] **P5 Matching bounds:** Persist indexed activity/segment route bounds, narrow
      candidate IDs before recording/geometry hydration, and provide focused bounds
      backfills. Preserve missing bounds and all supported drift profiles. Historical
      orchestration and pair persistence remain P14/P4; see
      [candidate filtering](specs/segment-processing.md#geographic-candidate-filtering).

- [x] **P3 Traversals:** Preserve every valid segment traversal across strict,
      fallback and eligible reworked-trail profiles, in recording order, with stable
      per-segment attempt numbers. See [matching](specs/segment-processing.md#matching-contract).
      This slice does not change persistence or workflows.

- [x] Preserve saved effort identities when activity matching repeats unchanged
      traversals; see [activity reconciliation](specs/segment-processing.md#activity-effort-reconciliation).
      LOAD05 still needs narrowed retry invalidation and historical pair writes;
      LOAD06 and LOAD19 remain prerequisites for gradual historical matching.
- [x] Fix segment geometry after creation and keep creator renaming as a metadata
      update without matching or standings work; see the
      [fixed-geometry contract](specs/segment-processing.md#fixed-geometry-and-creator-renaming).
      PR 100 follow-ups LOAD05, LOAD06, and LOAD19 still own stable effort
      persistence, lock ordering, and gradual historical matching.

- [x] Isolate personal segment results and comparison recordings by authenticated
      user, and isolate browser query data across account changes and logout; see
      [authentication](specs/authentication.md) and [segment visibility](specs/segment-processing.md#segment-ownership-and-visibility).

- [x] Persist each rider's all-time segment PR, second, and third with an idempotent
      Temporal backfill; see [achievements](specs/achievements.md).
- [x] Display saved personal PR, second and third through an authenticated API
      and segment-detail panel; see [achievements](specs/achievements.md#personal-api-and-ui).
- [x] Persist and display overall achievements: a global top ten with one position per athlete;
      see [achievements](specs/achievements.md#overall-achievements).
- [x] Preserve each activity's first earned-award decision, including no award,
      in typed rows, with legacy-highlight conversion and a bounded backfill;
      missing legacy results use live standings. See [earned awards](specs/achievements.md#earned-awards).
- [x] Allow every authenticated athlete to read saved overall standings for a global
      segment, including athletes without an effort.
- [x] Allow authenticated users to browse shared segments through the API and UI,
      with personal results, private source metadata, and creator writes isolated.
- [ ] Replace historical segment regeneration with
      [gradual nearby-activity matching](specs/segment-processing.md#incremental-matching-and-historical-activities).
      Persist conservative matching bounds, then use GPS-filtered Temporal pages
      and retry-safe activity/segment writes. PR 124 was closed without merging;
      its whole-history scan/global replacement does not meet this contract.
      Loading status stays in LOAD05, LOAD06 and LOAD19 of the
      [loading checklist](TODO-fix-data-loading.md).

- [x] Support individual GPX downloads from segment detail; see the
      [download contract](specs/segment-processing.md#segment-downloads).

## Performance

[PR 100](https://github.com/ericbutera/bike/pull/100) and the
[activity-processing spec](specs/performance.md) use the pre-PR-100 application as
the baseline. Keep the existing behavior and finish the saved-data path:
**DAG producers → backend reads → UI**. Completed work is recorded in the
[implemented foundation](specs/performance.md#implemented-foundation);
[LOAD status](TODO-fix-data-loading.md) is maintained separately.

These replace the broad DAG01–DAG09 checklist. Each row is one bounded output or
read cutover, not permission to redesign adjacent features. A producer includes
its own focused backfill/conversion and unit coverage. “After” means merge that
dependency before starting dependent implementation.

### Producer slices

Paths below are relative to `bike-rs/` unless qualified.

| Slice                                     | Work and saved result                                                                                                                                                                                                                             | After                                                   | Owned code and focused proof                                                                                                                                                                                                                                                                                                                                            |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **P1 Recording — merged #135**            | Reuse retained input/parser/object store; persist a normalized recording artifact with typed path/hash/bytes and an owning model API. Leave existing consumers usable during conversion.                                                          | Existing baseline                                       | New `bike-core` recording model/module, owning parse/import persistence and migration. Same input reuses the artifact; ordered samples survive reload; changed input gets a new reference.                                                                                                                                                                              |
| **P2 Metadata/laps — implemented #137**   | Move editable activity metadata and ordered lap facts out of the mixed activity JSON into typed columns/rows. Convert existing saved values and use the same producer for imports.                                                                | P1                                                      | Activity metadata/lap models and parser persistence. Preserve edits, missing optional fields and lap order. No UI redesign.                                                                                                                                                                                                                                             |
| **P3 Traversals — merged #134**           | Make the existing matcher retain every valid traversal across its eligible profiles. A strict lap must not suppress a later drift-tolerant lap; save recording order unchanged.                                                                   | Existing baseline                                       | `bike-core/src/segment_support.rs` matcher and existing matching fixtures. Six laps, mixed profiles, overlapping segments, direction and stable attempt numbers. No persistence/workflow changes.                                                                                                                                                                       |
| **P4 Pair writes**                        | Extend #131 reconciliation to historical activity/segment pairs; narrow invalidation to affected old/new segments. Establish consistent matching/results/deletion lock order and retain interrupted-refresh recovery.                             | P3, P6                                                  | `segment_support.rs`, `segment_regeneration.rs`, affected effort/segment model methods. Unchanged IDs, removed last effort, stale-source rejection, retry and concurrent deletion/import. LOAD05–06. Include [activity 1671 parity](specs/performance.md#activity-1671-regression-acceptance).                                                                          |
| **P5 Candidate bounds — merged #138**     | Save/backfill conservative indexed activity and segment bounds; provide bounded candidate-ID queries before loading recordings. Reuse spatial patterns and all eligible profile tolerances.                                                       | P1, P3                                                  | Dedicated route-bounds model/module and migration; small owning-model query changes. Prove accepted fixture matches are never filtered out, including wrapping and missing bounds. LOAD19 prerequisite.                                                                                                                                                                 |
| **P6 Current results — implemented #136** | Keep #125/#129 batching/atomic results. Make source-change detection retry before awards; refresh affected activity summaries once per outer segment batch, including lost last matches.                                                          | Existing baseline                                       | `analytics.rs`, `jobs/adapter.rs`, existing standings operation and narrow `operations.rs` registration. Multiple overlapping segments cause one summary refresh; changed input cannot freeze no-award. LOAD04 and #129 follow-up. Temporal caller change is reserved for P13.                                                                                          |
| **P7 Period results**                     | Save day/week/month/year/all results from saved efforts, with unique-athlete overall positions and personal fastest times. Reuse existing date/rank semantics; keep awards all-time.                                                              | P6                                                      | New period-result model/module and owning results operation. Repeated attempts, ties, date boundaries, deletion and empty periods. LOAD13 Progress producer.                                                                                                                                                                                                            |
| **P8 Effort samples**                     | Prepare samples plus scalar comparison/split/analysis facts once from saved effort bounds and recording. Store by stable effort ID with a typed artifact reference.                                                                               | P1, P3, P4                                              | New effort-sample model/module, existing pure analysis helpers, migration, owning Temporal operation and replay-safe ingestion connection. Two attempts keep distinct samples/IDs; retry and changed/removed efforts reconcile correctly. LOAD11–12 producer. Include [activity 1671 parity](specs/performance.md#activity-1671-regression-acceptance).                 |
| **P9 Climb facts — merged #133**          | Extend the existing saved climb stage with missing climbing-report facts and a typed artifact reference; convert/backfill only climb output.                                                                                                      | Existing baseline                                       | `activity_climbs.rs`, climb entity/output model and existing detector helpers. Saved metrics match existing fixture calculations; empty result and unchanged retry work. No report controller or workflow edits. LOAD10 producer.                                                                                                                                       |
| **P10 Training facts**                    | Persist existing per-activity training/report facts, typed activity zone rows and four rider zone ceilings. Keep effort-based DH classification dependent on saved efforts. Batch selected activity reads/writes; preserve existing calculations. | P1, P2, P4                                              | `activity_training_analysis.rs`, owning training/zone models and pure report calculations. Existing fixtures give the same facts; multi-activity input uses bounded writes. LOAD10/18 producer.                                                                                                                                                                         |
| **P11a Fitness**                          | Save daily fitness values needed by current views, including no-activity days. Refresh one affected rider/date range per bounded outer operation using existing jobs.                                                                             | P10                                                     | Owning fitness calculation/model. Empty history, earliest requested range, decay days, changed/deleted activity and retry. LOAD07/17 producer; no new scheduler framework.                                                                                                                                                                                              |
| **P11b Goal summaries**                   | Save existing training-goal sessions, progress and recommendation facts from saved training/effort results.                                                                                                                                       | P10, P7                                                 | `services/training_goals.rs` producer and compact result model. Existing goal fixtures, owner/date scope and empty output. LOAD13 goal producer; separate PR from P11a.                                                                                                                                                                                                 |
| **P12 Import recovery**                   | Persist Bike mutation facts in typed storage; avoid replaying completed unchanged import effects while recovering missing effects after interruption. Preserve provider payloads and existing Temporal protocol.                                  | P1, P2, P6, P10, P11a                                   | Strava native persistence/event model and `strava-temporal` upsert workflow. Unchanged completed input skips work; interrupted save resumes necessary effects; changed source/deletion reconciles. LOAD16.                                                                                                                                                              |
| **P13 DAG wiring**                        | Wire the existing named operations to the spec's dependencies. Independent recording branches run without matching; awards await successful results; pass segment batches and batch fitness scope through outer callers.                          | Producers being connected, especially P6/P8/P9/P10/P11a | Remaining outer batching/composition in `bike-temporal/src/workflows/steps/derived.rs`, `activity_pages.rs` and central registration. Producer slices include the stage connection required for real-data acceptance. Existing replay/patch conventions, failed matching, empty matches and retry preserve independent outputs. Completes caller portions of LOAD04/17. |
| **P14 Historical matching**               | Replace the old segment-wide regeneration scan with bounded, geographically filtered Temporal ID pages across riders. Reuse pair writes and refresh affected results per page; never record awards.                                               | P4, P5, P6, P13                                         | Segment regeneration entrypoint and dedicated historical workflow. Cursor retry, another rider, concurrent new import, removed pair and deletion; no full-scan transaction/global effort delete. Completes LOAD19.                                                                                                                                                      |

### Backend read slices

Start a backend slice only when its required saved outputs and focused backfill
path are merged. Missing output stays missing; do not add a request-time fallback.

| Slice                  | Work                                                                                                                                                                                   | After               | Owned code and proof                                                                                                                                                                                   |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **B1 Segment reads**   | Remove segment-list enqueueing; add bounded saved period/effort queries and selected prepared-sample/analysis reads. Keep personal telemetry owner-scoped.                             | P7, P8, P14         | `api/src/controllers/segments.rs` and owning read queries. GET performs no writes/jobs/history ranking/whole-recording slicing; limits apply before loading. LOAD08/12 and Progress portion of LOAD13. |
| **B2 Activity reads**  | Switch detail metadata/laps and matched-effort facts to saved rows/artifacts. Preserve already-merged ranks, awards and cards.                                                         | P2, P8, P9          | `api/src/controllers/activities.rs` and owning projections. Missing saved facts remain null/empty; no legacy JSON/recording-based fact fallback. LOAD11 backend portion.                               |
| **B3 Training reads**  | Read saved report/climb/fitness/goal facts, with owner/date filters in SQL. Remove GET rebuilds, recording expansion, decay calculation and report cooldown writes.                    | P9, P10, P11a, P11b | Report and fitness controllers plus `services/reports.rs` read path. Same fixture results from saved data; empty/missing data does not write or schedule. LOAD07/10/13.                                |
| **B4 Map inputs/read** | Extend saved map inputs with prepared geometry and image identity, then serve warm images after a compact ownership lookup. Include its producer/backfill before switching the reader. | P1, existing cards  | `activity_maps/` and its existing producer. Warm image request does not load/hash a recording; private cache/ownership behavior stays intact. LOAD15 backend portion.                                  |

B1, B2, B3 and B4 can run concurrently once their dependencies merge and their
read-query model edits do not overlap. Do not have them edit common response
types independently; assign any shared type addition to the first owning slice.

### UI and retirement slices

| Slice                           | Work                                                                                                                                                                                                              | After                                                                   | Owned files and proof                                                                                                                                                |
| ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **U1 Segment UI**               | Render saved positions, period results and selected prepared samples. Use server paging; remove browser PR/rank/attempt-result reconstruction. Keep comparison, Race Viewer, Progress and Analysis destinations.  | B1                                                                      | `bike-ui/components/segment-detail/`, segment helpers/pages. Saved ordering, repeated attempts and account switches use existing component/unit fixtures. LOAD11–13. |
| **U2 Activity UI**              | Render saved matched-effort facts, metadata/laps and climbs. Remove telemetry slicing used to calculate domain facts; preserve chip/traversal order and current layout.                                           | B2                                                                      | Activity-detail components and matched-segment helpers. Render deliberately non-sorted saved positions and missing values without replacement calculations. LOAD11.  |
| **U3 Report/map UI**            | Use completed report/fitness/goal contracts and remove legacy full-detail preview loading. Keep existing interactions and formatting.                                                                             | B3, B4                                                                  | Report/fitness consumers and `bike-ui/app/activity-previews/route.ts`. Existing fixtures render saved results; no browser rebuilding. LOAD07/10/13/15.               |
| **C1 Retire converted storage** | Remove remaining conversion-only award blobs and replaced activity/report JSON readers/writers/columns after their owning backfills and read cutovers. Reuse existing removal migrations; no new audit framework. | Relevant producers, backend and UI cutovers; verify backfill completion | Owning legacy fields/converters only. Existing domain fixtures run with obsolete fields absent; immutable awards survive.                                            |

### Current delivery sequence

P1 (#135), P2 (#137), P3 (#134), P6 (#136) and P9 (#133) are merged into main.
The shared real-ride fixture corpus [#139](https://github.com/ericbutera/bike/pull/139)
is also merged, as is P5 [#138](https://github.com/ericbutera/bike/pull/138).

Protect matching now with the existing City/Commons activity fixtures, missing
segment GPXs, fixed City effort expectations and specialized geographic cases.
Run these as ordinary unit tests for affected DAG slices. P5 also exercises
production candidate queries and matching against migrated PostgreSQL in the
existing connected E2E gate. The frozen activity 1671 reference remains available
for P4/P8 persistence, retry and wired-workflow gates.

P4 [#140](https://github.com/ericbutera/bike/pull/140) is in review. After
P5 #138 and P4 #140 merge, pause new producer work for the
[activity 1671 stage-by-stage checkpoint](specs/performance.md#checkpoint-after-prs-138-and-140).
Review every applicable stage's saved values and retry behavior using the exact
frozen activity and segment definitions. PR 139's City/Commons corpus supplies
ordinary matching regressions but is not the exact activity 1671 recording.

The checkpoint also covers existing activity/segment screens, Comparison/Race
Viewer, Progress/Analysis, reports/fitness/goals, heatmaps/maps and lifecycle
behavior through the owning unit and connected fixtures. Record each comparison
as verified, an explained behavior correction, or pending/blocked with its
missing inputs/producer. P8 remains the next planned producer after this review;
do not claim unimplemented stages or unwired ingestion paths passed. The owning
performance spec contains the comparison matrix; do not duplicate it here.

Keep one active implementation owner. P7 period results follows P6;
P10 training facts follows P2/P4. P13 owns outer DAG composition, and P14
uses P4/P5/P6/P13 for historical matching. Backend reads follow their saved
producers; UI follows the corresponding backend contract.

Each subsequent slice owns its model/query operation, migration if needed,
focused backfill, specification and behavior tests. Apply shared model,
migration and workflow registrations in that same slice. Resolve shared-file
edits before validating the final composition, rather than leaving incomplete
registration patches for a different agent.

Implementation PRs update their completed slice and associated LOAD entry.
AGENTS.md owns the development workflow.

## Other backlog

- [ ] Separate storage cleanup: typed API-client scopes, archive error samples
      and queryable integration diagnostic IDs. Keep provider payloads and
      Temporal wire representations. This is outside the activity-DAG cutover.

- [ ] Rebuild CI around plain Cargo and simple Docker builds using the review slices in [CI refactor](ci-refactor.md); defer performance tuning.
- [ ] Activate the [image storage safeguards](specs/image-storage.md) locally and on CI: bounded retention, byte-based GC, and fail-closed build admission. Filesystem isolation for a strict loaded-image quota remains separate.
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
