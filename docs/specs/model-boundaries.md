# Model boundaries and test fixtures

## Required ownership

- Controllers adapt HTTP input/output and call an application operation.
- Services and workers compose domain operations. Call sites expose intent,
  relevant IDs, and ownership scope; they do not assemble persistence policy.
- Owning entity/model modules construct and execute queries, including filters,
  joins, ordering, aggregates, pagination, locks, conflict handling, and writes.
- Reuse existing operations before adding methods. A query used once still
  belongs to its owner. Return domain records/results, not unfinished builders.
- Keep multi-entity transaction orchestration in a cohesive domain operation;
  delegate each entity's persistence details to that entity. Preserve lock scope,
  generation fences, affected-row checks, and commit-before-publication behavior.
- Split meaningful domain stages when a method grows. Do not move a large query
  chain into a generic service helper or replace explicit inputs with a vague bag.

For example, activity ownership checks use the existing
`activities::Model::find_owned(db, activity_id, user_id)`. The HTTP adapter maps
an absent result to the relevant response. A shared-import reference count stays
on `activities::Entity`, and deleting the import stays on `activity_imports::Entity`.

## Activity/import API operations

- Activity lists call `activities::Model::paginate_for_user` with the owner,
  pagination, and optional sport. The model owns legacy sport matching, the
  filtered total, normalized page bounds, and descending start-time/ID order.
- Activity detail, update, download, delete, and regeneration reuse `find_owned`.
  Import and archive-job reads also require their owner explicitly.
- `activity.update_details` owns title validation and the partial write. A blank
  title rejects the whole update; omitted fields retain their stored values.
- `activities::Model::linked_to_imports` hydrates only the owner's activities.
  An import's explicit activity link takes precedence over the activity's reverse
  import link. A missing or foreign explicit link leaves an owned reverse link
  intact. An empty import list performs no query.
- Recent manual uploads retain the 25-row limit and manual-source filter.
  Recent archive jobs retain the 10-row limit. Both sort by creation time
  descending and remain scoped to their owner.
- `activity_import_artifacts::Entity::downloadable_original` reuses the ordered
  artifact lookup. It selects only original FIT/TCX/GPX records for that owner
  and import. Preserve quality scores: FIT original 500, unknown FIT 490, TCX
  original 400, unknown TCX 390, GPX original 300, unknown GPX 290.
  Equal scores select the last candidate; ascending artifact-ID lookup makes
  that the highest ID. Generated exports and unsupported formats cannot win.
- HTTP status mapping, response formatting, path safety, and download headers
  remain in the API. Existing delete/regeneration workflows retain their
  transaction, publication, and cache-rebuild sequence.
- Focused checks cover persisted owner/source/sport filtering, totals, ordering,
  link precedence, partial writes, list limits, and artifact selection using
  shared fixtures. Mocked checks cover empty reads and database-error propagation.
- API checks exercise list/detail/update/download responses, hydrated imports,
  archive reads, hidden foreign records, and regeneration with a foreign import
  link. A rejected update preserves stored fields; downloads return retained bytes.

## Insert defaults and timestamps

- Declare insert defaults with SeaORM's `default_value` and `default_expr` on
  the owning entity. PostgreSQL migrations must supply the corresponding defaults;
  entity attributes alone do not change an existing database.
- Leave defaulted insert fields `NotSet`. Preserve explicit overrides, including
  historical timestamps. Omitted fields in update patches remain unset.
- Use `crate::timestamps::impl_timestamps!(ActiveModel)` for ordinary model
  updates. The shared SeaORM `before_save` hook refreshes only `updated_at` on
  updates; it leaves all insert values and `created_at` untouched. Account and
  feature-flag updates follow the same convention.
- Bulk updates and raw SQL supply their own update timestamps because they
  bypass ActiveModel hooks. Event, scheduling, expiry, freshness, and rebuild
  timestamps remain explicit domain inputs.
