# Read performance

Workers prepare derived data; API/UI reads stay bounded by the displayed page
or selected efforts. This is the target contract. [TODO](../TODO.md#performance)
tracks delivery; [background work](background-work.md) owns the existing pipeline.

## Prepared reads

| Surface          | Read boundary                                                                                                             |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------- |
| Activity feed    | One page of scalar metrics, location, compact previews, and stored achievements; no recording JSON.                       |
| Activity detail  | The owned activity and its efforts with stored ranks/PR context. Load route/chart/lap data once and share it across tabs. |
| Segment detail   | Stored summary and server-filtered, paginated effort rows. Fetch geometry separately.                                     |
| Race viewer      | The common route once and prepared series for explicitly selected, authorized effort IDs, with a selection cap.           |
| Segment analysis | Prepared splits and complete-history best-section aggregates; read a page plus its selected reference.                    |
| Activity map     | Check ownership and prepared route identity before cache lookup; load route data only on a miss.                          |

- Extend existing projections and shared queries. Small indexed joins are fine.
- Preserve repeated attempts, timing, telemetry, ownership, and selection order.
  Any downsampling needs accuracy fixtures. Do not store every pairwise comparison.
- Reuse loaded data across screens. Invalidate only affected queries. Title edits
  must not regenerate geometry.
- Prepare playback series once; use indexed seeks and reuse interpolation per
  rider/frame. Keep history-wide work outside animation updates.

## Stored-result availability

- GETs return stored output; missing/stale results never trigger history scans,
  recalculation, or repair.
- Keep stale output usable with an updating state. Show missing results as
  unavailable for that section; distinguish completed-empty from unprocessed.
- Use minimal freshness/completion information on the owning dataset and existing
  Temporal status. No generic readiness ledger or output coordinator.
- Preserve transactional replacement and existing source-change checks.
  Deletion and access restrictions take effect immediately.
- Cover imports, reprocessing, relevant edits/removals, retries, and a bounded
  backfill. Deploy and verify the producer before cutting over its reader.

## Reproducible performance datasets

Reuse existing fixture factories, real migrations, PostgreSQL, ordinary auth,
and an isolated disposable stack. Fix the seed and logical clock; record a
manifest and storage budget. Never seed the developer's database.

| Profile                    | Purpose                                                                                |
| -------------------------- | -------------------------------------------------------------------------------------- |
| Small (PERF08)             | 10 owners × 100 activities; repeatable API comparisons and correctness checks.         |
| Cardinality (PERF09a)      | 200 owners × 5,000 activities, including a heavy owner and skewed segment histories.   |
| Recording cohort (PERF09a) | A bounded set of long recordings and repeated attempts; playback with 2 and 10 riders. |
| Concurrent (PERF09b)       | Reads at 1/10/50 sessions while the real worker processes a fixed batch.               |

- Include ready, stale, missing, zero-effort, tied, and repeated-attempt fixtures.
  Verify small seeded projections against real worker calculations.
- Report actual rows, points, bytes, and storage. Lightweight records do not
  establish long-recording performance; seeded data does not prove worker throughput.
- Extend the small runner as needed. Keep generated output in Docker and clean
  task-owned stacks after use. Retain comparison reports with their PRs.
- Large/concurrent runs are explicit qualification, not per-commit gates.

## Before-and-after evidence

- Compare the same fixture and scenario on baseline/candidate release builds.
  Record revision/image IDs, command, resource limits, and cache state.
- Warm up, then collect three runs of at least 200 requests per changed scenario.
  Report p50/p95, errors, SQL calls/rows, and response bytes. Measure CPU/memory
  or browser/worker costs when relevant; label anything unmeasured.
- Check correctness and bounded work as unrelated history grows. Keep failed
  requests in results; stop at a declared timeout/resource budget.
- Explain regressions and run-to-run variation. Claim improvement only when the
  measured difference exceeds that variation.

## Scope

- Each TODO slice is a separate PR. Reuse the existing pipeline, fixtures, and
  tools; separate producers/backfills from reader cutovers.
- No database replacement, generic caching/coordination framework, or broad UI
  rewrite. Record unrelated findings for later slices.

## PERF08 small baseline

- `mise run perf:small` owns a disposable PostgreSQL/API stack using the existing
  runtime images and migrations. It generates ten non-admin owners with 100
  activities each, 201 GPS points per activity, two repeated attempts on each
  nonempty activity, and tied durations. Shared Rust fixtures provide defaults;
  the existing analytics builders prepare ranks and summaries before measurement.
- The seed contains empty, ready, missing, and stale activity result examples.
  Correctness checks cover tenant isolation, counts, repeated occurrences, route
  bounds, and stored rankings. Synthetic short recordings are not the later
  long-recording or skewed-capacity profile.
- Measure activity list/detail and segment summary/comparison at concurrency one,
  with ten warmups and three runs of 200 requests per scenario. Record the first
  request separately; it is not a cold database/cache claim.
- PostgreSQL `pg_stat_statements` deltas give query calls, execution time, and
  returned/affected rows for each request batch, including authentication.
  Report SQL execution time separately from HTTP wall time; their difference
  also includes networking and pool waits, not just application computation.
- The database has a 512 MiB filesystem ceiling; fixture storage is limited to
  128 MiB. API and PostgreSQL each have two CPUs and 1 GiB memory. No worker,
  browser, concurrent workload, application optimization, or new API is included.
- [The initial PERF08 report](../performance/PERF08.md) retains the two successful
  baseline runs and documents unmeasured metrics and workload limitations.
