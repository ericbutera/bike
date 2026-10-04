# EXP001-smoke revision 3: Repeatability and correctness smoke run

Reviewed aggregate copy. Private raw files and the exact measured dirty-source snapshot remain in ignored `results/20261003T182930592Z-exp001-smoke-b84088/`.

Run: `20261003T182930592Z-exp001-smoke-b84088`; outcome: **passed**.

Ideas: IDEA001, IDEA002, IDEA003. Change under test: Precision correction: prepare V1 binary coordinate/bounds and P1 geometry projections through the owning Rust decoder; PostGIS endpoint candidate filtering.

Source: `e640932586f21257bcaf31b08af6abc8164dfce1`; code fingerprint: `2c068029c91e4adf07ed91f28f66b9c4f1e7d5f2a80672bd59d071c4d32e59f5`.

Images: PostgreSQL `sha256:2621f9f68f64080b79d71a888efb49bc8a50b821472abecfe06d99e42db88fe7`; Rust probe `sha256:bcda6bd4646eefcc10b1716c9d41c0b8d0e1a73dffeab40a86143780757c1acb`.

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

| Corpus     | Case                 | Arm |   n | Median ms | p95 ms | Range ms          | PG CPU ms/op | Rust CPU ms/op | Rust peak/retained MiB | PG snapshot MiB | Datum KiB | Candidates | Vertices | Output equal |
| ---------- | -------------------- | --- | --: | --------: | -----: | ----------------- | -----------: | -------------: | ---------------------- | --------------: | --------: | ---------: | -------: | ------------ |
| history-1x | detail-maximum       | P0  |   1 |    136.00 |      — | 136.00–136.00     |         9.29 |         126.41 | 53.4/10.8              |          1241.4 |    7235.5 |          1 |    35797 | yes          |
| history-1x | detail-maximum       | P1  |   1 |    177.84 |      — | 177.84–177.84     |         8.46 |         160.44 | 53.4/10.9              |           313.0 |    7235.5 |          1 |    35797 | yes          |
| history-1x | detail-maximum       | V0  |   1 |    143.46 |      — | 143.46–143.46     |        40.37 |         128.01 | 53.4/10.8              |          1241.4 |    7235.5 |          1 |    35797 | yes          |
| history-1x | detail-maximum       | V1  |   1 |    140.49 |      — | 140.49–140.49     |         8.45 |         132.56 | 53.3/10.8              |          1241.3 |    7235.5 |          1 |    35797 | yes          |
| history-1x | detail-median        | P0  |   1 |     23.09 |      — | 23.09–23.09       |         3.67 |          16.14 | 10.6/4.5               |          1234.9 |     735.3 |          1 |     3254 | yes          |
| history-1x | detail-median        | P1  |   1 |     13.87 |      — | 13.87–13.87       |         1.93 |          12.12 | 11.0/4.5               |          1235.0 |     735.3 |          1 |     3254 | yes          |
| history-1x | detail-median        | V0  |   1 |     15.18 |      — | 15.18–15.18       |         3.35 |          12.01 | 10.6/4.5               |          1231.0 |     735.3 |          1 |     3254 | yes          |
| history-1x | detail-median        | V1  |   1 |     15.16 |      — | 15.16–15.16       |         4.01 |          11.77 | 10.6/4.5               |          1233.9 |     735.3 |          1 |     3254 | yes          |
| history-1x | detail-p95           | P0  |   1 |     52.27 |      — | 52.27–52.27       |         4.09 |          48.48 | 22.8/6.6               |          1237.1 |    2976.6 |          1 |    13677 | yes          |
| history-1x | detail-p95           | P1  |   1 |     55.70 |      — | 55.70–55.70       |         4.95 |          49.71 | 22.7/6.6               |          1237.3 |    2976.6 |          1 |    13677 | yes          |
| history-1x | detail-p95           | V0  |   1 |     54.15 |      — | 54.15–54.15       |         4.12 |          50.27 | 22.6/6.6               |          1235.3 |    2976.6 |          1 |    13677 | yes          |
| history-1x | detail-p95           | V1  |   1 |     59.49 |      — | 59.49–59.49       |         7.29 |          52.31 | 22.7/6.6               |          1237.0 |    2976.6 |          1 |    13677 | yes          |
| history-1x | detail-route-free    | P0  |   1 |      1.58 |      — | 1.58–1.58         |         1.25 |           0.40 | 10.7/3.6               |           334.6 |      28.8 |          1 |        0 | yes          |
| history-1x | detail-route-free    | P1  |   1 |      2.55 |      — | 2.55–2.55         |         2.15 |           0.65 | 10.6/3.6               |           335.1 |      28.8 |          1 |        0 | yes          |
| history-1x | detail-route-free    | V0  |   1 |      1.64 |      — | 1.64–1.64         |         1.37 |           0.49 | 10.6/3.6               |           333.5 |      28.8 |          1 |        0 | yes          |
| history-1x | detail-route-free    | V1  |   1 |      1.07 |      — | 1.07–1.07         |         0.87 |           0.33 | 10.7/3.6               |           333.9 |      28.8 |          1 |        0 | yes          |
| history-1x | heatmap-empty        | P0  |   1 |   7131.83 |      — | 7131.83–7131.83   |      1056.28 |        6273.86 | 132.2/68.3             |           754.1 | 1057898.7 |       1189 |  4854345 | yes          |
| history-1x | heatmap-empty        | P1  |   1 |      9.03 |      — | 9.03–9.03         |         8.95 |           0.24 | 10.9/3.4               |           747.6 |       0.0 |          0 |        0 | yes          |
| history-1x | heatmap-empty        | V0  |   1 |   7195.83 |      — | 7195.83–7195.83   |      1113.21 |        6410.88 | 132.7/95.1             |           547.6 | 1057898.7 |       1189 |  4854345 | yes          |
| history-1x | heatmap-empty        | V1  |   1 |      1.62 |      — | 1.62–1.62         |         1.51 |           0.23 | 10.8/3.4               |           541.2 |       0.0 |          0 |        0 | yes          |
| history-1x | heatmap-home-90days  | P0  |   1 |    480.52 |      — | 480.52–480.52     |        47.66 |         436.04 | 62.6/48.2              |           761.6 |   69254.6 |         37 |   346035 | yes          |
| history-1x | heatmap-home-90days  | P1  |   1 |     41.05 |      — | 41.05–41.05       |        28.64 |          12.66 | 10.8/4.1               |           756.7 |    1424.4 |          5 |    91158 | yes          |
| history-1x | heatmap-home-90days  | V0  |   1 |    474.07 |      — | 474.07–474.07     |        49.48 |         442.68 | 62.8/48.2              |           756.6 |   69254.6 |         37 |   346035 | yes          |
| history-1x | heatmap-home-90days  | V1  |   1 |     15.46 |      — | 15.46–15.46       |         4.61 |          11.54 | 10.7/4.1               |           752.1 |    1424.4 |          5 |    91158 | yes          |
| history-1x | heatmap-home         | P0  |   1 |   7113.96 |      — | 7113.96–7113.96   |      1104.91 |        6276.38 | 132.4/83.7             |           547.8 | 1057898.7 |       1189 |  4854345 | yes          |
| history-1x | heatmap-home         | P1  |   1 |    227.32 |      — | 227.32–227.32     |        80.67 |         151.56 | 28.2/6.1               |           593.7 |   19075.1 |        132 |  1220700 | yes          |
| history-1x | heatmap-home         | V0  |   1 |   7091.21 |      — | 7091.21–7091.21   |      1061.48 |        6253.14 | 132.5/93.4             |           709.9 | 1057898.7 |       1189 |  4854345 | yes          |
| history-1x | heatmap-home         | V1  |   1 |    196.06 |      — | 196.06–196.06     |        44.71 |         152.28 | 28.1/6.1               |           332.2 |   19075.1 |        132 |  1220700 | yes          |
| history-1x | heatmap-other-region | P0  |   1 |   7054.36 |      — | 7054.36–7054.36   |      1044.90 |        6282.85 | 137.1/83.0             |           813.7 | 1057898.7 |       1189 |  4854345 | yes          |
| history-1x | heatmap-other-region | P1  |   1 |     62.04 |      — | 62.04–62.04       |        26.17 |          35.90 | 10.7/9.7               |           336.0 |    4762.7 |         92 |   304735 | yes          |
| history-1x | heatmap-other-region | V0  |   1 |   7066.23 |      — | 7066.23–7066.23   |      1059.92 |        6256.88 | 134.4/110.5            |           805.0 | 1057898.7 |       1189 |  4854345 | yes          |
| history-1x | heatmap-other-region | V1  |   1 |     46.73 |      — | 46.73–46.73       |        11.33 |          35.70 | 10.5/9.6               |           803.4 |    4762.7 |         92 |   304735 | yes          |
| history-1x | map-input-maximum    | P0  |   1 |     59.02 |      — | 59.02–59.02       |         7.03 |          52.24 | 46.5/16.2              |           319.0 |    7235.5 |          1 |    35797 | yes          |
| history-1x | map-input-maximum    | P1  |   1 |     19.17 |      — | 19.17–19.17       |        14.07 |           5.37 | 10.6/7.2               |           332.9 |     559.3 |          1 |    35797 | yes          |
| history-1x | map-input-maximum    | V0  |   1 |     59.42 |      — | 59.42–59.42       |         9.81 |          51.33 | 46.4/16.2              |           316.0 |    7235.5 |          1 |    35797 | yes          |
| history-1x | map-input-maximum    | V1  |   1 |      9.01 |      — | 9.01–9.01         |         2.50 |           5.76 | 10.6/7.2               |           318.4 |     559.3 |          1 |    35797 | yes          |
| history-1x | map-input-median     | P0  |   1 |      8.20 |      — | 8.20–8.20         |         1.82 |           5.38 | 10.5/4.8               |          1235.1 |     735.3 |          1 |     3254 | yes          |
| history-1x | map-input-median     | P1  |   1 |      9.71 |      — | 9.71–9.71         |         8.82 |           0.92 | 10.7/3.9               |          1236.1 |      50.9 |          1 |     3254 | yes          |
| history-1x | map-input-median     | V0  |   1 |      8.42 |      — | 8.42–8.42         |         2.56 |           5.18 | 10.5/4.7               |          1235.0 |     735.3 |          1 |     3254 | yes          |
| history-1x | map-input-median     | V1  |   1 |      1.74 |      — | 1.74–1.74         |         1.15 |           0.81 | 10.6/3.9               |          1235.7 |      50.9 |          1 |     3254 | yes          |
| history-1x | map-input-p95        | P0  |   1 |     22.80 |      — | 22.80–22.80       |         3.90 |          19.10 | 20.1/8.7               |          1237.5 |    2976.4 |          1 |    13677 | yes          |
| history-1x | map-input-p95        | P1  |   1 |     11.62 |      — | 11.62–11.62       |         8.86 |           2.90 | 10.7/4.9               |          1238.6 |     213.7 |          1 |    13677 | yes          |
| history-1x | map-input-p95        | V0  |   1 |     23.11 |      — | 23.11–23.11       |         3.68 |          19.56 | 20.1/8.8               |          1237.1 |    2976.4 |          1 |    13677 | yes          |
| history-1x | map-input-p95        | V1  |   1 |      4.98 |      — | 4.98–4.98         |         2.30 |           3.22 | 10.7/4.9               |          1237.6 |     213.7 |          1 |    13677 | yes          |
| history-1x | match-local          | P0  |   1 |     17.49 |      — | 17.49–17.49       |         4.69 |          13.51 | 10.8/5.1               |           766.8 |    1611.3 |         32 |    11180 | yes          |
| history-1x | match-local          | P1  |   1 |     18.60 |      — | 18.60–18.60       |        10.86 |           4.50 | 10.5/4.9               |           769.1 |     735.1 |          0 |     3254 | yes          |
| history-1x | match-local          | V0  |   1 |     15.26 |      — | 15.26–15.26       |         3.14 |          12.20 | 10.6/5.1               |           765.1 |    1611.3 |         32 |    11180 | yes          |
| history-1x | match-local          | V1  |   1 |      6.24 |      — | 6.24–6.24         |         1.92 |           4.47 | 10.5/5.0               |           765.1 |     735.1 |          0 |     3254 | yes          |
| history-1x | match-long           | P0  |   1 |    104.49 |      — | 104.49–104.49     |         9.55 |          95.61 | 46.4/15.8              |           770.3 |    8111.4 |         32 |    43723 | yes          |
| history-1x | match-long           | P1  |   1 |    109.99 |      — | 109.99–109.99     |        28.91 |          78.47 | 46.4/16.4              |           780.9 |    7235.2 |          0 |    35797 | yes          |
| history-1x | match-long           | V0  |   1 |    108.33 |      — | 108.33–108.33     |         9.90 |          96.90 | 46.5/15.8              |           768.1 |    8111.4 |         32 |    43723 | yes          |
| history-1x | match-long           | V1  |   1 |     57.04 |      — | 57.04–57.04       |         8.67 |          47.38 | 46.5/14.3              |           769.0 |    7235.2 |          0 |    35797 | yes          |
| history-1x | match-other-region   | P0  |   1 |     17.07 |      — | 17.07–17.07       |         4.01 |          13.21 | 10.7/5.2               |           777.2 |    1680.3 |         32 |    11542 | yes          |
| history-1x | match-other-region   | P1  |   1 |     18.25 |      — | 18.25–18.25       |        11.94 |           5.09 | 10.6/5.1               |           780.9 |     804.1 |          0 |     3616 | yes          |
| history-1x | match-other-region   | V0  |   1 |     17.13 |      — | 17.13–17.13       |         3.48 |          13.74 | 10.4/5.2               |           772.2 |    1680.3 |         32 |    11542 | yes          |
| history-1x | match-other-region   | V1  |   1 |      7.16 |      — | 7.16–7.16         |         2.60 |           4.71 | 10.4/5.1               |           774.5 |     804.1 |          0 |     3616 | yes          |
| history-1x | race-busy            | P0  |   1 |   1209.23 |      — | 1209.23–1209.23   |       216.69 |        1039.20 | 365.0/90.5             |           674.1 |  175461.6 |        204 |  1985658 | yes          |
| history-1x | race-busy            | P1  |   1 |   1157.06 |      — | 1157.06–1157.06   |       180.35 |        1059.37 | 365.1/90.6             |           743.1 |  175461.6 |        204 |  1985658 | yes          |
| history-1x | race-busy            | V0  |   1 |   1168.14 |      — | 1168.14–1168.14   |       190.33 |        1094.23 | 365.1/90.5             |           400.9 |  175461.6 |        204 |  1985658 | yes          |
| history-1x | race-busy            | V1  |   1 |   1180.80 |      — | 1180.80–1180.80   |       222.63 |        1036.67 | 365.0/90.5             |           535.7 |  175461.6 |        204 |  1985658 | yes          |
| history-1x | race-sparse          | P0  |   1 |     54.52 |      — | 54.52–54.52       |        11.57 |          44.54 | 23.7/14.3              |           761.0 |    6953.6 |          4 |    31943 | yes          |
| history-1x | race-sparse          | P1  |   1 |     52.12 |      — | 52.12–52.12       |        10.92 |          42.73 | 23.8/14.3              |           767.2 |    6953.6 |          4 |    31943 | yes          |
| history-1x | race-sparse          | V0  |   1 |     48.96 |      — | 48.96–48.96       |         7.86 |          43.08 | 23.7/14.3              |           747.8 |    6953.6 |          4 |    31943 | yes          |
| history-1x | race-sparse          | V1  |   1 |     55.46 |      — | 55.46–55.46       |        27.18 |          43.55 | 23.6/14.3              |           757.5 |    6953.6 |          4 |    31943 | yes          |
| history-1x | segment-history-busy | P0  |   1 |   6652.72 |      — | 6652.72–6652.72   |       965.09 |        5922.43 | 73.5/30.4              |          1055.1 |  989566.8 |       1061 |  4540979 | yes          |
| history-1x | segment-history-busy | P1  |   1 |  13928.96 |      — | 13928.96–13928.96 |     12559.33 |        1569.70 | 53.3/34.5              |           503.3 |  201288.5 |        230 |   935331 | yes          |
| history-1x | segment-history-busy | V0  |   1 |   6552.65 |      — | 6552.65–6552.65   |       942.29 |        5980.71 | 73.7/68.5              |           869.6 |  989566.8 |       1061 |  4540979 | yes          |
| history-1x | segment-history-busy | V1  |   1 |   1943.75 |      — | 1943.75–1943.75   |       210.81 |        1776.38 | 56.3/29.4              |           948.8 |  208219.4 |        245 |   965320 | yes          |