- Queue timestamps are claim identities. Keep the queue's insert hook using one
  application-clock instant and preserve explicit values; task state transitions
  continue owning their timestamps. Do not apply the ordinary update hook there.
- This convention does not introduce soft deletion. Existing deletes retain
  their ownership, transaction, and derived-state cleanup behavior.
- Test shared behavior once with mocks and representative existing fixtures.
  Verify database defaults, overrides, partial updates, and queue bulk inserts
  without a central list of entity seeds. Keep focused regressions for domain
  exceptions such as legacy import versions. Migration files contain schema
  changes; verify application behavior in the owning model tests.

## Shared fixture contract

- Use fixture factories with shared test helpers. Keep routine setup shared so
  each test shows only its meaningful inputs, actions, and assertions.
- Cross-crate Rust model fixtures live in `bike-core::test_support`.
- Compile them only for unit tests or the existing `test-fixtures` feature.
  Other crates enable that feature in dev-dependencies.
- Fixtures provide valid, reusable defaults with fixed timestamps and coherent
  owner IDs. Preserve existing provider-specific identities when adapting them.
- Override only fields relevant to the scenario, using struct updates. A local
  helper may describe a scenario but must build on the shared default.
- Insert fixtures with `test_support::insert(&db, fixture_with_overrides).await`.
  Use Rust's `ActiveModel { field: Set(value), ..fixture() }` struct-update syntax
  to combine fixture fields and explicit overrides without a runtime field map
  or separate fixture registry.
- All Rust unit-test fixture inserts and updates use the shared `insert` and
  `update` helpers in `bike-core::test_support`, including API and worker tests.
  Pass update ActiveModels with their identity and changed fields only; use
  `..Default::default()` for omitted fields and `Set(None)` for explicit clears.
  The helpers accept connections and transactions through `ConnectionTrait`.
  Direct bulk statements and expected-error operations remain explicit when
  their database behavior is the assertion being tested.
- SQLite fixture timestamp defaults must round-trip through SQLx's RFC 3339
  parameter format so timestamp equality and version fences match PostgreSQL's
  typed timestamp behavior. Reuse `table_with_timestamp_defaults` for those tables.
- Insert fixtures provide required business inputs and deterministic timestamps.
  Leave database-owned defaults unset unless the test exercises an override;
  set timestamp fields to `NotSet` when testing database-generated timestamps.
- Use `new_activity()` and `new_activity_import()` for inserts with generated
  primary keys. These return ActiveModels whose IDs are unset.
- Retain partial ActiveModels used to update specific stored fields. They are
  deliberate patches and must not accidentally populate unrelated fields.
- Tests may inspect persistence directly. Unit tests use mocks and fixtures;
  existing owning database tests verify behavior that depends on persistence.
- Regression checks cover stable fixture identity/time and distinct generated
  IDs across repeated inserts, as well as the existing consuming scenarios.

```rust
let import = activity_imports::Model {
    id: 1042,
    user_id: 42,
    activity_id: Some(42),
    ..bike_core::test_support::activity_import()
};
```

## Audit snapshot: 2026-10-14

- Scope: handwritten Rust in `bike-core`, `api`, `worker`, and `strava-temporal`.
  Exclude generated SDK/vendor code, migrations, fixture executables, owning
  entity modules, test-only files, and inline `cfg(test)` blocks from runtime
  query counts. Resume scanning after inline test blocks.
- Count query-builder starts (`Entity::find`, `find_by_id`, `delete_many`,
  `delete_by_id`, `update_many`, `insert`, and `insert_many`), not each chained
  method. Nested subqueries count separately. This includes simple primary-key
  lookups as well as complex queries; it is a migration inventory, not 220
  distinct bugs or a claim about business behavior.
- Original audit: **220 runtime query-builder call sites in 42 files**: API 58,
  core 159, worker 3, Temporal 0.
- Completed migration: **0 remaining audited runtime query-builder call sites**.
  All 220 original starts in 42 caller files moved to or reused owning operations.
  Two additional aliased OAuth lookups also moved to the user model.
  Shared framework aggregation remains in the platform data layer.
