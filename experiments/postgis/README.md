# PostGIS evaluation

The [evaluation spec](../../docs/specs/postgis-evaluation.md) defines the controlled
comparison of current Rust/vanilla PostgreSQL, optimized vanilla projections,
extension-only controls, and Rust/PostGIS spatial access. It covers segments,
proposed heatmaps, activity detail, map-renderer inputs, and race comparisons.

The Compose harness restores fresh fixture databases, builds experimental
projections, runs release-mode Rust data-access probes, verifies equal outputs,
and writes Markdown/JSON reports. Each run is isolated and removes its database
volume, containers, and network on completion, failure, or handled interruption.
Build images/cache and local reports remain available; application and live data
are outside the harness.

## Run and compare

From this directory, with Docker running and the prepared fixtures present:

```sh
mise run experiment:smoke
mise run experiment:smoke
mise run experiment:compare -- results/<first-run> results/<second-run>
mise run experiment:cleanup-check
```

The smoke configuration uses the current-history corpus and one observation per
case/arm. It validates repeatability and correctness, with no performance ranking.
The [reviewed notebook](notebook.md) links the measured resource tables and
verified rerun comparison. The separately configured growth smoke uses the 3x
fixture:

```sh
mise run experiment -- --experiment experiments/EXP001-growth-smoke.json
```

The default pilot uses both corpora and ten observations per case/arm:

```sh
mise run experiment
```

`--experiment experiments/<config>.json` selects another versioned test. A config
can extend an existing test while changing one proposed parameter. Samples and
batches determine repetition; below 100 observations the report omits p95.
Batch order alternates V0/V1/P0/P1 and its reverse. All arms use the same PostgreSQL
binary/image, with PostGIS enabled only in P0/P1 databases.

Each run receives a unique Compose project and `results/<run-id>/` directory.
Fixtures are mounted nowhere inside database/build images; SQL is streamed into
fresh databases. The private selector fixes the actual activity/segment/tile
parameters. The harness has no published ports, external application credentials,
production database connection, or shared application network/volume.

Reports contain:

- `report.md` and `report.json`: scope, equal-output checks, timing/CPU/memory,
  materialized rows/vertices/bytes, storage, and bulk preparation observations.
- `experiment.json`, `ideas.json`, `environment.json`, `workloads.json`: exact
  hypothesis/configuration, source and fixture hashes, image/compiler versions,
  resource settings, and workload parameters.
- `run.json`: durable project/output identity written before Docker starts,
  including the image tag needed for recovery after an uncatchable interruption.
- `source-snapshot.json`: the measured source files, including uncommitted code,
  so an initial test can be reproduced after later edits.
- `observations.jsonl`, `batches/`, `plans/`, `storage.json`, `correctness.json`:
  individual values, fresh-process peaks, server counters, separate EXPLAIN plans,
  relation heap/TOAST/index sizes, and correctness fingerprints.
- `results/index.jsonl`: an append-only local index linking idea → test revision
  → run → report, including failed/interrupted runs.

The comparison command rejects failed runs, output changes, and differing
fixtures, workloads, sample settings, compiler, database image or environment.
Image comparisons preserve filesystem layers and runtime settings while ignoring
only Compose's unique project label; actual image IDs remain in the report.
It identifies source changes and writes `comparison.md` into the later run.
SQL + transfer includes client protocol/socket work; EXPLAIN separates server
execution/serialization. Rust CPU uses process counters and PG CPU uses cgroup
counter deltas, including server background work and counter-query overhead.
Datum bytes measure returned column data; query-parameter uploads and protocol
framing are excluded. PostgreSQL memory is a shared-server snapshot including file
cache, rather than a query peak. Renderer queue/Chromium/screenshot measurements,
cold-cache cases and write/WAL/lifecycle costs are subsequent experiments.

## Keep the learning loop reviewable

Define the idea and expected evidence in [ideas.json](ideas.json), then link it
from a versioned [experiment configuration](experiments/EXP001.json). Record the
change under test and explicit limitations. Preserve previous run directories;
the runner creates new reports rather than overwriting prior results. Capture
reviewed observations, interpretation, and the next test in the
[research notebook](notebook.md). Task completion stays in `docs/TODO.md`.

Normal cleanup runs automatically and Ctrl-C/SIGTERM preserve a partial report.
After an uncatchable process/host failure, use the exact project recorded in
`run.json` with `docker compose --project-name <recorded-project> --file
compose.yaml down --volumes --remove-orphans`; provide `POSTGIS_RUN_DIR` and
`POSTGIS_IMAGE_TAG` from that run's directory/project suffix. Cleanup targets only
that experiment project. Avoid running against a different project name.

Prepared and restore-verified on 2026-10-03:

| Fixture             | Activities | Route points | Compressed dump | Restored database |
| ------------------- | ---------: | -----------: | --------------: | ----------------: |
| `history-1x.sql.gz` |      1,189 |    4,854,345 |       126.9 MiB |         214.3 MiB |
| `history-3x.sql.gz` |      3,567 |   14,563,035 |       380.3 MiB |         626.8 MiB |

