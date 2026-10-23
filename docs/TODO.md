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

- [x] Generate and persist ordered activity climbs in the upload/reprocessing DAG;
      include a one-off Temporal backfill for missing climb outputs. See the
      [stage rollout contract](specs/background-work.md#activity-climbs-and-stage-backfills).

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
- [ ] Match activities across global segments in both activity and segment
      regeneration; see [segment visibility](specs/segment-processing.md#segment-ownership-and-visibility).

- [x] Support individual GPX downloads from segment detail; see the
      [download contract](specs/segment-processing.md#segment-downloads).

## Other backlog

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
