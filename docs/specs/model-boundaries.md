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
- Result: **220 runtime query-builder call sites in 42 files**: API 58, core 159,
  worker 3, Temporal 0. Query construction remains unchanged in this audit-first
  update; the earlier activity-deletion refactor remains part of the PR.
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
| api/src/controllers/activities.rs | 8 |
| api/src/controllers/activity_imports.rs | 5 |
| api/src/controllers/admin.rs | 13 |
| api/src/controllers/fitness.rs | 2 |
| api/src/controllers/segments.rs | 26 |
| api/src/controllers/user_preferences.rs | 3 |
| api/src/metrics.rs | 1 |
| bike-core/src/activity_import_execution.rs | 2 |
| bike-core/src/activity_import_lifecycle.rs | 1 |
| bike-core/src/activity_import_lock.rs | 2 |
| bike-core/src/activity_import_pipeline.rs | 5 |
| bike-core/src/activity_import_pipeline/replay.rs | 2 |
| bike-core/src/activity_import_pipeline/stage_execution.rs | 1 |
| bike-core/src/activity_import_recovery.rs | 3 |
| bike-core/src/activity_lifecycle.rs | 7 |
| bike-core/src/activity_training_analysis.rs | 4 |
| bike-core/src/analytics.rs | 9 |
| bike-core/src/archive_import.rs | 1 |
| bike-core/src/archive_import/batch.rs | 7 |
| bike-core/src/auth/controllers/auth.rs | 1 |
| bike-core/src/auth/extractors/auth_info.rs | 1 |
| bike-core/src/auth/extractors/user_context.rs | 3 |
| bike-core/src/background_jobs/admin.rs | 4 |
| bike-core/src/background_jobs/batches.rs | 15 |
| bike-core/src/background_jobs/batches/reprocessing.rs | 1 |
| bike-core/src/background_jobs/batches/segments.rs | 5 |
| bike-core/src/background_jobs/diagnostics.rs | 25 |
| bike-core/src/background_jobs/diagnostics/graph.rs | 9 |
| bike-core/src/background_jobs/diagnostics/health.rs | 3 |
| bike-core/src/background_jobs/diagnostics/statistics.rs | 1 |
| bike-core/src/heatmaps/data.rs | 2 |
| bike-core/src/heatmaps/projection.rs | 12 |
| bike-core/src/platform/feature_flags/service.rs | 3 |
| bike-core/src/segment_regeneration.rs | 4 |
| bike-core/src/segment_support.rs | 4 |
| bike-core/src/services/reports.rs | 7 |
| bike-core/src/services/training_goals.rs | 8 |
| bike-core/src/strava/connection.rs | 3 |
| bike-core/src/strava/native/persistence.rs | 2 |
| bike-core/src/training_profile.rs | 1 |
| bike-core/src/xc_goal_backfill.rs | 1 |
| worker/src/tasks/processors/process_activity_import.rs | 3 |
| **Total** | **220** |

### Follow-up migration order and acceptance

- API activity/import access: reuse `find_owned`, move list pagination/sport
  filtering, import hydration, and original-source selection onto their owners.
  Preserve owner isolation, supported-source quality ordering, totals, and ties.
- Segment operations: move repeated owner reads, dedupe candidate selection,
  effort/leaderboard joins, and the four-table deletion sequence behind named
  entity/domain methods. Preserve the existing transaction and cache rebuilds.
- Preferences, fitness, reports, and goals: share owner/profile reads and bounded
  analytics queries. Keep ride eligibility and freshness interpretation central.
- Import lifecycle/recovery and worker reads: share resumable-import selection,
  attempt lookups, linked activities, reference counts, and owned locks. Preserve
  retry, cancellation, deduplication, and transaction boundaries.
- Background jobs and diagnostics: move graph reads, unfinished-output
  subqueries, retention deletes, and batch mutations to owning persistence
  methods. Preserve page limits/cursors, retention roots, leases, and revision
  fences; diagnostic DTO assembly remains a domain concern.
- Heatmaps, auth, feature flags, and Strava: keep source changes and publication
  generation/row locks atomic; preserve enabled-user and token policy.
- For each slice, remove the raw builders from production call sites, reuse
  existing methods/fixtures, and run the owning happy-path and failure tests.
  Update this inventory and `docs/TODO.md` as slices are completed.