- Test inventory: **103 qualified `Model`/`ActiveModel` construction or patch
  expressions in 38 files** before fixture consolidation. These are candidates
  for review, not 103 proven duplicate fixtures. Unqualified entity aliases,
  DTOs, JSON payloads, and existing fixture calls are outside this textual count.
- Imports: **12 expressions in nine files**, comprising **11 seed constructors
  and one update-only patch**. All 11 seeds now reuse the shared import fixture;
  the metrics timestamp patch intentionally remains a partial ActiveModel.
- Shared activity defaults now support lifecycle, analytics, training analysis,
  reports, API responses/history/fitness/metrics, worker pipeline tests, and
  Strava native tests. Existing Strava account/connection fixtures moved to the
  shared module, with compatibility exports for adapter consumers.

### Repeated test-model families before consolidation

Counts include explicit scenario overrides and update-only patches. Do not
replace an intentional patch with a full fixture.

| Family | Construction/patch sites | Files |
| --- | ---: | ---: |
| activities | 20 | 12 |
| activity_imports | 12 | 9 |
| users | 9 | 8 |
| analytics_user_states | 6 | 3 |
| fitness_freshness_daily | 6 | 2 |
| segment_efforts | 5 | 2 |
| strava_connections | 3 | 3 |

### Runtime query inventory

Paths are relative to `bike-rs`. Counts exclude owning entity implementations.

| Module | Query starts |
| --- | ---: |
| api/src/controllers/activities.rs | 0 (complete) |
| api/src/controllers/activity_imports.rs | 0 (complete) |
| api/src/controllers/admin.rs | 0 (complete) |
| api/src/controllers/fitness.rs | 0 (complete) |
| api/src/controllers/segments.rs | 0 (complete) |
| api/src/controllers/user_preferences.rs | 0 (complete) |
| api/src/metrics.rs | 0 (complete) |
| bike-core/src/activity_import_execution.rs | 0 (complete) |
| bike-core/src/activity_import_lifecycle.rs | 0 (complete) |
| bike-core/src/activity_import_lock.rs | 0 (complete) |
| bike-core/src/activity_import_pipeline.rs | 0 (complete) |
| bike-core/src/activity_import_pipeline/replay.rs | 0 (complete) |
| bike-core/src/activity_import_pipeline/stage_execution.rs | 0 (complete) |
| bike-core/src/activity_import_recovery.rs | 0 (complete) |
| bike-core/src/activity_lifecycle.rs | 0 (complete) |
| bike-core/src/activity_training_analysis.rs | 0 (complete) |
| bike-core/src/analytics.rs | 0 (complete) |
| bike-core/src/archive_import.rs | 0 (complete) |
| bike-core/src/archive_import/batch.rs | 0 (complete) |
| bike-core/src/auth/controllers/auth.rs | 0 (complete) |
| bike-core/src/auth/extractors/auth_info.rs | 0 (complete) |
| bike-core/src/auth/extractors/user_context.rs | 0 (complete) |
| bike-core/src/background_jobs/admin.rs | 0 (complete) |
| bike-core/src/background_jobs/batches.rs | 0 (complete) |
| bike-core/src/background_jobs/batches/reprocessing.rs | 0 (complete) |
| bike-core/src/background_jobs/batches/segments.rs | 0 (complete) |
| bike-core/src/background_jobs/diagnostics.rs | 0 (complete) |
| bike-core/src/background_jobs/diagnostics/graph.rs | 0 (complete) |
| bike-core/src/background_jobs/diagnostics/health.rs | 0 (complete) |
| bike-core/src/background_jobs/diagnostics/statistics.rs | 0 (complete) |
| bike-core/src/heatmaps/data.rs | 0 (complete) |
| bike-core/src/heatmaps/projection.rs | 0 (complete) |
| bike-core/src/platform/feature_flags/service.rs | 0 (complete) |
| bike-core/src/segment_regeneration.rs | 0 (complete) |
| bike-core/src/segment_support.rs | 0 (complete) |
| bike-core/src/services/reports.rs | 0 (complete) |
| bike-core/src/services/training_goals.rs | 0 (complete) |
| bike-core/src/strava/connection.rs | 0 (complete) |
| bike-core/src/strava/native/persistence.rs | 0 (complete) |
| bike-core/src/training_profile.rs | 0 (complete) |
| bike-core/src/xc_goal_backfill.rs | 0 (complete) |
| worker/src/tasks/processors/process_activity_import.rs | 0 (complete) |
| **Total remaining** | **0** |

