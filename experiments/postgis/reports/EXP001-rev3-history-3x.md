# EXP001-GROWTH-SMOKE revision 3: Threefold history growth: harness correctness and resource smoke check

Reviewed aggregate copy. Private raw files and the exact measured dirty-source snapshot remain in ignored `results/20261003T184220158Z-exp001-growth-smoke-38bb0b/`.

Run: `20261003T184220158Z-exp001-growth-smoke-38bb0b`; outcome: **passed**.

Ideas: IDEA001, IDEA002, IDEA003. Change under test: Precision correction: prepare V1 binary coordinate/bounds and P1 geometry projections through the owning Rust decoder; PostGIS endpoint candidate filtering.

Source: `e640932586f21257bcaf31b08af6abc8164dfce1`; code fingerprint: `2c068029c91e4adf07ed91f28f66b9c4f1e7d5f2a80672bd59d071c4d32e59f5`.

Images: PostgreSQL `sha256:3d70c6d528563d54d70ac2f6f6d4759e2ef4cae66bb0b14fec4b879385a0ba7a`; Rust probe `sha256:b18fb93d242a98e56b0fcd95cee4dc3d19c6886e56ce84b026251b021accec4d`.

Rust toolchain: rustc 1.93.1 (01f6ddf75 2026-02-11); aarch64-unknown-linux-gnu.

Cache: post-restore state; no per-case warmup; fresh Rust process; no application cache. Warmups: 0; observations per case/arm: 1.

## Scope and measurement limits

- Pilot sampling: report actual observation counts and median/range; no p95 adoption claim below 100 observations.
- Data-access probes use the synchronous postgres driver with Bike's owning decoder/matcher; SeaORM pool, HTTP/auth and complete endpoint DTO costs are excluded.
- Both optimized arms preserve current Rust-decoded coordinate bits through WKB; SQL-only JSON-to-geometry preparation failed output equality and was rejected.
- Detail excludes training-analysis/source-artifact lookups, which are absent from these data-access fixtures.
- Median/p95 detail and map cases are selected among route-bearing activities; manifests' global quantiles also include route-free activities.
- Map input only: proxy HTTP/auth, renderer queue/Chromium/basemap/screenshot costs remain unmeasured.
- Heatmaps are a coordinate-gap-aware raster prototype; gaps over approximately 5 km break lines, with temporal-gap policy, overview/LOD, additional zooms/sport filters and tile caches still unevaluated.
- No cold-cache, concurrency, write/WAL or backfill-lifecycle measurements in this initial experiment.
- One observation per case/arm: validates the harness, never establishes a performance ranking.
- SQL/transfer includes client protocol decoding and socket wait; separate EXPLAIN plans describe server execution/serialization outside ranked observations.
- Payload bytes count PostgreSQL binary datum bytes, excluding protocol framing and network headers.
- Rust CPU uses getrusage. PG CPU uses isolated server cgroup counter deltas per batch, including counter-query overhead; it is not SQL elapsed time.
- Rust peak RSS is a fresh process lifetime peak per case/batch, including warmup/connection setup. PG memory is a shared server cgroup snapshot, including file cache; it is not per-query peak memory.
- All four arm databases share one otherwise idle server. Its shared buffer/file-cache state and lifetime memory peak must not be interpreted as isolated per-arm memory.
- Private raw observations, parameters and plans remain in this run directory. This pilot does not authorize a product storage decision.

## Data-access results

