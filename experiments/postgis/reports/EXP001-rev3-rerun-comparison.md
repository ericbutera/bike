# 20261003T182930592Z-exp001-smoke-b84088 → 20261003T183649774Z-exp001-smoke-8c59e8

Source files changed: none.

Equal outputs verified in both runs. Small sample changes describe variation, not an adoption decision.

| Corpus     | Case                 | Arm | Before ms | After ms | Change % | Rust CPU change % | PG CPU change % |
| ---------- | -------------------- | --- | --------: | -------: | -------: | ----------------: | --------------: |
| history-1x | detail-maximum       | P0  |    136.00 |   137.27 |      0.9 |               0.3 |             8.0 |
| history-1x | detail-maximum       | P1  |    177.84 |   132.95 |    -25.2 |             -21.8 |            -0.5 |
| history-1x | detail-maximum       | V0  |    143.46 |   136.47 |     -4.9 |              -1.1 |           -73.7 |
| history-1x | detail-maximum       | V1  |    140.49 |   137.66 |     -2.0 |              -4.1 |            35.3 |
| history-1x | detail-median        | P0  |     23.09 |    15.09 |    -34.6 |             -24.2 |           -16.2 |
| history-1x | detail-median        | P1  |     13.87 |    14.36 |      3.6 |               1.8 |             9.6 |
| history-1x | detail-median        | V0  |     15.18 |    15.46 |      1.9 |              -0.8 |            -1.7 |
| history-1x | detail-median        | V1  |     15.16 |    16.74 |     10.4 |               1.0 |            21.8 |
| history-1x | detail-p95           | P0  |     52.27 |    55.05 |      5.3 |               1.0 |            48.7 |
| history-1x | detail-p95           | P1  |     55.70 |    54.43 |     -2.3 |              -1.7 |            18.4 |
| history-1x | detail-p95           | V0  |     54.15 |    52.86 |     -2.4 |              -2.7 |            14.3 |
| history-1x | detail-p95           | V1  |     59.49 |    53.89 |     -9.4 |              -7.7 |           -20.7 |
| history-1x | detail-route-free    | P0  |      1.58 |     1.02 |    -35.0 |             -19.7 |           -33.8 |
| history-1x | detail-route-free    | P1  |      2.55 |     1.14 |    -55.4 |             -44.5 |           -56.5 |
| history-1x | detail-route-free    | V0  |      1.64 |     1.03 |    -37.0 |             -35.7 |           -38.8 |
| history-1x | detail-route-free    | V1  |      1.07 |     1.41 |     31.3 |              11.0 |            36.9 |
| history-1x | heatmap-empty        | P0  |   7131.83 |  7020.04 |     -1.6 |               0.2 |             0.5 |
| history-1x | heatmap-empty        | P1  |      9.03 |    11.45 |     26.7 |               8.7 |             9.2 |
| history-1x | heatmap-empty        | V0  |   7195.83 |  7067.17 |     -1.8 |              -2.3 |            -2.8 |
| history-1x | heatmap-empty        | V1  |      1.62 |     1.84 |     13.5 |               4.8 |            10.7 |
| history-1x | heatmap-home-90days  | P0  |    480.52 |   481.66 |      0.2 |               1.7 |             1.1 |
| history-1x | heatmap-home-90days  | P1  |     41.05 |    24.01 |    -41.5 |              -6.5 |           -57.4 |
| history-1x | heatmap-home-90days  | V0  |    474.07 |   477.88 |      0.8 |              -0.7 |            50.7 |
| history-1x | heatmap-home-90days  | V1  |     15.46 |    15.17 |     -1.9 |              -2.5 |           -11.1 |
| history-1x | heatmap-home         | P0  |   7113.96 |  7198.00 |      1.2 |               0.8 |            -0.3 |
| history-1x | heatmap-home         | P1  |    227.32 |   218.31 |     -4.0 |               1.1 |           -12.2 |
| history-1x | heatmap-home         | V0  |   7091.21 |  7157.68 |      0.9 |               1.3 |             3.7 |
| history-1x | heatmap-home         | V1  |    196.06 |   201.87 |      3.0 |               0.5 |            71.9 |
| history-1x | heatmap-other-region | P0  |   7054.36 |  7048.53 |     -0.1 |              -0.9 |            -2.5 |
| history-1x | heatmap-other-region | P1  |     62.04 |    62.80 |      1.2 |              -1.9 |             6.2 |
| history-1x | heatmap-other-region | V0  |   7066.23 |  7027.13 |     -0.6 |              -0.9 |             0.5 |
| history-1x | heatmap-other-region | V1  |     46.73 |    50.01 |      7.0 |               5.3 |             6.9 |
| history-1x | map-input-maximum    | P0  |     59.02 |    58.18 |     -1.4 |              -1.1 |             8.2 |
| history-1x | map-input-maximum    | P1  |     19.17 |    15.83 |    -17.4 |               9.5 |           -32.3 |
| history-1x | map-input-maximum    | V0  |     59.42 |    62.17 |      4.6 |               2.5 |           -19.7 |
| history-1x | map-input-maximum    | V1  |      9.01 |     8.09 |    -10.2 |               4.7 |           -17.1 |
| history-1x | map-input-median     | P0  |      8.20 |     6.74 |    -17.8 |              -5.9 |             2.6 |
| history-1x | map-input-median     | P1  |      9.71 |    10.21 |      5.2 |              -9.0 |             6.7 |
| history-1x | map-input-median     | V0  |      8.42 |     6.80 |    -19.2 |              -1.7 |            -2.0 |
| history-1x | map-input-median     | V1  |      1.74 |     1.86 |      7.0 |              -7.9 |            13.7 |
| history-1x | map-input-p95        | P0  |     22.80 |    24.63 |      8.0 |               5.0 |             1.8 |
| history-1x | map-input-p95        | P1  |     11.62 |    24.64 |    112.1 |              -0.1 |           154.4 |
| history-1x | map-input-p95        | V0  |     23.11 |    23.95 |      3.6 |               3.4 |            19.4 |
| history-1x | map-input-p95        | V1  |      4.98 |     3.99 |    -19.8 |             -15.4 |           -39.5 |
| history-1x | match-local          | P0  |     17.49 |    16.24 |     -7.2 |              -8.4 |           -25.1 |
| history-1x | match-local          | P1  |     18.60 |    18.20 |     -2.2 |               9.5 |            24.4 |
| history-1x | match-local          | V0  |     15.26 |    16.79 |     10.0 |               5.7 |            14.6 |
| history-1x | match-local          | V1  |      6.24 |     6.29 |      0.7 |              -9.4 |            27.6 |
| history-1x | match-long           | P0  |    104.49 |   106.37 |      1.8 |               1.2 |            -9.5 |
| history-1x | match-long           | P1  |    109.99 |    72.60 |    -34.0 |             -40.9 |            -6.9 |
| history-1x | match-long           | V0  |    108.33 |   109.09 |      0.7 |               3.1 |             3.3 |
| history-1x | match-long           | V1  |     57.04 |    54.79 |     -3.9 |              -3.1 |           -14.3 |
| history-1x | match-other-region   | P0  |     17.07 |    24.01 |     40.6 |              25.9 |            76.0 |
| history-1x | match-other-region   | P1  |     18.25 |    17.38 |     -4.8 |               4.3 |             2.5 |
| history-1x | match-other-region   | V0  |     17.13 |    21.28 |     24.2 |               3.1 |            14.6 |
| history-1x | match-other-region   | V1  |      7.16 |     8.00 |     11.7 |               3.4 |            22.8 |
| history-1x | race-busy            | P0  |   1209.23 |  1258.81 |      4.1 |               4.1 |            10.4 |
| history-1x | race-busy            | P1  |   1157.06 |  1140.22 |     -1.5 |              -1.5 |           -16.0 |
| history-1x | race-busy            | V0  |   1168.14 |  1181.27 |      1.1 |              -4.5 |           -12.6 |
| history-1x | race-busy            | V1  |   1180.80 |  1229.12 |      4.1 |               0.2 |            -8.9 |
| history-1x | race-sparse          | P0  |     54.52 |    51.71 |     -5.2 |              -2.4 |           -14.2 |
| history-1x | race-sparse          | P1  |     52.12 |    56.66 |      8.7 |               1.7 |           163.7 |
| history-1x | race-sparse          | V0  |     48.96 |    50.02 |      2.2 |              -0.9 |            -3.2 |
| history-1x | race-sparse          | V1  |     55.46 |    51.77 |     -6.7 |              -1.9 |           -65.9 |
| history-1x | segment-history-busy | P0  |   6652.72 |  6613.08 |     -0.6 |              -1.4 |             5.0 |
| history-1x | segment-history-busy | P1  |  13928.96 | 13902.68 |     -0.2 |               0.5 |            -0.2 |
| history-1x | segment-history-busy | V0  |   6552.65 |  6561.40 |      0.1 |              -1.8 |            -2.2 |
| history-1x | segment-history-busy | V1  |   1943.75 |  1772.43 |     -8.8 |              -7.1 |            -4.8 |