## Stage medians

| Corpus     | Case                 | Arm | SQL + transfer ms | Decode ms | Domain ms | Output ms | SQL calls |
| ---------- | -------------------- | --- | ----------------: | --------: | --------: | --------: | --------: |
| history-1x | detail-maximum       | P0  |             11.38 |     43.46 |      0.00 |     29.22 |         2 |
| history-1x | detail-maximum       | P1  |             10.19 |     83.27 |      0.00 |     31.21 |         2 |
| history-1x | detail-maximum       | V0  |             12.91 |     50.15 |      0.00 |     29.70 |         2 |
| history-1x | detail-maximum       | V1  |              9.82 |     44.59 |      0.00 |     29.31 |         2 |
| history-1x | detail-median        | P0  |              5.43 |      7.36 |      0.00 |      3.62 |         2 |
| history-1x | detail-median        | P1  |              1.93 |      4.24 |      0.00 |      2.84 |         2 |
| history-1x | detail-median        | V0  |              3.36 |      4.04 |      0.00 |      2.89 |         2 |
| history-1x | detail-median        | V1  |              3.24 |      4.06 |      0.00 |      2.95 |         2 |
| history-1x | detail-p95           | P0  |              4.41 |     16.92 |      0.00 |     11.74 |         2 |
| history-1x | detail-p95           | P1  |              5.21 |     18.75 |      0.00 |     11.44 |         2 |
| history-1x | detail-p95           | V0  |              4.74 |     16.59 |      0.00 |     11.81 |         2 |
| history-1x | detail-p95           | V1  |              8.07 |     18.46 |      0.00 |     12.33 |         2 |
| history-1x | detail-route-free    | P0  |              1.29 |      0.18 |      0.00 |      0.00 |         2 |
| history-1x | detail-route-free    | P1  |              2.15 |      0.27 |      0.00 |      0.00 |         2 |
| history-1x | detail-route-free    | V0  |              1.29 |      0.20 |      0.00 |      0.00 |         2 |
| history-1x | detail-route-free    | V1  |              0.81 |      0.17 |      0.00 |      0.00 |         2 |
| history-1x | heatmap-empty        | P0  |           1168.22 |   5422.96 |    539.78 |      0.00 |        76 |
| history-1x | heatmap-empty        | P1  |              8.84 |      0.00 |      0.18 |      0.00 |         1 |
| history-1x | heatmap-empty        | V0  |           1101.75 |   5553.16 |    540.61 |      0.00 |        76 |
| history-1x | heatmap-empty        | V1  |              1.42 |      0.00 |      0.19 |      0.00 |         1 |
| history-1x | heatmap-home-90days  | P0  |             62.16 |    378.68 |     39.65 |      0.00 |         4 |
| history-1x | heatmap-home-90days  | P1  |             29.12 |      0.20 |     11.67 |      0.00 |         1 |
| history-1x | heatmap-home-90days  | V0  |             51.64 |    382.63 |     39.78 |      0.00 |         4 |
| history-1x | heatmap-home-90days  | V1  |              4.61 |      0.18 |     10.61 |      0.00 |         1 |
| history-1x | heatmap-home         | P0  |           1154.10 |   5377.70 |    581.14 |      0.00 |        76 |
| history-1x | heatmap-home         | P1  |             81.68 |      2.09 |    142.82 |      0.00 |         1 |
| history-1x | heatmap-home         | V0  |           1146.52 |   5363.52 |    580.79 |      0.00 |        76 |
| history-1x | heatmap-home         | V1  |             49.30 |      2.20 |    144.01 |      0.00 |         1 |
| history-1x | heatmap-other-region | P0  |           1092.18 |   5422.19 |    538.97 |      0.00 |        76 |
| history-1x | heatmap-other-region | P1  |             28.11 |      0.40 |     33.51 |      0.00 |         1 |
| history-1x | heatmap-other-region | V0  |           1123.67 |   5402.71 |    539.53 |      0.00 |        76 |
| history-1x | heatmap-other-region | V1  |             13.07 |      0.28 |     33.35 |      0.00 |         1 |
| history-1x | map-input-maximum    | P0  |              8.69 |     45.32 |      0.00 |      3.51 |         1 |
| history-1x | map-input-maximum    | P1  |             13.99 |      0.03 |      0.00 |      3.55 |         1 |
| history-1x | map-input-maximum    | V0  |             10.22 |     44.23 |      0.00 |      3.56 |         1 |
| history-1x | map-input-maximum    | V1  |              2.86 |      0.06 |      0.00 |      4.38 |         1 |
| history-1x | map-input-median     | P0  |              2.63 |      4.96 |      0.00 |      0.48 |         1 |
| history-1x | map-input-median     | P1  |              9.02 |      0.02 |      0.00 |      0.49 |         1 |
| history-1x | map-input-median     | V0  |              1.91 |      5.42 |      0.00 |      0.96 |         1 |
| history-1x | map-input-median     | V1  |              1.02 |      0.02 |      0.00 |      0.52 |         1 |
| history-1x | map-input-p95        | P0  |              4.31 |     16.08 |      0.00 |      1.91 |         1 |
| history-1x | map-input-p95        | P1  |              8.87 |      0.02 |      0.00 |      2.12 |         1 |
| history-1x | map-input-p95        | V0  |              4.24 |     16.34 |      0.00 |      1.99 |         1 |
| history-1x | map-input-p95        | V1  |              1.28 |      0.03 |      0.00 |      2.90 |         1 |
| history-1x | match-local          | P0  |              4.79 |      7.99 |      4.63 |      0.01 |         4 |
| history-1x | match-local          | P1  |             11.09 |      7.47 |      0.00 |      0.01 |         2 |
| history-1x | match-local          | V0  |              3.52 |      7.05 |      4.55 |      0.00 |         4 |
| history-1x | match-local          | V1  |              2.07 |      4.14 |      0.00 |      0.00 |         2 |
| history-1x | match-long           | P0  |             11.61 |     45.16 |     47.46 |      0.00 |         4 |
| history-1x | match-long           | P1  |             31.15 |     78.51 |      0.00 |      0.00 |         2 |
| history-1x | match-long           | V0  |             14.43 |     46.32 |     47.36 |      0.00 |         4 |
| history-1x | match-long           | V1  |             11.93 |     44.97 |      0.00 |      0.00 |         2 |
| history-1x | match-other-region   | P0  |              4.42 |      7.38 |      5.20 |      0.00 |         4 |
| history-1x | match-other-region   | P1  |             12.29 |      5.92 |      0.00 |      0.01 |         2 |
| history-1x | match-other-region   | V0  |              3.93 |      7.67 |      5.44 |      0.00 |         4 |
| history-1x | match-other-region   | V1  |              2.68 |      4.45 |      0.00 |      0.00 |         2 |
| history-1x | race-busy            | P0  |            227.65 |    887.68 |     25.37 |     26.10 |         4 |
| history-1x | race-busy            | P1  |            153.30 |    903.33 |     28.79 |     28.40 |         4 |
| history-1x | race-busy            | V0  |            159.02 |    911.28 |     27.89 |     26.33 |         4 |
| history-1x | race-busy            | V1  |            199.39 |    888.60 |     24.52 |     26.08 |         4 |
| history-1x | race-sparse          | P0  |             12.91 |     35.55 |      1.21 |      2.07 |         4 |
| history-1x | race-sparse          | P1  |             12.05 |     34.69 |      1.29 |      1.98 |         4 |
| history-1x | race-sparse          | V0  |              8.47 |     35.28 |      1.15 |      1.96 |         4 |
| history-1x | race-sparse          | V1  |             12.47 |     37.28 |      1.27 |      2.14 |         4 |
| history-1x | segment-history-busy | P0  |           1021.22 |   4968.56 |    655.66 |      0.29 |        69 |
| history-1x | segment-history-busy | P1  |          12422.27 |   1006.15 |    497.97 |      0.28 |        17 |
| history-1x | segment-history-busy | V0  |            856.66 |   5028.63 |    659.87 |      0.29 |        69 |
| history-1x | segment-history-busy | V1  |            234.85 |   1199.23 |    506.23 |      0.31 |        18 |