| Corpus     | Case                 | Arm |   n | Median ms | p95 ms | Range ms            | PG CPU ms/op | Rust CPU ms/op | Rust peak/retained MiB | PG snapshot MiB | Datum KiB | Candidates | Vertices | Output equal |
| ---------- | -------------------- | --- | --: | --------: | -----: | ------------------- | -----------: | -------------: | ---------------------- | --------------: | --------: | ---------: | -------: | ------------ |
| history-3x | detail-maximum       | P0  |   1 |    132.89 |      — | 132.89–132.89       |         9.15 |         124.03 | 53.4/10.8              |          1170.8 |    7235.5 |          1 |    35797 | yes          |
| history-3x | detail-maximum       | P1  |   1 |    132.43 |      — | 132.43–132.43       |         7.79 |         124.89 | 53.5/10.8              |          1170.8 |    7235.5 |          1 |    35797 | yes          |
| history-3x | detail-maximum       | V0  |   1 |    135.11 |      — | 135.11–135.11       |         9.38 |         125.23 | 53.2/10.8              |          1163.1 |    7235.5 |          1 |    35797 | yes          |
| history-3x | detail-maximum       | V1  |   1 |    133.81 |      — | 133.81–133.81       |         9.64 |         124.34 | 53.4/10.8              |          1167.0 |    7235.5 |          1 |    35797 | yes          |
| history-3x | detail-median        | P0  |   1 |     14.87 |      — | 14.87–14.87         |         3.60 |          11.40 | 10.7/4.5               |          1152.6 |     735.3 |          1 |     3254 | yes          |
| history-3x | detail-median        | P1  |   1 |     14.22 |      — | 14.22–14.22         |         2.65 |          11.64 | 10.8/4.5               |          1153.6 |     735.3 |          1 |     3254 | yes          |
| history-3x | detail-median        | V0  |   1 |     15.17 |      — | 15.17–15.17         |         3.64 |          11.59 | 10.6/4.5               |          1146.1 |     735.3 |          1 |     3254 | yes          |
| history-3x | detail-median        | V1  |   1 |     15.75 |      — | 15.75–15.75         |         3.47 |          12.04 | 10.4/4.5               |          1149.4 |     735.3 |          1 |     3254 | yes          |
| history-3x | detail-p95           | P0  |   1 |     53.54 |      — | 53.54–53.54         |         5.16 |          48.55 | 22.6/6.6               |          1158.0 |    2976.6 |          1 |    13677 | yes          |
| history-3x | detail-p95           | P1  |   1 |     54.69 |      — | 54.69–54.69         |         4.27 |          50.54 | 22.6/6.6               |          1158.5 |    2976.6 |          1 |    13677 | yes          |
| history-3x | detail-p95           | V0  |   1 |     52.88 |      — | 52.88–52.88         |         4.13 |          49.05 | 22.8/6.6               |          1154.4 |    2976.6 |          1 |    13677 | yes          |
| history-3x | detail-p95           | V1  |   1 |     54.54 |      — | 54.54–54.54         |         5.11 |          49.58 | 22.8/6.6               |          1156.1 |    2976.6 |          1 |    13677 | yes          |
| history-3x | detail-route-free    | P0  |   1 |      1.16 |      — | 1.16–1.16           |         0.89 |           0.35 | 10.6/3.6               |          1173.3 |      28.8 |          1 |        0 | yes          |
| history-3x | detail-route-free    | P1  |   1 |      1.68 |      — | 1.68–1.68           |         1.40 |           0.46 | 10.6/3.6               |          1173.8 |      28.8 |          1 |        0 | yes          |
| history-3x | detail-route-free    | V0  |   1 |      1.21 |      — | 1.21–1.21           |         0.92 |           0.36 | 10.6/3.6               |          1173.5 |      28.8 |          1 |        0 | yes          |
| history-3x | detail-route-free    | V1  |   1 |      1.11 |      — | 1.11–1.11           |         0.93 |           0.32 | 10.4/3.6               |          1173.5 |      28.8 |          1 |        0 | yes          |
| history-3x | heatmap-empty        | P0  |   1 |  20101.14 |      — | 20101.14–20101.14   |      2832.96 |       17632.68 | 277.7/266.3            |          1082.1 | 3173696.1 |       3567 | 14563035 | yes          |
| history-3x | heatmap-empty        | P1  |   1 |     31.97 |      — | 31.97–31.97         |        31.35 |           0.37 | 10.7/3.4               |           323.9 |       0.0 |          0 |        0 | yes          |
| history-3x | heatmap-empty        | V0  |   1 |  20159.62 |      — | 20159.62–20159.62   |      2852.39 |       17758.47 | 277.4/253.2            |          1257.8 | 3173696.1 |       3567 | 14563035 | yes          |
| history-3x | heatmap-empty        | V1  |   1 |      1.75 |      — | 1.75–1.75           |         1.60 |           0.21 | 10.9/3.4               |           449.9 |       0.0 |          0 |        0 | yes          |
| history-3x | heatmap-home-90days  | P0  |   1 |    465.19 |      — | 465.19–465.19       |        70.43 |         427.12 | 62.7/48.2              |           352.2 |   69254.6 |         37 |   346035 | yes          |
| history-3x | heatmap-home-90days  | P1  |   1 |     24.67 |      — | 24.67–24.67         |        13.85 |          11.04 | 11.0/4.1               |           347.0 |    1424.4 |          5 |    91158 | yes          |
| history-3x | heatmap-home-90days  | V0  |   1 |    470.07 |      — | 470.07–470.07       |        57.44 |         426.44 | 62.6/48.2              |           344.1 |   69254.6 |         37 |   346035 | yes          |
| history-3x | heatmap-home-90days  | V1  |   1 |     17.43 |      — | 17.43–17.43         |         6.22 |          11.41 | 10.8/4.1               |           340.5 |    1424.4 |          5 |    91158 | yes          |
| history-3x | heatmap-home         | P0  |   1 |  20572.26 |      — | 20572.26–20572.26   |      2961.11 |       18106.54 | 275.9/276.3            |          1538.2 | 3173696.1 |       3567 | 14563035 | yes          |
| history-3x | heatmap-home         | P1  |   1 |    634.69 |      — | 634.69–634.69       |       178.56 |         466.63 | 77.8/8.8               |           861.0 |   57225.3 |        396 |  3662100 | yes          |
| history-3x | heatmap-home         | V0  |   1 |  20641.95 |      — | 20641.95–20641.95   |      2947.45 |       18184.99 | 275.6/269.7            |           937.3 | 3173696.1 |       3567 | 14563035 | yes          |
| history-3x | heatmap-home         | V1  |   1 |    536.28 |      — | 536.28–536.28       |       111.82 |         442.71 | 77.7/8.6               |           906.9 |   57225.3 |        396 |  3662100 | yes          |
| history-3x | heatmap-other-region | P0  |   1 |  20484.40 |      — | 20484.40–20484.40   |      2957.73 |       17994.12 | 276.2/276.5            |          1354.8 | 3173696.1 |       3567 | 14563035 | yes          |
| history-3x | heatmap-other-region | P1  |   1 |    170.11 |      — | 170.11–170.11       |        66.12 |         103.77 | 21.7/21.7              |           627.2 |   14288.0 |        276 |   914205 | yes          |
| history-3x | heatmap-other-region | V0  |   1 |  20305.55 |      — | 20305.55–20305.55   |      2877.66 |       17872.73 | 278.1/278.6            |          1491.5 | 3173696.1 |       3567 | 14563035 | yes          |
| history-3x | heatmap-other-region | V1  |   1 |    167.87 |      — | 167.87–167.87       |        67.36 |         112.78 | 21.9/21.5              |           722.7 |   14288.0 |        276 |   914205 | yes          |
| history-3x | map-input-maximum    | P0  |   1 |     54.60 |      — | 54.60–54.60         |         6.34 |          48.52 | 46.4/16.2              |          1172.2 |    7235.5 |          1 |    35797 | yes          |
| history-3x | map-input-maximum    | P1  |   1 |     14.55 |      — | 14.55–14.55         |         9.25 |           5.55 | 11.1/7.2               |          1174.3 |     559.3 |          1 |    35797 | yes          |
| history-3x | map-input-maximum    | V0  |   1 |     57.71 |      — | 57.71–57.71         |        32.72 |          51.20 | 46.4/16.2              |          1170.8 |    7235.5 |          1 |    35797 | yes          |
| history-3x | map-input-maximum    | V1  |   1 |      7.35 |      — | 7.35–7.35           |         2.44 |           5.14 | 10.5/7.2               |          1172.3 |     559.3 |          1 |    35797 | yes          |
| history-3x | map-input-median     | P0  |   1 |      7.30 |      — | 7.30–7.30           |         1.46 |           4.78 | 10.9/4.8               |          1153.8 |     735.3 |          1 |     3254 | yes          |
| history-3x | map-input-median     | P1  |   1 |      9.63 |      — | 9.63–9.63           |         8.92 |           0.80 | 10.7/3.9               |          1155.4 |      50.9 |          1 |     3254 | yes          |
| history-3x | map-input-median     | V0  |   1 |      6.14 |      — | 6.14–6.14           |         1.51 |           4.86 | 10.9/4.8               |          1153.3 |     735.3 |          1 |     3254 | yes          |
| history-3x | map-input-median     | V1  |   1 |      2.50 |      — | 2.50–2.50           |         1.84 |           0.85 | 10.7/3.9               |          1153.8 |      50.9 |          1 |     3254 | yes          |
| history-3x | map-input-p95        | P0  |   1 |     23.38 |      — | 23.38–23.38         |         3.71 |          19.85 | 20.0/8.8               |          1158.8 |    2976.4 |          1 |    13677 | yes          |
| history-3x | map-input-p95        | P1  |   1 |     11.68 |      — | 11.68–11.68         |         9.14 |           2.76 | 10.7/4.9               |          1160.8 |     213.7 |          1 |    13677 | yes          |
| history-3x | map-input-p95        | V0  |   1 |     22.32 |      — | 22.32–22.32         |         3.52 |          19.14 | 20.0/8.7               |          1158.0 |    2976.4 |          1 |    13677 | yes          |
| history-3x | map-input-p95        | V1  |   1 |      4.84 |      — | 4.84–4.84           |         2.10 |           2.95 | 10.7/4.9               |          1159.0 |     213.7 |          1 |    13677 | yes          |
| history-3x | match-local          | P0  |   1 |     15.38 |      — | 15.38–15.38         |         3.48 |          12.05 | 10.7/5.1               |          1006.7 |    1611.3 |         32 |    11180 | yes          |
| history-3x | match-local          | P1  |   1 |     23.96 |      — | 23.96–23.96         |        19.37 |           4.65 | 10.3/5.0               |          1032.8 |     735.1 |          0 |     3254 | yes          |
| history-3x | match-local          | V0  |   1 |     17.71 |      — | 17.71–17.71         |         4.48 |          12.77 | 10.6/5.1               |          1003.9 |    1611.3 |         32 |    11180 | yes          |
| history-3x | match-local          | V1  |   1 |      6.81 |      — | 6.81–6.81           |         2.73 |           4.23 | 10.7/5.0               |          1004.8 |     735.1 |          0 |     3254 | yes          |
| history-3x | match-long           | P0  |   1 |    110.19 |      — | 110.19–110.19       |        10.57 |          98.72 | 46.4/15.8              |          1039.3 |    8111.4 |         32 |    43723 | yes          |
| history-3x | match-long           | P1  |   1 |     73.66 |      — | 73.66–73.66         |        26.81 |          46.73 | 46.5/16.4              |          1048.6 |    7235.2 |          0 |    35797 | yes          |
| history-3x | match-long           | V0  |   1 |    112.52 |      — | 112.52–112.52       |        12.39 |         100.33 | 46.5/15.8              |          1034.9 |    8111.4 |         32 |    43723 | yes          |
| history-3x | match-long           | V1  |   1 |     55.51 |      — | 55.51–55.51         |         9.75 |          45.90 | 46.4/14.3              |          1038.3 |    7235.2 |          0 |    35797 | yes          |
| history-3x | match-other-region   | P0  |   1 |     17.40 |      — | 17.40–17.40         |         3.90 |          13.66 | 11.0/5.2               |          1044.5 |    1680.3 |         32 |    11542 | yes          |
| history-3x | match-other-region   | P1  |   1 |     16.39 |      — | 16.39–16.39         |        11.88 |           4.68 | 10.4/5.1               |          1047.9 |     804.1 |          0 |     3616 | yes          |
| history-3x | match-other-region   | V0  |   1 |     17.98 |      — | 17.98–17.98         |         4.37 |          13.79 | 10.7/5.2               |          1041.2 |    1680.3 |         32 |    11542 | yes          |
| history-3x | match-other-region   | V1  |   1 |      7.89 |      — | 7.89–7.89           |         3.16 |           4.79 | 10.4/5.1               |          1042.3 |     804.1 |          0 |     3616 | yes          |
| history-3x | race-busy            | P0  |   1 |   3974.28 |      — | 3974.28–3974.28     |       837.29 |        3352.66 | 1095.8/309.0           |           618.3 |  526381.5 |        612 |  5956974 | yes          |
| history-3x | race-busy            | P1  |   1 |   3641.89 |      — | 3641.89–3641.89     |       554.70 |        3209.34 | 1095.8/309.0           |           937.7 |  526381.5 |        612 |  5956974 | yes          |
| history-3x | race-busy            | V0  |   1 |   3716.58 |      — | 3716.58–3716.58     |       549.03 |        3438.77 | 1095.8/309.0           |          1440.4 |  526381.5 |        612 |  5956974 | yes          |
| history-3x | race-busy            | V1  |   1 |   3612.68 |      — | 3612.68–3612.68     |       522.23 |        3267.54 | 1095.9/309.2           |          1756.2 |  526381.5 |        612 |  5956974 | yes          |
| history-3x | race-sparse          | P0  |   1 |    151.60 |      — | 151.60–151.60       |        26.06 |         129.48 | 53.2/14.3              |           992.0 |   20835.4 |         12 |    95829 | yes          |
| history-3x | race-sparse          | P1  |   1 |    156.38 |      — | 156.38–156.38       |        25.40 |         132.32 | 53.1/14.3              |          1006.3 |   20835.4 |         12 |    95829 | yes          |
| history-3x | race-sparse          | V0  |   1 |    189.26 |      — | 189.26–189.26       |        61.12 |         137.35 | 53.2/14.3              |           957.7 |   20835.4 |         12 |    95829 | yes          |
| history-3x | race-sparse          | V1  |   1 |    157.31 |      — | 157.31–157.31       |        27.86 |         129.84 | 53.2/14.3              |           977.6 |   20835.4 |         12 |    95829 | yes          |
| history-3x | segment-history-busy | P0  |   1 |  20132.20 |      — | 20132.20–20132.20   |      3031.19 |       17592.54 | 75.0/59.9              |           531.4 | 2968697.0 |       3183 | 13622919 | yes          |
| history-3x | segment-history-busy | P1  |   1 | 107274.56 |      — | 107274.56–107274.56 |    103845.05 |        4756.48 | 53.6/29.1              |           298.0 |  603861.9 |        690 |  2805975 | yes          |
| history-3x | segment-history-busy | V0  |   1 |  20052.18 |      — | 20052.18–20052.18   |      2968.91 |       17642.83 | 77.0/54.0              |           356.0 | 2968697.0 |       3183 | 13622919 | yes          |
| history-3x | segment-history-busy | V1  |   1 |   5376.63 |      — | 5376.63–5376.63     |       669.65 |        4876.19 | 55.2/33.7              |          1279.0 |  624654.5 |        735 |  2895942 | yes          |

