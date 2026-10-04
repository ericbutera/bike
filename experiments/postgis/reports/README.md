# Reviewed experiment evidence

EXP001 revision 3 ran 17 cases across V0 (current vanilla reads), V1 (binary
projections/bounds), P0 (extension enabled, unchanged reads) and P1 (geometry and
spatial filtering). Two fresh current-history runs and a separate 3x growth run
passed all 204 case/arm output checks and cleanup. The unchanged 1x rerun passed
controlled comparison, including cross-run output equality.

Each case/arm has **one observation per run**, with no per-case warmup. These
smoke measurements establish execution, output equality and measured resource
scope; they do not establish p95, product latency budgets or a performance ranking.

| Database storage     |    V0 |    V1 |    P0 |    P1 |
| -------------------- | ----: | ----: | ----: | ----: |
| Current history, MiB | 214.1 | 262.7 | 221.9 | 270.7 |
| 3x history, MiB      | 626.6 | 771.7 | 634.4 | 779.9 |

Coordinate projections reduced returned data in both optimized strategies. The
current-history home heatmap went from 1,033.1 MiB to 18.6 MiB with identical
PNG output; the 3x case went from 3,099.3 MiB to 55.9 MiB. PostGIS busy-history
matching exposed repeated spatial evaluation before every page of 16 activities;
the notebook records the plan evidence and a candidate-ID discovery follow-up.
The busy 3x race still loaded 514.0 MiB and peaked near 1,095.8 MiB of Rust RSS
across all strategies, which warrants a separate slice/read experiment.

Detailed tables and exact source/run identities:

- [Current-history data access, CPU, memory, stages and preparation](EXP001-rev3-history-1x.md)
- [Unchanged rerun variation](EXP001-rev3-rerun-comparison.md)
- [3x data access, CPU, memory, stages and preparation](EXP001-rev3-history-3x.md)
- [Aggregate machine-readable evidence](2026-10-03-harness.json)
- [Ideas, rejected results, interpretation and next tests](../notebook.md)

PG memory is a shared-server cgroup snapshot including file cache, rather than
isolated query/arm peaks. Rust peaks are fresh-process lifetime RSS. Probes use
the owning decoder/matcher with a synchronous PostgreSQL client and exclude full
SeaORM/HTTP/auth/renderer costs. Full renderer, isolated PG peaks, cold-cache,
additional filter/zoom cases and import/WAL/lifecycle measurements remain GEO02.

Private parameters, raw observations, plans and exact dirty-source snapshots stay
in ignored `results/<run-id>/`. Startup recovery hardening was verified separately
with the reusable cleanup check; see the notebook. No location/telemetry dump is
included in these aggregate artifacts.
