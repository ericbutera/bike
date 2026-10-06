# PostGIS research notebook

This is a record of experimental reasoning and evidence. Project work/status
belongs in [docs/TODO.md](../../docs/TODO.md#active-work).

Each [idea](ideas.json) states a proposed explanation, a falsifiable comparison,
primary metrics, and output invariants. Each [test](experiments/EXP001.json)
records its ID/revision, change under test, hypothesis IDs, scope and parameters.
The runner captures every outcome with exact source/input/environment snapshots
and appends its run ID to the local `results/index.jsonl`.

When changing an experiment, increment its revision or create a new ID that
extends the previous config. Change one independent variable when practical.
Preserve earlier reports, including failures; describe why a test was corrected.
Compare compatible runs with the owning mise task before interpreting timing.
Different fixture/cache/precision/workload cases belong to different comparisons.

For each reviewed result record the following:

| Field          | Evidence                                                                        |
| -------------- | ------------------------------------------------------------------------------- |
| Idea and test  | Idea ID, experiment ID/revision, proposed change and predicted outcome          |
| Runs           | Run IDs, source revision/hash, fixture hashes and protected raw reports         |
| Observation    | Measured values and equal-output checks; explicit sampling/resource scope       |
| Interpretation | What the test supports or contradicts, including uncertainty                    |
| Next test      | One concrete change or missing measurement that resolves the remaining question |

Aggregate findings can be committed here or linked from a reviewed report;
private coordinates, telemetry and raw plans stay in ignored run directories.

## 2026-10-03: reject incorrect results before timing comparisons

EXP001 revisions 1–3 test IDEA001–003. The first two runs exposed problems in
the harness/preparation, rather than supporting a performance conclusion:

- `20261003T180143722Z-exp001-smoke-664284` failed regional case discovery:
  subtracting a negative longitude without parentheses created SQL `--`. All
  four databases had restored successfully; project cleanup passed. Revision 2
  corrected the selector and added a negative-coordinate regression.
- `20261003T181634412Z-exp001-smoke-5753cf` ran all 68 case/arm operations, then
  rejected three P1 outputs: median/p95 map coordinates and the other-region
  heatmap PNG. Other-region candidate rows and vertex counts agreed between
  optimized arms, while the coordinates differed. SQL decimal conversion and
  the current Rust JSON parser can produce different floating-point bits. A
  synthetic decimal reproduces the one-ULP difference in the owning probe test.
  Cleanup passed; timings remain invalid for ranking across the full experiment.
- Revision 3 prepares both optimized projections through the same current Rust
  decoder: vanilla stores WKB in `bytea`, PostGIS stores those coordinate bits in
  geometry. No rounding or tolerance is applied to the output checks. The two
  arms now share a binary coordinate representation, avoiding a JSON-versus-WKB
  decoder confound. Preparation also records Rust CPU and peak RSS.

Unique Compose project labels changed Docker image IDs even when every layer
and runtime setting was identical. Rerun compatibility therefore compares a
canonical image-content/runtime digest, ignoring only the project label, while
retaining actual IDs for traceability. A regression rejects changed filesystem
layers and environment settings. One sandbox-denied Buildx cache write produced
a preserved failure before database startup; the authorized Docker run was
retried with cache access.

The next test is an unchanged revision-3 rerun: all 17 cases must agree across
four arms and across runs before interpreting variation. Follow it with a
handled-interruption check during fixture restore and a separate 3x growth run.

## 2026-10-03: initial correct projection run

Run `20261003T182930592Z-exp001-smoke-b84088` used revision 3 and source digest
`2c068029c91e4adf07ed91f28f66b9c4f1e7d5f2a80672bd59d071c4d32e59f5`.
All 68 case/arm output checks passed; cleanup passed. One observation per
case/arm, post-restore state, no per-case warmup: timings describe this run and
cannot establish a ranking, percentile, product latency budget or adoption.

IDEA001 has useful materialization evidence. The 13,677-point map case retained
identical coordinates while returned datum bytes fell from 2,976.4 KiB in V0 to
213.7 KiB in either optimized arm. The home heatmap retained the identical PNG:
1,189 raw activities/4,854,345 vertices versus 132 candidates/1,220,700 vertices;
returned data fell from 1,057,898.7 KiB to 19,075.1 KiB. This shows why projections
deserve evaluation independently of the extension. Added database storage was
48.6 MiB for V1 and 56.6 MiB for P1, against a 214.1 MiB raw database.

IDEA002 does not imply that fewer candidates produce a faster query. Busy-segment
history returned 1,061 candidates in V0, 245 in V1 and 230 in P1, with equal
matched efforts. In this observation P1 spent 12,559.3 ms of PG CPU versus
210.8 ms in V1. The separately instrumented first-page plan used the geography
index, evaluated 238 candidate geometries, joined 230 surviving activities, then
sorted them before returning 16. Each subsequent keyset page repeats that
spatial evaluation. The first-page EXPLAIN execution was 768.6 ms for P1 versus
9.5 ms for V1, including separately requested text serialization; these plan
observations are outside the timing sample.

The next focused test should discover spatial candidate IDs once per matching
operation, then hydrate/process them in the existing pages of 16. Keep the Rust
matcher and effort fingerprints unchanged. A later independent test can compare
chunked geometry against full activity lines. This sequence separates repeated
search cost from geometry size and avoids attributing projection gains to PostGIS.

IDEA003 preserves ordinary detail/race inputs and their queries in all arms.
The busy race loaded 204 activities and about 171.3 MiB of returned data; Rust
process peaks were about 365 MiB in every arm. A separate race slice/projection
experiment is needed before claiming a benefit for that page. Extension enablement
alone added 7.8 MiB of database metadata; the common server image already contains
the extension libraries, so this is not an image-size comparison.

Interruption run `20261003T183600145Z-exp001-smoke-917519` received SIGINT during
the first fixture restore. It exited with an interrupted report and successful
cleanup. Independent Docker label queries found zero containers, volumes or
networks for its project. Other application test containers were preserved.

Unchanged rerun `20261003T183649774Z-exp001-smoke-8c59e8` passed all 68 checks
and cleanup. Its source digest, fixture, compiler, server settings, workload
parameters and image content matched the first correct run; actual image IDs
differed only through the run labels. The comparison command passed and verified
equal fingerprints across both runs. Reviewed [data-access/resource tables](reports/EXP001-rev3-history-1x.md),
[rerun variation](reports/EXP001-rev3-rerun-comparison.md) and
[aggregate JSON](reports/2026-10-03-harness.json) preserve this evidence without
private locations or telemetry. The single samples remain smoke measurements.

## 2026-10-03: growth and recovery evidence

Growth run `20261003T184220158Z-exp001-growth-smoke-38bb0b` used the same measured
source, compiler, server image/settings and four strategies against 3,567
activities/14,563,035 route points. All 68 checks and cleanup passed. The
[3x report](reports/EXP001-rev3-history-3x.md) and aggregate JSON retain CPU,
memory, stage and relation-size results. Added storage was 145.1 MiB in V1 and
153.3 MiB in P1, against a 626.6 MiB raw database.

The busy race loaded 612 activities/514.0 MiB of returned data and peaked near
1,095.8 MiB of Rust RSS in all four strategies. The unchanged race query therefore
needs its own focused read/slice experiment. Home heatmap data fell from
3,099.3 MiB raw to 55.9 MiB in either optimized strategy with equal PNG output.
Busy-history P1 used 103,845 ms of PG CPU in the one timed observation versus
670 ms in V1: increasing the corpus amplified the repeated spatial-search cost.
Discovering candidate IDs once remains the next proposed test, rather than a
reason to adopt or reject the extension from a smoke sample.

After those timed runs, bookkeeping was hardened without changing the Rust
workloads: `run.json` now records recovery identity before Docker starts, limits
come from resolved Compose configuration, and the actual database limits are
checked. A failed startup guard exposed Compose's memory-byte values as strings;
numeric normalization and a focused regression fixed it, with cleanup preserved.
The reusable `mise run experiment:cleanup-check` then passed as run
`20261003T190353521Z-exp001-smoke-a66fc5`, including durable identity, 2 CPU/3 GiB
actual limits, interrupted restore, and independent empty project-resource lists.
These bookkeeping changes have separate source snapshots from the timed runs;
their timings have not been substituted into the earlier reports.
