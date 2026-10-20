# PERF08: small API baseline

This baseline covers 10 owners, 1,000 activities, 10 owned segments and 1,980
repeated efforts. It measures the current application; it contains no application
optimization or capacity claim. The [raw results](PERF08-results.json) retain all
runs, the fixture fingerprint, source identities, image IDs and resource limits.

## Reproduce

Enter the existing [checks container](../development.md#checks-container), then:

```sh
mise run perf:build
mise run perf:small
mise run perf:small
```

`perf:build` uses the existing release API Bake target. `perf:small` runs the
existing migrations and shared Rust fixtures against its own PostgreSQL/API
stack, then removes that stack and its volumes. Generated output goes to
`.artifacts/performance/<project>/` inside the shared Docker disk. It never
seeds the developer database. Set `BIKE_API_IMAGE` to a previously built,
revision-labelled image to compare another revision against the same fixture.
The report records the actual runtime image ID.

## Dataset and controls

- Manifest version 1, seed 8, logical clock `2026-09-01T00:00:00Z`.
- Each owner has 100 short synthetic rides: 201 GPS points and 101 chart points
  per ride. Two traversals of the same climb occur on each of 99 rides. The
  remaining ride has no efforts. Durations include ties.
- Existing model fixtures provide defaults and the existing analytics builder
  computes all ranks and summaries. Assertions verify 198 efforts per segment,
  a 150-second best duration and the current ordinal tie-breaking behavior.
  This does not test worker scheduling, matching throughput or import capacity.
- Activity 1001 has zero efforts; 1002 has deliberately missing analytics/ranks;
  1003 has stale analytics. Their reads are checked for correctness outside the
  timed scenarios. Missing-rank fallback is existing application behavior, not
  a new fallback introduced by this work.
- Non-admin refresh-token sessions use ordinary application authentication.
  Anonymous requests and cross-owner activity/segment/comparison reads are
  rejected. Every measured response is checked for counts and geometry.
- Local Docker VM: ARM64, four CPUs, approximately 15.7 GiB RAM. API and database
  each have two CPUs and 1 GiB memory. PostgreSQL 17.11 uses a bounded 512 MiB
  tmpfs; the declared fixture budget is 128 MiB. This is a warm local baseline,
  not a production disk-I/O benchmark. No worker or browser is running.
- Release API source: `9139a674e0027a1817accfc09eb8ab796069797a` (PR 100's
  documentation branch; application code is unchanged from `a03e1c071`).
  The runner was uncommitted during measurement; its exact source checksum and
  dirty flag are recorded. Prepared projection timestamps come from the real
  builder; the fingerprint covers deterministic source fixture values.
- Ten warmups, then three batches of 200 requests for each scenario, at
  concurrency one. The first scenario request is recorded separately after
  authentication/availability checks; it does not mean cold PostgreSQL/OS caches.
  Seeding, migrations, startup and query plans are excluded from latency.

## Results

The table summarizes six warm batches per scenario across two fresh databases.
HTTP columns are ranges of batch percentiles. SQL time is the range of mean
PostgreSQL execution time per request, including authentication. SQL row counts
are returned/affected rows, not physical rows scanned.

| Scenario                         | HTTP p50 ms | HTTP p95 ms | SQL mean ms | SQL calls/request | SQL rows/request | Response bytes |
| -------------------------------- | ----------: | ----------: | ----------: | ----------------: | ---------------: | -------------: |
| Activity list (20 rows)          |   8.14–8.46 |   8.61–9.78 |   0.77–0.84 |                 5 |               43 |        104,319 |
| Activity detail (2 efforts)      |   1.62–1.73 |   1.79–2.36 | 0.094–0.109 |                 7 |                7 |         54,449 |
| Segment summary                  |   0.86–1.00 |   0.96–1.44 | 0.030–0.037 |                 5 |                5 |            290 |
| Segment comparison (198 efforts) | 39.32–39.73 | 42.24–43.08 |   3.58–3.67 |                 6 |              301 |      1,659,182 |

Both fresh seeds produced identical input fingerprints, response byte counts,
SQL calls and SQL row counts. All 4,800 measured requests passed correctness with
zero request errors/timeouts. These baseline runs compare the same revision;
small timing differences are run variation, not an optimization speedup.

The segment comparison downloads all 198 efforts and 10,098 effort GPS points.
Its 1.66 MB response and approximately 40 ms HTTP median are much larger than
its approximately 4 ms SQL execution time. The difference includes server work,
network transfer and pool waits; this measurement does not isolate serialization
or decoding. A low query count does not establish a cheap read.

The proposed 250 ms warm summary/list target is met by this small fixture.
Selected playback, long recordings and hundreds of concurrent users remain
unmeasured. No new latency gate is imposed from this baseline alone. Throughput
in the raw report includes client-side correctness validation and is not server
capacity. Process CPU, peak memory and individual decode/build/serialization
stages are explicitly unmeasured.

## Query plans and validation

The runner saves native `EXPLAIN (ANALYZE, BUFFERS)` plans for representative
activity-page and segment-effort reads after measurement. The first successful
run used `idx-activities-user-started-at` with a bounded incremental sort (20
returned rows, 13 shared-buffer hits, 0.065 ms execution). The effort plan used
`idx-segment-efforts-segment-id` plus an in-memory sort (198 returned rows, 12
shared-buffer hits, 0.119 ms execution). These are two illustrative statements,
not a complete endpoint plan inventory.

The fixture/percentile/correctness unit tests, two full disposable-stack runs,
Rust formatting, the existing Clippy configuration,
ShellCheck, and applicable commit hooks are the completion checks. Temporary
stacks are removed; requested evidence is retained in this directory.

## Review correction

The original version-1 stale fixture used an update hook that overwrote its old
timestamp. Those runs measured the four scenarios above, not stale-rank fallback.
Version 2 sets and verifies stale effort timestamps directly, measures missing,
stale, and empty details, and records returned states. Optional
`BIKE_PERF_RANK_GROWTH=1 mise run perf:small` repeats detail reads after growing
the selected segment from 198 to 1,926 efforts on other activities. This bounded
probe reuses the same runner; it is not capacity qualification.