## Stage medians

| Corpus     | Case                 | Arm | SQL + transfer ms | Decode ms | Domain ms | Output ms | SQL calls |
| ---------- | -------------------- | --- | ----------------: | --------: | --------: | --------: | --------: |
| history-3x | detail-maximum       | P0  |             10.73 |     41.95 |      0.00 |     29.14 |         2 |
| history-3x | detail-maximum       | P1  |              9.63 |     43.11 |      0.00 |     29.78 |         2 |
| history-3x | detail-maximum       | V0  |             10.15 |     45.12 |      0.00 |     29.62 |         2 |
| history-3x | detail-maximum       | V1  |             11.37 |     42.82 |      0.00 |     29.63 |         2 |
| history-3x | detail-median        | P0  |              3.73 |      4.05 |      0.00 |      2.68 |         2 |
| history-3x | detail-median        | P1  |              2.63 |      3.86 |      0.00 |      2.79 |         2 |
| history-3x | detail-median        | V0  |              3.62 |      3.75 |      0.00 |      2.93 |         2 |
| history-3x | detail-median        | V1  |              3.88 |      4.07 |      0.00 |      3.09 |         2 |
| history-3x | detail-p95           | P0  |              5.75 |     16.32 |      0.00 |     11.36 |         2 |
| history-3x | detail-p95           | P1  |              4.57 |     18.41 |      0.00 |     11.46 |         2 |
| history-3x | detail-p95           | V0  |              4.49 |     16.76 |      0.00 |     11.80 |         2 |
| history-3x | detail-p95           | V1  |              5.39 |     17.02 |      0.00 |     12.17 |         2 |
| history-3x | detail-route-free    | P0  |              0.87 |      0.19 |      0.00 |      0.00 |         2 |
| history-3x | detail-route-free    | P1  |              1.36 |      0.23 |      0.00 |      0.00 |         2 |
| history-3x | detail-route-free    | V0  |              0.92 |      0.18 |      0.00 |      0.00 |         2 |
| history-3x | detail-route-free    | V1  |              0.85 |      0.17 |      0.00 |      0.00 |         2 |
| history-3x | heatmap-empty        | P0  |           3166.34 |  15355.30 |   1578.81 |      0.00 |       224 |
| history-3x | heatmap-empty        | P1  |             31.73 |      0.00 |      0.22 |      0.00 |         1 |
| history-3x | heatmap-empty        | V0  |           3141.24 |  15459.29 |   1555.86 |      0.00 |       224 |
| history-3x | heatmap-empty        | V1  |              1.57 |      0.00 |      0.16 |      0.00 |         1 |
| history-3x | heatmap-home-90days  | P0  |             55.78 |    370.22 |     39.17 |      0.00 |         4 |
| history-3x | heatmap-home-90days  | P1  |             14.14 |      0.12 |     10.37 |      0.00 |         1 |
| history-3x | heatmap-home-90days  | V0  |             61.06 |    370.22 |     38.77 |      0.00 |         4 |
| history-3x | heatmap-home-90days  | V1  |              6.47 |      0.11 |     10.80 |      0.00 |         1 |
| history-3x | heatmap-home         | P0  |           3254.73 |  15621.81 |   1694.21 |      0.00 |       224 |
| history-3x | heatmap-home         | P1  |            183.99 |      5.10 |    444.26 |      0.00 |         1 |
| history-3x | heatmap-home         | V0  |           3249.54 |  15637.41 |   1754.08 |      0.00 |       224 |
| history-3x | heatmap-home         | V1  |            114.28 |      5.79 |    414.69 |      0.00 |         1 |
| history-3x | heatmap-other-region | P0  |           3274.17 |  15631.65 |   1577.09 |      0.00 |       224 |
| history-3x | heatmap-other-region | P1  |             72.49 |      0.56 |     97.01 |      0.00 |         1 |
| history-3x | heatmap-other-region | V0  |           3181.82 |  15530.12 |   1592.82 |      0.00 |       224 |
| history-3x | heatmap-other-region | V1  |             68.10 |      0.63 |     99.05 |      0.00 |         1 |
| history-3x | map-input-maximum    | P0  |              7.60 |     42.20 |      0.00 |      3.39 |         1 |
| history-3x | map-input-maximum    | P1  |              9.23 |      0.03 |      0.00 |      3.71 |         1 |
| history-3x | map-input-maximum    | V0  |              8.27 |     44.28 |      0.00 |      3.65 |         1 |
| history-3x | map-input-maximum    | V1  |              2.43 |      0.04 |      0.00 |      3.39 |         1 |
| history-3x | map-input-median     | P0  |              1.33 |      5.37 |      0.00 |      0.47 |         1 |
| history-3x | map-input-median     | P1  |              8.94 |      0.01 |      0.00 |      0.52 |         1 |
| history-3x | map-input-median     | V0  |              1.53 |      3.99 |      0.00 |      0.49 |         1 |
| history-3x | map-input-median     | V1  |              1.72 |      0.01 |      0.00 |      0.54 |         1 |
| history-3x | map-input-p95        | P0  |              3.99 |     16.78 |      0.00 |      2.06 |         1 |
| history-3x | map-input-p95        | P1  |              9.09 |      0.02 |      0.00 |      1.98 |         1 |
| history-3x | map-input-p95        | V0  |              3.68 |     15.94 |      0.00 |      2.09 |         1 |
| history-3x | map-input-p95        | V1  |              2.01 |      0.02 |      0.00 |      2.07 |         1 |
| history-3x | match-local          | P0  |              3.87 |      7.02 |      4.43 |      0.00 |         4 |
| history-3x | match-local          | P1  |             19.86 |      4.06 |      0.00 |      0.00 |         2 |
| history-3x | match-local          | V0  |              5.34 |      7.50 |      4.54 |      0.00 |         4 |
| history-3x | match-local          | V1  |              2.86 |      3.92 |      0.00 |      0.00 |         2 |
| history-3x | match-long           | P0  |             13.02 |     48.79 |     48.16 |      0.00 |         4 |
| history-3x | match-long           | P1  |             29.20 |     44.10 |      0.00 |      0.00 |         2 |
| history-3x | match-long           | V0  |             15.04 |     50.65 |     46.65 |      0.00 |         4 |
| history-3x | match-long           | V1  |             11.96 |     43.41 |      0.00 |      0.00 |         2 |
| history-3x | match-other-region   | P0  |              4.32 |      7.76 |      5.22 |      0.00 |         4 |
| history-3x | match-other-region   | P1  |             12.00 |      4.35 |      0.00 |      0.01 |         2 |
| history-3x | match-other-region   | V0  |              4.89 |      7.59 |      5.42 |      0.00 |         4 |
| history-3x | match-other-region   | V1  |              3.33 |      4.48 |      0.00 |      0.00 |         2 |
| history-3x | race-busy            | P0  |            906.54 |   2760.55 |     91.96 |     81.01 |         4 |
| history-3x | race-busy            | P1  |            592.04 |   2755.60 |     89.63 |     79.73 |         4 |
| history-3x | race-busy            | V0  |            522.95 |   2856.47 |     95.14 |     91.85 |         4 |
| history-3x | race-busy            | V1  |            498.99 |   2793.95 |    105.84 |     84.60 |         4 |
| history-3x | race-sparse          | P0  |             29.34 |    104.98 |      3.60 |      5.98 |         4 |
| history-3x | race-sparse          | P1  |             32.42 |    106.68 |      3.46 |      5.87 |         4 |
| history-3x | race-sparse          | V0  |             64.19 |    107.19 |      3.71 |      5.94 |         4 |
| history-3x | race-sparse          | V1  |             34.89 |    105.36 |      3.79 |      5.90 |         4 |
| history-3x | segment-history-busy | P0  |           3382.55 |  14757.91 |   1974.16 |      0.89 |       201 |
| history-3x | segment-history-busy | P1  |         102710.48 |   3061.84 |   1496.65 |      0.85 |        46 |
| history-3x | segment-history-busy | V0  |           3256.84 |  14806.51 |   1970.86 |      0.86 |       201 |
| history-3x | segment-history-busy | V1  |            700.70 |   3164.72 |   1502.93 |      0.81 |        48 |