## Restored storage and preparation

| Corpus     | Arm | Raw DB MiB | Final DB MiB | Added MiB | Projection/extension preparation ms | PG CPU ms | Rust CPU ms | Rust peak MiB | PostgreSQL/PostGIS                                                                                                                                                                                                                                                                                                                                                                        |
| ---------- | --- | ---------: | -----------: | --------: | ----------------------------------: | --------: | ----------: | ------------: | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| history-1x | V0  |      214.1 |        214.1 |       0.0 |                                0.00 |      0.20 |        0.30 |          10.4 | PostGIS disabled                                                                                                                                                                                                                                                                                                                                                                          |
| history-1x | V1  |      214.1 |        262.7 |      48.6 |                            12914.23 |   4231.36 |     5856.41 |          73.3 | PostGIS disabled                                                                                                                                                                                                                                                                                                                                                                          |
| history-1x | P0  |      214.1 |        221.9 |       7.8 |                              259.19 |    248.60 |        0.43 |          10.7 | POSTGIS="3.6.4 94d984b" [EXTENSION] PGSQL="170" GEOS="3.14.1-CAPI-1.20.5" (compiled against GEOS 3.13.1) PROJ="9.8.1 NETWORK_ENABLED=OFF URL_ENDPOINT=https://cdn.proj.org USER_WRITABLE_DIRECTORY=/var/lib/postgresql/.local/share/proj DATABASE_PATH=/usr/share/proj/proj.db" (compiled against PROJ 9.6.0) LIBXML="2.9.14" LIBJSON="0.18" LIBPROTOBUF="1.5.1" WAGYU="0.5.0 (Internal)" |
| history-1x | P1  |      214.1 |        270.7 |      56.6 |                            14723.85 |   6418.05 |     5822.90 |          73.4 | POSTGIS="3.6.4 94d984b" [EXTENSION] PGSQL="170" GEOS="3.14.1-CAPI-1.20.5" (compiled against GEOS 3.13.1) PROJ="9.8.1 NETWORK_ENABLED=OFF URL_ENDPOINT=https://cdn.proj.org USER_WRITABLE_DIRECTORY=/var/lib/postgresql/.local/share/proj DATABASE_PATH=/usr/share/proj/proj.db" (compiled against PROJ 9.6.0) LIBXML="2.9.14" LIBJSON="0.18" LIBPROTOBUF="1.5.1" WAGYU="0.5.0 (Internal)" |

Relation heap/TOAST/index details are in `storage.json`; raw per-operation values in `observations.jsonl`; instrumented server plans in `plans/`. Preparation streams current Rust-decoded coordinates into binary projections in pages of 16, then builds indexes. Preparation CPU and memory are recorded separately; this does not measure the complete activity-import lifecycle.

## Idea → test → result

Each idea remains linked to the exact config, fixture hashes, source hashes and output-equality checks in `report.json`. Add reviewed interpretation and a proposed next test to the notebook; timing direction from this pilot alone is not a conclusion.
