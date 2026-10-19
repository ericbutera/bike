# Operation tracing

- Trace expensive work at shared domain/service boundaries so HTTP requests,
  inline work, and Temporal activities get the same operation names.
- Keep operation spans at `INFO`, with a stable `bike.operation` string. Use IDs,
  input counts, and cache outcomes; skip automatic argument recording. Never
  attach credentials, file paths, GPS arrays, activity bodies, or provider bodies.
- Preserve the calling trace and parent across async work, blocking worker
  threads, and existing gRPC/HTTP propagation. Blocking workers must inherit both
  the current span and subscriber.
- Task orchestration and scheduling stay outside this contract. Instrument the
  reusable work so replacement Temporal tasks inherit it through normal calls.

## Operations

| Work | `bike.operation` | Useful context |
| --- | --- | --- |
| Heatmap projection rebuild | `heatmap.rebuild` | Activity ID, generation |
| Heatmap geometry preparation | `heatmap.geometry` | Route point count |
| Heatmap tile request and rendering | `heatmap.tile`, `heatmap.tile.render` | User/tile IDs, cache hit, chunk count |
| Heatmap metadata and zones | `heatmap.metadata`, `heatmap.zones` | User ID |
| Activity map image | `map.image` | User/activity IDs, variant/theme, cache hit |
| Map renderer wait and RPC | `map.render.wait`, `map.render` | Existing gRPC service/method/address fields |
| Activity achievements rebuild | `achievements.rebuild` | Requested activity count |
| Segment rankings/analytics | `segments.rebuild` | Requested segment count |
| Segment matching | `segments.match.activity`, `segments.match.segment` | IDs and route point count |
| Fitness/freshness rebuild | `fitness.rebuild` | User ID, input activity count, computed day count |
| Training analysis | `training.rebuild`, `training.compute`, `training.backfill` | Requested activity count, activity/user ID |
| Activity import and parsing | `activity.import`, `activity.import.process`, `activity.parse` | User/import ID, format, artifact kind, byte count |
| Scheduled Strava setup | `strava.sync.prepare` | Connections scanned, events prepared |
| Strava ownership validation | `strava.connection.validate` | Connection/user IDs |
| Existing Temporal Strava attempt | Registered activity name, e.g. `strava_rebuild_fitness` | Existing processor/event/execution fields |

- Keep the existing `bike.maps.snapshot` span name for map RPC compatibility;
  its operation field is `map.render`. Existing renderer server, browser, and
  screenshot spans remain descendants of that RPC.
- Export scheduled setup and validation as **Prepare scheduled Strava syncs**
  and **Validate Strava connection and customer**. Their operation fields remain
  stable even when the display names differ.
- `achievements.rebuild` replaces the less specific
  `activity.analytics.rebuild` span name.
- Dependency spans retain their existing provider, operation, RPC, and server
  fields so expensive application work can be separated from vendor latency.

## Database detail and N+1 visibility

- Keep SeaORM's semantic client spans. Remove duplicate raw ORM wrapper and
  SQLx implementation spans from export; logging remains controlled separately.
- Sample exported database detail per immediate owning span:
  - Up to three distinct normal SQL statements, identified without bind values.
  - Up to two calls taking at least 100 ms.
  - Up to two failed calls, with a separate budget from slow calls.
- Keep all instrumented database calls in the totals, including calls whose
  spans are omitted. Summaries roll up through local application parents:
  - `bike.db.calls`: call count, including transaction begin/commit/rollback.
  - `bike.db.duration_ms`: sum of call durations, including acquisition/waiting.
  - `bike.db.max_duration_ms`: longest call duration.
  - `bike.db.errors`: failed call count.
  - `bike.db.spans_kept`, `bike.db.spans_dropped`: detail retained/omitted.
- This is bounded diagnostic detail, not statistical sampling. Repeated fast SQL
  can produce one child span while its parent still reports 100 calls. Slow and
  failed samples are representative; the longest-call total remains exact even
  when that specific call is omitted.
- Summaries are attached when a parent closes. Retain in-flight state only for
  local parents; stop aggregation at remote parents and clear it on shutdown.
- Database durations are elapsed time, not PostgreSQL execution time or CPU
  time. Parallel call durations can overlap. Compare stage wall time, database
  totals, workload counts, and vendor spans before choosing an optimization.
- Parent totals include descendants; do not add parent and child totals together.
  Retained child spans are biased samples, so do not derive database throughput
  or latency percentiles from them. Summaries describe each retained trace;
  existing trace retention and sampling still apply.
- Keep idle polling excluded. These spans do not explain gaps between Temporal
  activities; use workflow history/runtime evidence for scheduling delays.

## Finding hotspots

- Slow heatmap rebuilds:

  ```traceql
  { span.bike.operation = "heatmap.rebuild" && span:duration > 500ms }
  ```

- Training rebuilds with many database calls, including omitted detail:

  ```traceql
  { span.bike.operation = "training.rebuild" && span.bike.db.calls > 50 }
  ```

- Slow map image cache misses:

  ```traceql
  { span.bike.operation = "map.image" && span.cache.hit = false && span:duration > 1s }
  ```

- Compare `map.render.wait` with `map.render` to separate local serialization
  from renderer RPC time. Expand retained children to inspect vendor work or SQL.
- Apply Grafana's selected time range and search the desired operation; historical
  traces keep their original names/detail until new code is deployed.
- Query syntax follows the [Tempo TraceQL reference](https://grafana.com/docs/tempo/latest/traceql/construct-traceql-queries/).

## Verification

- Use the production exporter policy with an in-memory exporter and SeaORM mocks.
- Use deterministic span fixtures for exact fast/slow sampling counts and the
  inclusive 100 ms boundary. ORM wiring tests retain real elapsed time and must
  honor separate fast/slow budgets when scheduling or CPU limits delay a query.
- Verify repeated-query suppression retains full counts and duration/error totals,
  independent normal/slow/error budgets, local ancestor aggregation, and cleanup
  at remote boundaries and shutdown.
- Verify named stages, query ancestry, workload fields, cache hit/miss behavior,
  and trace continuity through blocking heatmap rendering and map RPCs.
- Keep build/unit evidence separate from deployed trace verification.

## Local SQL diagnostics

- Development API containers enable SQLx query events at `DEBUG`; runtime/E2E
  defaults and Temporal worker logging retain their existing filters.
- `mise run db:queries` follows API query events from the current Compose project.
  Show statement text, elapsed milliseconds, returned/affected rows, log level,
  HTTP method/route, trace ID, operation, and activity ID when available.
- Keep each executed statement visible, including repetitions, so developers can
  identify N+1 patterns. This stream is independent of exported span sampling.
- `--activities` restricts output to requests under `/api/activities`, including
  authentication queries before the controller starts. `--min-ms` filters by
  elapsed milliseconds; `--tail` includes recent raw log lines before following.
- Use SQLx's existing structured events rather than adding query instrumentation
  or logging bound parameters. Prepared SQL retains its placeholders. Elapsed
  time includes client execution and result transfer; use PostgreSQL
  `EXPLAIN (ANALYZE, BUFFERS)` on a selected read query for a server execution plan.
- Recreate only the development API after changing its logging environment;
  preserve PostgreSQL, Temporal, uploads, and their persistent volumes.
- Verify the formatter with fixture events covering list/detail requests,
  repeated statements, slow statements, missing context, and output filters.