## Restored storage and preparation

| Corpus     | Arm | Raw DB MiB | Final DB MiB | Added MiB | Projection/extension preparation ms | PG CPU ms | Rust CPU ms | Rust peak MiB | PostgreSQL/PostGIS                                                                                                                                                                                                                                                                                                                                                                        |
| ---------- | --- | ---------: | -----------: | --------: | ----------------------------------: | --------: | ----------: | ------------: | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| history-3x | V0  |      626.6 |        626.6 |       0.0 |                                0.00 |      0.16 |        0.56 |          10.4 | PostGIS disabled                                                                                                                                                                                                                                                                                                                                                                          |
| history-3x | V1  |      626.6 |        771.7 |     145.1 |                            36936.26 |  12807.15 |    17313.08 |          76.4 | PostGIS disabled                                                                                                                                                                                                                                                                                                                                                                          |
| history-3x | P0  |      626.6 |        634.4 |       7.8 |                              248.60 |    240.14 |        2.56 |          10.6 | POSTGIS="3.6.4 94d984b" [EXTENSION] PGSQL="170" GEOS="3.14.1-CAPI-1.20.5" (compiled against GEOS 3.13.1) PROJ="9.8.1 NETWORK_ENABLED=OFF URL_ENDPOINT=https://cdn.proj.org USER_WRITABLE_DIRECTORY=/var/lib/postgresql/.local/share/proj DATABASE_PATH=/usr/share/proj/proj.db" (compiled against PROJ 9.6.0) LIBXML="2.9.14" LIBJSON="0.18" LIBPROTOBUF="1.5.1" WAGYU="0.5.0 (Internal)" |
| history-3x | P1  |      626.6 |        779.9 |     153.3 |                            41231.66 |  18855.83 |    17283.47 |          75.9 | POSTGIS="3.6.4 94d984b" [EXTENSION] PGSQL="170" GEOS="3.14.1-CAPI-1.20.5" (compiled against GEOS 3.13.1) PROJ="9.8.1 NETWORK_ENABLED=OFF URL_ENDPOINT=https://cdn.proj.org USER_WRITABLE_DIRECTORY=/var/lib/postgresql/.local/share/proj DATABASE_PATH=/usr/share/proj/proj.db" (compiled against PROJ 9.6.0) LIBXML="2.9.14" LIBJSON="0.18" LIBPROTOBUF="1.5.1" WAGYU="0.5.0 (Internal)" |

Relation heap/TOAST/index details are in `storage.json`; raw per-operation values in `observations.jsonl`; instrumented server plans in `plans/`. Preparation streams current Rust-decoded coordinates into binary projections in pages of 16, then builds indexes. Preparation CPU and memory are recorded separately; this does not measure the complete activity-import lifecycle.

## Idea → test → result

Each idea remains linked to the exact config, fixture hashes, source hashes and output-equality checks in `report.json`. Add reviewed interpretation and a proposed next test to the notebook; timing direction from this pilot alone is not a conclusion.