Both retain the same 32 physical segments. Restored sizes describe these
normalized fixtures on the recorded PostgreSQL build; they are not a vanilla
versus PostGIS storage comparison. See [verification](fixtures/verification.json)
for exact counts, relationship checks, version, limits, and relation sizes.

## Prepare the two dumps

From this directory, after checking `mise.toml`:

```sh
mise run fixtures:prepare
mise run fixtures:verify
```

The first preparation uses `kubectl` and the existing PostgreSQL pod to take a
repeatable-read, read-only snapshot of the Rust `bike` database's activity owner
1, segments, and that owner's efforts. `export.sql` is the complete source query;
it reads no auth records and writes nothing to that database. Further runs reuse
`fixtures/source.ndjson.gz`. Move that local source snapshot aside to deliberately
capture a newer history; the new fingerprint identifies different input data.

Outputs:

- `fixtures/history-1x.sql.gz`: observed history with synthetic identities.
- `fixtures/history-3x.sql.gz`: three time-shifted history blocks for the same rider,
  retaining the same physical segments; growth sensitivity, not a forecast.
- `fixtures/history-{1,3}x.manifest.json`: source/dump hashes, counts, calendar/sport
  distribution, vertex quantiles, and opaque regional visit counts.
- `fixtures/private-cases.json`: real geographic bounds and deterministic fixture
  IDs for selecting busy/travel/large-route workloads.

Full route samples, telemetry, relative timing, dates/sports, and recorded effort
windows remain representative of the source. Synthetic IDs/titles/names replace
identities; import/source links are cleared. JSON formatting is normalized.
Compressed dumps/source/private cases are Git-ignored; aggregate manifests remain
reviewable. These are **data-access fixtures**, not complete application backups.

The source contains 4,854,345 route points and 71 route-free/insufficient-route
activities. Thirty descriptive region groups capture its geographic concentration;
the largest contains 776 of 1,118 route-bearing activities. Three orphan effort
references are preserved and recorded, rather than silently removed.

## Restore in an isolated database

Restore the same extension-neutral dump into each empty comparison database,
never the live Bike database. It creates a new `postgis_eval` schema, without
drops, and preserves the live `json` column types and ordinary indexes.

For a disposable PostgreSQL container with an existing empty `postgis_eval_1x`
database:

```sh
gzip -dc fixtures/history-1x.sql.gz |
  docker exec -i <experiment-postgres-container> psql -X -U postgres \
    -d postgis_eval_1x -v ON_ERROR_STOP=1
```

Use `history-3x.sql.gz` and an empty `postgis_eval_3x` database for the larger
fixture. Later PostGIS setup builds projections in that disposable database;
it does not need a different activity dump. The spec requires identical raw data
and controlled PostgreSQL builds/settings before comparing performance.

Verify restored counts and relationships against the manifests. Both initial
restore checks passed and are reported in `fixtures/verification.json`;
storage observations from those checks do not establish PostGIS performance.

The existing restore verification task expects its own network-isolated container:

```sh
docker run --rm -d --name bike-postgis-fixture-check --network none \
  --cpus 2 --memory 2g -e POSTGRES_PASSWORD=fixture-only postgres:17
mise run fixtures:restore-check
docker stop bike-postgis-fixture-check
```

Wait for PostgreSQL readiness before the check. The task creates fresh named
fixture databases and refuses to overwrite them. Its `--rm` container removes
the temporary databases when stopped; the compressed fixtures and verification
record remain on disk. The image/version is recorded for validation, not pinned
as a future benchmark control. No ports are exposed.

## Probe scope

The probes cover owned detail inputs, coordinate-only map inputs, complete
segment race-comparison inputs, activity-to-segment matching, segment-to-history
matching, and home/other-region/empty/recent heatmap rasters. Preparation streams
pages through the owning Rust decoder to build binary coordinate/bounds or
geometry/GiST projections in disposable databases. V1 stores WKB in ordinary
`bytea`; P1 stores the identical coordinate bits in PostGIS geometry. This avoids
the decimal-conversion discrepancy found in an earlier SQL-only preparation.
Rust and PG preparation CPU, elapsed time and Rust peak RSS are recorded.

The owning Rust activity decoder, effort slicer, matcher and sport aliases are
compiled from current `bike-rs` source. Only ORM/OpenAPI derives are removed from
the experiment's generated module. Its existing focused tests run in the probe
build; changed module boundaries or unsafe proximity tolerances fail closed.
Current matching page sizes (25 segments / 16 activities) and shared segment
scope are preserved across all arms.
The read-only history probe omits the redundant selected owner ID (four returned
bytes per row), while retaining its ownership predicate. Its replay-input lookup
and segment lookup are included; effort writes and the complete worker workflow
are excluded. Resource limits come from resolved Compose configuration, with the
database's actual Docker limits checked and recorded after startup.

These are data-access probes using a synchronous PostgreSQL client. They do not
measure full SeaORM/HTTP/auth/Next endpoints. Detail fixtures omit training and
source-artifact tables; map probes stop before the existing renderer. Heatmaps
remain an explicitly labeled raster prototype. The spec's broader workload and
resource gates remain open until the corresponding measurements exist.