### Completed migration and acceptance

- Completed API activity/import access: reuse `find_owned`; owners handle list
  pagination/sport filtering, import hydration, original-source selection, and
  activity patches. Preserve owner isolation, quality ordering, totals, and ties.
- Segment operations now own scoped reads, dedupe candidate selection,
  effort/leaderboard joins, and the four-table deletion sequence, preserving
  the transaction and cache rebuilds.
- Preferences, fitness, reports, and goals share owner/profile reads and bounded
  analytics queries, with centralized ride eligibility and freshness rules.
- Import lifecycle/recovery and workers share resumable-import selection,
  attempt lookups, linked activities, reference counts, and owned locks,
  preserving retry, cancellation, deduplication, and transaction boundaries.
- Background jobs and diagnostics delegate graph reads, unfinished-output
  subqueries, retention deletes, and batch mutations to owning persistence
  methods, preserving page limits/cursors, retention roots, leases, and revision
  fences. Diagnostic DTO assembly remains a domain concern.
- Heatmaps, auth, feature flags, and Strava retain atomic source changes,
  publication generation/row locks, enabled-user policy, and token policy.
- Query construction lives on owning models, including scoped projections,
  partial report/leaderboard records, diagnostic aggregates, retention selection,
  locks, batch mutations, and compare-and-swap publication guards. Callers consume
  complete results and compose named operations.
- Fixture consolidation covers activities/imports, users, analytics user states,
  daily fitness, segment efforts, and Strava connections. Shared insert helpers
  leave generated primary keys unset; update-only patches remain partial.
- Regression coverage includes segment-deletion rollback, filtered report reads,
  owner-scoped segment dedupe, and distinct generated IDs. Existing domain tests
  remain the behavior checks for retries, cancellation, barriers, cursors,
  retention, generation fences, authentication, and source identity.
- Validate the complete PR after the migration and fixture edits, then resolve
  failures together. Keep the full coverage and integration gates in CI.

## Migration regression coverage

- Changed-line coverage must meet the existing per-crate 80% gate before integration.
- Unit tests exercise heatmap owner/time/sport predicates, spatial cursor bounds,
  chunk batching, generation fences, atomic publication, and write rollback with
  mocked PostgreSQL storage. Native PostgreSQL behavior stays in the owning SQL suite.
- Segment and preference handler tests verify owned results and persisted edits.
  Worker tests verify explicit import links, owned fallback links, and terminal skips.
- Model tests cover ranked segment reads, stored match indices, training projections,
  OAuth identity links, and feature-flag storage/cache consistency.

## Mutation boundaries: 2026-10-15

