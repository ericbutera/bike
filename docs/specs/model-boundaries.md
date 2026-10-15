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

## Shared fixture contract

- Cross-crate Rust model fixtures live in `bike-core::test_support`.
- Compile them only for unit tests or the existing `test-fixtures` feature.
  Other crates enable that feature in dev-dependencies.
- Keep one complete default per model. Use a fixed timestamp and coherent
  owner IDs. Preserve existing provider-specific identities when adapting them.
- Make scenario differences visible with struct updates; a local helper may
  describe a scenario but must build on the shared default.
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
