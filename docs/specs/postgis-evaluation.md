# PostGIS data-access and resource evaluation

Status: experiment specification, 2026-10-03. No product implementation or
deployment is authorized by this experiment. Work and results belong under
[`experiments/postgis`](../../experiments/postgis/README.md); completion status
belongs in [the Bike backlog](../TODO.md#personal-heatmaps).

## Evaluation question

Measure whether Rust with PostGIS provides enough benefit over Rust with vanilla
PostgreSQL to justify spatial projections and their operational/storage cost.
Evaluate segment matching and proposed heatmaps directly, and establish controls
for activity detail, static map rendering, and race viewer data access.

The decision needs measured CPU time, memory, physical storage, query/response
latency, rows/vertices/bytes materialized, and preparation/write costs. SQL
execution time alone does not answer it. Move work between PostgreSQL and Rust
only when the complete operation improves or an explicit tradeoff is acceptable.

Deliverables in the current phase are this spec, a reproducible fixture preparer,
two concrete SQL dumps, source-derived workload profiles, and restore/consistency
verification. The follow-up request starts a repeatable Compose harness and
initial timed Rust/PostGIS data-access probes. Full renderer/resource/adoption
measurements remain a later experiment scope. No performance advantage is
claimed from fixture preparation.

## Repeatable experiment and learning loop

The [Compose harness](../../experiments/postgis/README.md) owns disposable local
databases and release-mode Rust probes. Each run gets a unique project, fresh
database volume and new output directory. The four arms share identical server
binaries/settings; the raw fixture is restored separately into each arm database.
Normal completion, failure and handled interruption remove that project's
containers, database volume and internal network. Reports/build cache persist.
No application volume, network, public port or live database is part of a run.

Use versioned [ideas](../../experiments/postgis/ideas.json) and
[test configurations](../../experiments/postgis/experiments/EXP001.json). Each test
records a proposed change, predicted evidence, idea IDs, parameters and limits.
Every run captures source contents/hashes, dirty/source revision, exact fixture
hashes, compiler/image versions, environment/settings, workload selection and raw
observations. The append-only local index links idea → test revision → outcome;
the [notebook](../../experiments/postgis/notebook.md) records reviewed observations,
interpretation and the next test. Preserve failed tests and increment the test
revision when correcting an independent variable or measurement definition.

Produce Markdown and JSON reports, separate instrumented server plans, per-stage
timings, actual CPU counters, memory scope, datum/vertex/row counts and physical
relation sizes. Cross-arm output fingerprints must agree before ranking results.
Rerun comparisons reject different inputs, workloads, environments, sampling or
outputs and identify source changes. A small smoke run proves execution and
cleanup; the initial pilot shows measured direction and scope, with p95 omitted
below 100 observations. Neither completes the full adoption gate below.

The initial implementation uses a synchronous PostgreSQL client with the owning
Rust decoder, slicer, matcher and sport aliases compiled from current source.
It measures data-access inputs, rather than full SeaORM/HTTP/auth endpoints.
Current matching pages remain bounded. PostgreSQL memory is initially a shared
server snapshot, including file cache; Rust memory is a fresh-process peak and
retained RSS. Cold-cache, isolated per-arm/query PG peaks, complete renderer
work, additional heatmap zoom/filter/overview cases and write/WAL/lifecycle
costs remain required before adopting product spatial storage.

Both optimized arms prepare coordinates through the current Rust decoder and
preserve their floating-point bits in binary: ordinary PostgreSQL `bytea` for V1,
PostGIS geometry for P1. SQL-only decimal conversion failed exact output checks
in the initial experiment; the [notebook](../../experiments/postgis/notebook.md)
records that rejected result. Paged preparation has its own elapsed time,
PostgreSQL/Rust CPU and Rust process peak, separate from read observations.

## Observed source and synthetic dumps

Use a repeatable-read, read-only snapshot of the Rust `bike` database. The
2026-10-03 inventory has one activity owner, 1,189 activities dated 2018-10-29
through 2026-10-02, 32 segments, and 3,415 efforts. Source years contain
20, 117, 166, 201, 149, 151, 118, 140, and 127 activities respectively;
2018 and 2026 are partial years. These observed frequencies supersede an assumed
ten-rides-per-day generator for this evaluation.

The snapshot contains 4,854,345 route points; the median activity has 3,007 and
the p95 has 13,015, with a maximum of 35,797. Seventy-one activities have fewer
than two valid points. The descriptive region grouping contains 30 groups;
776 of the 1,118 activities with routes start in its busiest group. That grouping
captures geographic concentration without claiming each other group is a trip
or assuming the user has lived in only one region.

Prepare these two extension-neutral fixtures once and restore the same dump into
each comparison arm:

| Fixture             | Purpose                    | Construction                                                                                                                                                             |
| ------------------- | -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `history-1x.sql.gz` | Current personal history   | All observed activity routes/telemetry, sports, dates, segment geometries, and effort windows; synthetic IDs, names, titles, and source identifiers                      |
| `history-3x.sql.gz` | Larger-history sensitivity | Three calendar-year-shifted copies of that same history for the same owner, retaining the same physical segments and copying referenced efforts into each activity block |

The larger dump is a sensitivity case, not a forecast of future riding. Repeat
complete history blocks rather than distributing random routes uniformly across
the world or simply duplicating every ride on the same date. Preserve seasonality,
periods without riding, return visits to established paths, and sequences of
travel-region rides. Shift by the minimum whole-year interval that separates the
observed blocks; clamp February 29 to February 28 when necessary. Do not jitter
coordinates: identical geography deliberately stresses growing use of familiar
paths. Preserve each activity's original elapsed duration when shifting dates.
New-region scenarios use actual less-visited regions in the source.

The preparer retains complete route samples and point timestamps/distances, all
chart samples, laps, summary metrics, legacy sport values, segment routes, and
recorded effort boundaries/durations. It clears import/source links and replaces
identities rather than exporting authentication records or source files. Growth
stored ranks are cleared because copied efforts need a fresh ranking; they are
not a ranking correctness oracle. The fixture schema mirrors the live column
types and ordinary indexes on the three data tables. In particular, the live
route columns are `json`, not `jsonb`; do not silently change that baseline.

Dumps create only the new `postgis_eval` schema and a minimal synthetic rider
lookup. They are data-access fixtures, not a full restorable Bike application
database. Production auth, imports, tasks, training analysis, and analytics cache
tables are outside these dumps. A probe needing another lookup must supply the
same small deterministic lookup in every arm and report that scope; it must not
claim whole-endpoint measurements from an incomplete route-only query.

Full coordinates/telemetry remain local in Git-ignored compressed dumps and a
private workload selector. Commit only the generator, spec, and aggregate
manifests. Reuse the saved source snapshot for reproducibility; reading newer
history produces a new dataset fingerprint, not another run of the same dataset.

Each manifest records snapshot time, source revision/version/hash, dump hash and
compressed bytes, row/point counts, actual calendar range, sport/month/year
histograms, route/chart/segment point quantiles, route-free counts, and opaque
regional visit counts. Regions are a documented 80 km first-fit grouping of route
starts, not geocoded destinations. A non-home region transition is a descriptive
travel proxy, not an exact trip count. Preserve actual route geometry and use
per-route bounds for spatial workloads, not just those region labels.

Verify gzip integrity, SQL restore, exact row counts, mapped relationships,
valid effort windows or explicitly recorded source anomalies, and repeated-block
geometry/telemetry identity. Synthetic formatting changes affect compression:
measure restored storage and record the difference from source storage. A
compressed SQL dump's size is not PostgreSQL's physical storage size.

Three source efforts reference missing activities. Preserve them with synthetic
missing IDs, record expected orphan counts of three/nine in the 1x/3x fixtures,
and verify all remaining effort windows. Do not silently repair production or
drop those records to improve a benchmark. No non-orphan effort windows are
invalid in the prepared snapshot.

## Comparison arms: isolate what changes

| Arm | Database and data access                                                                                                                  | Purpose                                                                                 |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| V0  | Vanilla PostgreSQL; current Rust queries and complete JSON loading                                                                        | Actual existing baseline for segments, activity detail, map inputs, and race comparison |
| V1  | Vanilla PostgreSQL; import-prepared coordinate-only geometry, bounds/tile memberships and detail levels; unchanged Rust domain algorithms | Determines how much improvement comes from projections rather than PostGIS              |
| P0  | Identical ordinary queries/data/indexes with PostGIS merely enabled                                                                       | Negative control for extension overhead and nonspatial reads                            |
| P1  | PostGIS activity/segment geometry with spatial indexes; spatial candidate filtering; same Rust matching/aggregation/output                | Measures the extension's spatial data-access benefit against V1                         |

For proposed heatmaps, V0 has no existing implementation. Label its raw-history
query/aggregation prototype **H0**, never "current heatmap performance". Compare
it with vanilla projected H1 and PostGIS projected H2 using exactly the same Rust
coverage-mask aggregation and PNG encoder. A PostGIS/vector-tile alternative is
an optional separately labeled experiment, not a like-for-like replacement for
raster numbers.

Retain the common original JSON samples in all arms: PostGIS geometry is an
additional spatial projection, not a lossless replacement for timestamped
telemetry. P1's matching prefilter must fetch full ordered samples for surviving
candidates. Display-only geometry can be simplified independently. Compare
equivalent levels of detail and response precision; report simplified output as
a separate quality/cost tradeoff if it differs from the current full route.

Candidate filtering must be conservative. The current Rust segment matcher has
ordered endpoints, strict/fallback/reworked profiles, distance/shape checks, and
repeat-effort behavior. Spatial predicates may discard impossible candidates but
must not discard matches accepted by that matcher. Include the broadest existing
tolerance and simplification error in the prefilter. Preserve shared-segment
candidate scope: current activity matching considers segments belonging to other
owners; do not speed it up by incorrectly restricting them to the activity owner.

Use an indexed geography expression or an appropriate projected coordinate system
for meter-based proximity. A `geometry` distance in SRID 4326 is in degrees.
Do not cast/transform a stored column and assume its different index still
applies; capture the actual query plan. [PostGIS ST_DWithin](https://postgis.net/docs/ST_DWithin.html)
documents distance units and index-assisted filtering.

## Current access paths to reproduce

These source paths establish V0 at the recorded revision. Preserve their column
selection, batching, and scope before adding experimental projections:

| Feature                         | Current path                                                                                                                                                                             | Baseline behavior                                                                                                                                                    |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Activity detail                 | [`get_activity`](../../bike-rs/api/src/controllers/activities.rs)                                                                                                                        | Loads the owned activity row and full derived JSON, segment efforts, training analysis, and an optional original-source artifact lookup                              |
| Map image                       | [Next image proxy](../../bike-ui/app/activity-map-images/[variant]/[styleVersion]/route.ts), then [renderer](../../map-renderer/server.mjs)                                              | Fetches authorized activity detail, decodes it and strips telemetry to coordinates before calling the renderer; this still happens before a renderer image-cache hit |
| Race viewer                     | [`get_segment_comparison` / `load_effort_responses`](../../bike-rs/api/src/controllers/segments.rs)                                                                                      | Loads every effort for the selected segment, batches referenced activities with complete derived JSON and rider names, then decodes and slices effort routes in Rust |
| Activity-to-segment matching    | [`replace_segment_efforts_for_activity`](../../bike-rs/bike-core/src/segment_support.rs), [`segments::Model::matching_routes_page`](../../bike-rs/bike-core/src/entities/segments.rs)    | Reads all shared candidate segment routes in ID pages of 25; Rust endpoint/shape checks run after route materialization                                              |
| Segment-to-history regeneration | [`replace_segment_efforts_for_segment`](../../bike-rs/bike-core/src/segment_support.rs), [`activities::Model::matching_routes_page`](../../bike-rs/bike-core/src/entities/activities.rs) | Reads owned cycling activities' complete derived JSON in ID pages of 16 before Rust matching; analytics/cache rebuilds are separate workflow stages                  |
| Heatmaps                        | [Proposed product spec](heatmaps.md)                                                                                                                                                     | No current endpoint or query; H0/H1/H2 are explicitly experimental workloads                                                                                         |

The dump excludes training analysis and source-artifact tables. Detail probes
must report that limitation, or add equivalent deterministic auxiliary data to
all arms before claiming a whole-detail-response comparison. Preserve the
existing paged segment baseline rather than comparing against an invented
unbounded full-table materialization.

## Controlled execution environment

Use disposable local Linux containers, isolated from production and the existing
other test databases. All timed Rust probes use the same release build, toolchain,
target architecture, allocator, dependency lockfile, and source revision. Avoid
comparing macOS Rust RSS with Linux PostgreSQL/container RSS as a single ranking.

Pin image digests and exact PostgreSQL/PostGIS/GEOS/PROJ versions when building
the benchmark. For V0/V1/P0/P1, prefer identical PostgreSQL binaries and container
image with the extension enabled only in the PostGIS databases. Optionally repeat
V0 using the actual vanilla image as a deployment-cost control. Do not attribute
changes caused by different PostgreSQL minor versions or builds to PostGIS.

Record CPU model/count, container CPU/memory limits, Docker VM allocation, kernel,
filesystem/storage type, architecture, and free disk. Match `shared_buffers`,
`work_mem`, cache configuration, parallelism, JIT, planner settings, statistics,
connection pools, and extension instrumentation. Restore/index/analyze before
timing. Run arms sequentially, alternating their order across batches. Avoid
unrelated test/worker traffic during timed runs.

Start with one request/job at a time, matching ordinary use. A small four-request
viewport batch is useful for heatmap tiles and renderer queueing; broad load
testing and synthetic high concurrency are outside this decision. Preparation,
backfill, and index builds are separate measurements, never hidden inside warm
request timings.

Define three cache dimensions independently:

1. Database buffers/OS filesystem cache: warmed; fresh PostgreSQL buffers with
   potentially warm OS cache; truly cold filesystem only when controlled locally.
   Restarting PostgreSQL does not flush the OS page cache.
2. Rust/projection/decoded-route cache: disabled, then bounded and warmed if a
   proposed design includes one. Give both arms the same budget.
3. Image/tile cache and browser/basemap cache: explicit miss/hit cases. Keep
   renderer basemap bytes local or repeatable for controlled comparisons; report
   real network fetching separately.

For each selected case, use a short warmup and at least 100 sequential warm
observations split across alternating batches. Obtain at least 20 independent
database-buffer-cold observations when useful; show their individual values and
range rather than promising a stable p95 from a small sample. Record sample count,
median, p95 for adequate samples, max, and variation. Performance probes and
EXPLAIN collection run separately to avoid counting EXPLAIN overhead as a
normal request. Stop/reduce repetitions if a result already answers a narrow
question; extend only noisy or decision-critical cases.

## Workload matrix

Select actual fixture IDs and bounding boxes deterministically from the private
selector and store exact parameters in each result. Use both corpus sizes for
the core cases. Start with representative cases below rather than a combinatorial
matrix of every date, sport, zoom, and cache setting.

| Workload                   | Required cases                                                                                                                                  | Result to preserve                                                                                          |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| Activity detail            | Median-, p95-, and maximum-point activities; one route-free activity                                                                            | Owned scalar/telemetry/route response, full ordered points, same fields/precision                           |
| Map renderer inputs        | Same selected routes; 288x192 thumbnail and 1000x300 full image, DPR 1/2; one light/dark pair                                                   | Same coordinate payload/dimensions/style/attribution; compare input access independently of screenshot work |
| Race viewer/comparison     | Segment with most observed efforts; a sparse segment; current complete comparison scope and a separately labeled selected-effort probe          | Same effort IDs/order, rider lookup, route slices, elapsed times/distances and playback inputs              |
| Segment matching on import | Representative local, long/lapping, and travel-region rides matched against all eligible shared segment candidates                              | Exact existing matched efforts and boundaries, including direction/repeats                                  |
| Segment regeneration       | Busiest and travel-region segments searched against owned ride history; one no-match region                                                     | Same complete scoped matches; candidates rejected before route materialization are reported                 |
| Heatmap metadata           | All time, last observed 90 days, one complete year, road/MTB subtype aliases and all eligible sports                                            | Same ownership/filter eligibility/counts/bounds; no raw history sent to browser                             |
| Heatmap tile generation    | Busy home tile, home/travel boundary, infrequent travel tile, and empty tile at local zooms 12/16/18; one regional and one all-history overview | Same distinct-activity coverage field and palette/width treatment, including gutter/seams                   |

"Last 90 days" is anchored to the fixture's latest activity, not the wall clock,
including for the growth corpus. Choose heatmap geographic tiles from real route
coverage, not a random bounding box. At overview zoom, aggregate each activity's
coarse coverage once; summing fine cells would overcount a single route.

The source has only 32 distinct segments. Preserve that as the primary case; do
not multiply identical segment rows and present resulting spatial-index wins as
current behavior. If many-segment sensitivity becomes decision-critical, define
a separate route-derived segment corpus with its own manifest and label it as
additional synthetic scope.

Reuse current Rust algorithms and DTO shaping in the eventual probe wherever
possible. For race comparisons, distinguish the endpoint's current full effort
scope from a hypothetical narrowed selection; a frontend showing five riders
does not establish that the backend currently reads only five routes. For
activity detail, a faster coordinate-only map lookup must not substitute for the
full telemetry response and be called an equivalent endpoint.

## Stage-by-stage accounting

For every workload record this breakdown:

1. Connection/pool wait and request setup.
2. SQL planning/execution, rows inspected/returned, buffer hits/reads, TOAST
   access, temp spill, and server output serialization.
3. Database-to-Rust transfer bytes/time, row decoding, JSON/WKB decoding, and
   Rust objects/vertices materialized.
4. Domain work: segment prefilter/full matcher; race slicing/resampling; map
   coordinate preparation; heatmap mask union/count aggregation.
5. Response/PNG serialization, output bytes, and cache lookup/write.
6. When rendering: proxy/API round trips, renderer queue wait, Chromium/MapLibre
   work, screenshot/encoding, and return transfer.

Report both total elapsed time and CPU consumed by **each process group**:
PostgreSQL (including workers), Rust, Next proxy where measured, renderer Node,
and renderer Chromium children. A lower Rust CPU number with a larger database
CPU cost is not automatically a win. Sum CPU across groups for the same complete
operation, but do not sum overlapping wall-clock stage times into a fake total.

Capture paired trace/request IDs and raw per-stage observations in JSON Lines;
timestamps use a monotonic clock for durations. Count SQL calls and inspected/
returned activities, segments, efforts, route JSON bytes, and vertices. The
Next map-image proxy requests authorized activity detail before rendering; include
that fetch and its full-route materialization in the current map-input baseline.
The renderer's existing render/queue/cache metrics support the breakdown but are not
per-operation CPU or Chromium-memory measurements.

## Measurement definitions

### Database and Rust CPU

Measure CPU seconds with dedicated process/container CPU-counter deltas over an
isolated batch, then divide by successful operations. Include PostgreSQL parallel
workers and all renderer children. Use cgroup `cpu.stat` or equivalent Linux
process accounting; record user/system CPU when available. Subtract a measured
idle baseline only when its uncertainty is reported. Short operations may need a
batch for counter resolution.

`EXPLAIN ANALYZE` and `pg_stat_statements.total_exec_time` measure elapsed database
execution, not CPU time. Keep those fields separate. Optional `pg_stat_kcache`
requires the same instrumentation in all arms and is not required to start.
[PostgreSQL EXPLAIN](https://www.postgresql.org/docs/17/sql-explain.html) and
[pg_stat_statements](https://www.postgresql.org/docs/17/pgstatstatements.html)
define the SQL timing and buffer metrics used here.

Collect diagnostic plans with `EXPLAIN (ANALYZE, BUFFERS, SETTINGS, SERIALIZE TEXT,
FORMAT JSON)` using the same queries as the probes. Output serialization can
fetch additional TOAST data; default EXPLAIN skips that conversion. EXPLAIN does
not transfer rows to the client, so measure transfer and decoding with the real
Rust query separately. Keep instrumented plan collection outside ranked timing
batches.

### Memory

Measure Rust baseline, peak and retained resident memory, plus allocations/peak
live heap where a profiler is used. Record peak in fresh probe processes and
steady-state retained memory in reused processes separately. A lifetime high-water
mark on a reused process is not a per-request peak.

For PostgreSQL report total cgroup memory, anonymous/file-cache breakdown,
shared-buffer configuration, and backend/parallel-worker private or proportional
memory where available. Report sort/hash node memory and temp spill alongside
process measurements. PostgreSQL's `EXPLAIN (MEMORY)` describes **planner memory**,
not total execution/query/server memory; it cannot replace these measurements.
Avoid summing backend RSS, which double-counts shared mappings.

Sample container/process memory during a batch; state sample interval and the
possibility of missed short peaks. Record a cgroup peak when available. Renderer
memory includes Chromium subprocesses, not just Node. Do not describe a query as
bounded-memory merely because it returns a small PNG: measure candidate JSON,
Rust decoded arrays, per-activity masks, and renderer buffers.

### Physical storage and write costs

For each restored arm record:

- Empty database/extension footprint and final absolute database bytes.
- Common raw activity JSON/TOAST, segment JSON/TOAST, effort rows, and ordinary
  indexes as separate components.
- Added geometry, simplified levels, tile membership/coverage masks if present,
  GiST/geography indexes, metadata, and rendered caches.
- Data heap, TOAST, index bytes and bytes per activity/vertex; compressed logical
  dump bytes separately from physical relation bytes.
- Projection/backfill CPU, memory and elapsed time; index creation time/bytes;
  new ride insert, reprocess, delete, and invalidation latency plus WAL bytes.

Use `pg_relation_size`, `pg_table_size`, `pg_indexes_size`,
`pg_total_relation_size`, and `pg_database_size`. Include TOAST in totals once;
do not add it again to a value that already includes it. Restore fresh volumes
for initial storage, then measure post-mutation growth/bloat separately with the
same maintenance policy. [PostgreSQL size functions](https://www.postgresql.org/docs/17/functions-admin.html#FUNCTIONS-ADMIN-DBSIZE)
define what each value includes.

Do not promise that PostGIS saves storage if the design retains raw telemetry
and adds geometry/indexes. Compute the incremental cost and any JSON/display
projection savings independently. Include extension/image and backup/restore
costs in the operational comparison, not only the route-column size.

## Correctness before performance ranking

Compare candidate supersets and final output fingerprints against current Rust.
Segment results preserve ordered traversal, repeated effort boundaries, direction,
distance/shape tolerance, shared-segment visibility, and reworked-trail fallback.
The fixture's recorded efforts describe source state; freshly running current
Rust is the benchmark oracle if matcher versions have changed.

Race inputs preserve all timing/distance data; nearest-point geometry alone
cannot establish lap/time alignment. Display simplification never changes the
full samples used by matching or race playback. Heatmap masks count each activity
once per spatial cell, preserve arbitrary date/sport filtering and geography,
and agree between H1/H2 before colorizing. Geometry-only changes must retain
owner scoping, invalid/GPS-free handling, breaks, and antimeridian behavior.

Use existing focused fixtures for reverse direction/loops/GPS gaps/ownership
when the real snapshot lacks them. Keep those correctness fixtures separate from
the empirical workload distribution. Do not distort ride frequency or create
uniform travel patterns to cover a correctness case.

## Result artifacts and adoption decision

Store local raw runs under `experiments/postgis/results/<run-id>/`:
`environment.json`, `workloads.json`, `observations.jsonl`, `plans/*.json`,
`storage.json`, `correctness.json`, and a report. Commit a reviewed aggregate
report with corpus hashes/source revision and links to protected raw evidence.

Required comparison tables include:

| Workload/corpus/cache        | Arm                     | Samples      | Median/p95 elapsed          | PG/Rust/renderer CPU per operation | Rust/PG/renderer peak and retained MiB | Rows/vertices/bytes    | Correctness           |
| ---------------------------- | ----------------------- | ------------ | --------------------------- | ---------------------------------- | -------------------------------------- | ---------------------- | --------------------- |
| Populated by measured probes | V0/V1/P0/P1 or H0/H1/H2 | Actual count | Separate stage/total values | CPU seconds, not SQL elapsed       | Separate process/cache definitions     | Actual materialization | Pass/fail/unsupported |

| Corpus/arm                    | Common data + indexes | Projection + spatial index bytes | Empty/total DB bytes | Bytes per activity | Build/insert/reprocess/delete CPU and latency | Backup/cache bytes |
| ----------------------------- | --------------------- | -------------------------------- | -------------------- | ------------------ | --------------------------------------------- | ------------------ |
| Populated after restore/build | Measured              | Measured                         | Measured             | Measured           | Measured                                      | Measured           |

For each feature state whether a win comes from spatial filtering, projection,
reduced transfer/decoding, or renderer/cache changes. Report relative improvement
and absolute milliseconds/CPU/MiB/bytes. A near-zero improvement for activity
detail or race viewer is a valid finding; never substitute another feature's
spatial query results for those measurements.

Choose PostGIS if the measured spatial workloads improve enough to justify its
incremental storage, write and operational costs, with unchanged correctness and
no material regression in ordinary reads. A provisional meaningful improvement
is at least 20% less end-to-end elapsed or total CPU in a decision-critical spatial
case, exceeding run variation; a 1 ms saving alone is weak justification. Treat
that as a decision aid, not an automatic gate. Compare P1 to V1 as well as V0.

Complete measurements for all five requested feature families before authorizing
the product's spatial storage design. This does not require implementing the full
heatmap product, three backend ports, or a production rollout: isolated Rust data
access/aggregation probes and the existing renderer are sufficient to answer the
bounded questions, provided limitations are recorded.