- Baseline: `220ecaad` (PR #58). Preserve the completed query-start audit above
  as historical evidence; it did not count writes assembled through ActiveModels.
- The next audit found **24 qualified ActiveModel constructors in 12 caller files**.
  Including conversion-based updates/deletes and ORM-valued helpers expands
  the initial inventory to **19 caller files**. These are migration candidates, not
  24 independent defects. Tests, owning entity modules, and fixture executables
  use the same exclusions as the query audit.
- Production callers must not construct, convert, or return ActiveModels for
  another layer to finish. Owning model methods construct and execute the write.
- Keep meaningful workflow orchestration in its domain/service module. Use
  explicit ownership IDs and cohesive commands, source slices, checkpoints,
  or computed summaries where those values share a real boundary.
- Preserve partial-update semantics, source identity, generated IDs, timestamps,
  row locks, transaction scope, compare-and-swap predicates, and publication order.

### Constructor inventory at the baseline

| Caller | Constructors |
| --- | ---: |
| api/src/controllers/user_preferences.rs | 1 |
| api/src/controllers/segments.rs | 2 |
| bike-core/src/strava/connection.rs | 1 |
| bike-core/src/analytics.rs | 6 |
| bike-core/src/activity_import_execution.rs | 1 |
| bike-core/src/archive_import.rs | 4 |
| bike-core/src/activity_training_analysis.rs | 1 |
| bike-core/src/activity_import_lock.rs | 1 |
| bike-core/src/activity_import_pipeline.rs | 3 |
| bike-core/src/activity_import_lifecycle.rs | 1 |
| bike-core/src/provider_rate_limit.rs | 2 |
| bike-core/src/background_jobs/execution.rs | 1 |
| **Total** | **24** |

### Mutation acceptance

- Segment models own detail validation/patches, activity-derived route replacement,
  duplicate builder-source adoption, and uploaded-route insertion. Title changes
  and activity analytics rebuilds share a transaction. Regeneration is queued after
  the route update commits. Duplicate adoption retains the stored route/mode/star.
- Preference models validate a complete update before writing, apply profile/goal
  fields once, and preserve omitted-value and goal-reset behavior. A small service
  performs the existing backfill handoff after the save and reloads its resulting
  state. Backfill enqueue failures retain the existing committed-save behavior.
- Import models own processing, processed, failed, duplicate, replay, requeue,
  artifact creation/relinking, and recovery writes. Preserve attempt reuse,
  failure counters, stage checkpoints, ownership, and revision fences.
- Analytics owners persist computed daily fitness, segment/user summaries,
  effort ranks, and activity highlights. Domain code retains calculation and
  multi-entity transaction orchestration.
- Lock, quota, credential, account, archive-job, and work-evidence owners apply
  their own writes, preserving unique-conflict handling, refresh metadata,
  reservation transactions, and evidence failure policy.
- Validate the completed PR as a batch with existing mocks/fixtures and focused
  regression coverage. Keep the 80% changed-line gate and required hooks active.
- Worker payload parsing, general error mapping cleanup, and nested aggregation
  remain separate follow-ups.

### Completed mutation migration

- Moved mutation assembly and execution from the 19 initial callers into their
  owning models, including conversion-based patches, artifact copies/deletion,
  and fitness-row construction.
- The final audit also found the unqualified durable-queue constructor in
  `background_jobs/durable.rs`. Its existing `Model::enqueue` operation now
  lives in `background_jobs/entities/background_tasks.rs`; the durable adapter
  retains its public re-export. This brings the completed scope to **20 caller
  files**, with **zero remaining production ActiveModel constructions or
  conversions** outside owning modules in the audited runtime files.
- Segment detail patches, builder sources, route replacement/import, and duplicate
  adoption live in `entities/segments/mutations.rs`. A builder source validates
  the owned activity and route slice before use. Existing modes/stars survive
  route replacement and duplicate adoption.
- Preference validation and persistence live in
  `entities/user_preferences/mutations.rs`; the preference service performs
  the backfill handoff. The HTTP request schema keeps its existing name and fields.
- Import lifecycle, replay/requeue, parsed activity persistence, archive jobs,
  artifact copies, attempt checkpoints, analytics summaries, quota reservations,
  credential encryption/refresh, admin promotion, queue insertion, and work
  evidence use purpose-named owner methods. Domain modules retain file/network
  operations, queue handoffs, calculation, and multi-model transaction ordering.
- Regression cases cover whole-patch validation, no-op writes, segment rename
  rollback, builder owner checks/deduplication, retained mode/star, preference
  insert/update/clear and backfill decisions, ingestion failure/reuse, copied
  artifact identity, encrypted credential refresh, archive transitions, and
  successful/failed work evidence.

### Validation evidence

- Local affected-library batch plus focused correction run: **509 passing unit
  cases** across API, core, and worker; **two existing cases remain ignored**.
  All 42 model/handler boundary cases and the admin bootstrap case pass,
  including the 18 new regressions.
- Required workspace Clippy passed with all targets/features and warnings denied.
  Required formatting and hooks run again against the final staged commit.
- Core and API reuse a shared SQLite table helper to reproduce the migration's
  segment freshness and account timestamp defaults.
  Production migration history and transaction behavior are unchanged.
- Full changed-line coverage, PostgreSQL, integration, build, and browser gates
  remain the CI acceptance evidence for this PR.

### Shared field mapping follow-up

- Share cohesive field groups on the owning `ActiveModel`: a builder link, route,
  activity-derived segment source, athlete profile, sealed tokens, processing stage,
  completion, and archive progress each have one named mapping.
- Use consuming `with_…` methods and Rust struct-update syntax. Each operation
  supplies its meaningful differences and reuses the common field mapping.
  Creation uses model/database defaults; updates start from the stored
  model so identity, omitted fields, and unrelated state stay intact.
- Reuse the existing parsed-activity and validated-preference mappings; keep
  domain-specific mapping local to its owner.
- Preserve SeaORM's `Set`, `Unchanged`, and `NotSet` semantics. An omitted update
  differs from an explicit `Set(None)`; shared mapping must retain that distinction.
- Builder-link adoption changes the title/link while retaining stored route
  geometry and import metadata. Route replacement uses the fuller source mapping
  and retains mode/star. Creation uses the database's default mode/star.
- Credential connect/refresh share profile and encrypted-token mapping. Refresh
  keeps the stored scope and athlete profile when those inputs are omitted. A
  supplied athlete replaces its nullable fields, including explicit null values.
- Import transitions share stage/completion fields. Failure supplies its own
  message/attempt count; native completion retains the prior processing-event
  timestamp; an omitted activity link leaves the current link intact.
- Archive progress updates and completion share counter mapping. Queue/restart
  use database defaults and explicit counter resets respectively, preserving each
  operation's status, errors, and timestamps.
- Focused regressions check retained route/import metadata, native event/identity
  preservation, supplied and omitted credential fields, and progress/restart state.
- Follow-up validation: all **46 focused model/handler boundary cases** pass,
  including four additional mapping regressions (**22 new cases** in the PR).

### Necessary fields and insert defaults correction

- Insert literals contain only business inputs and meaningful overrides. Omit
  generated IDs, null placeholders, zero counters, and flags already supplied
  by the owning model or database. Computed input values remain explicit even
  when their result happens to be zero.
- Task enqueue supplies type, payload, retry policy, and schedule. Its owner
  supplies initial pending status and timestamps; the database supplies attempts.
- Attempt creation supplies import/source/stage/reuse data. Its owner supplies
  queued status and timestamps; nullable error and execution times stay unset.
- Raw upload persistence supplies upload metadata and its processing stage/event.
  Import version comes from existing model behavior, attempts from the database,
  and absent activity/completion/error fields stay unset.
- Segment mode/star, credential sync state, archive counters, quota usage, and
  user disabled state use their existing database defaults. Entity metadata
  declares those defaults so generated fixture tables match production.
- Default ActiveModels remain empty patches. Insert hooks fill only omitted
  creation fields and preserve explicitly supplied values. Do not introduce
  creation defaults into update literals or replace them with blanket
  `default_values()`.
- Retain explicit clears and resets for existing rows: stage errors, reconnect
  disconnect state, route source metadata, and archive retry counters.
- Focused regressions verify task/attempt creation defaults and upload defaults;
  existing cases cover retained flags, sync state, progress, retries, and updates.
- Historical migrations remain unchanged; this correction uses existing schema
  defaults and model insert behavior.
- Local correction batch: **515 passing API/core/worker library cases**, with
  **two existing ignored cases**, including **48 model/handler boundary cases**
  and **24 new regression cases** across the PR. Required formatting and Clippy
  run against the final staged correction with warnings denied.
