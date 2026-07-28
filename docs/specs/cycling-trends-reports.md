# Cycling Trends Reports Specification

Bike should make ride history useful in ways Strava does not: compare endurance, climbing, fatigue, and execution across rides over time. The reports experience should answer "what is changing?" before it answers "what happened on one ride?"

This spec ports the useful concepts from `scripts/cycling_trends` into Bike's Rust backend and `/training/reports` UI.

## Product Intent

The reports screen is the exploratory trend surface for ride analysis. It should help a rider compare training rides, benchmark rides, and race efforts across months and seasons.

Reports are ad-hoc. A rider chooses a date range, chooses a report, optionally adjusts report-specific filters, and generates results on demand. The screen should not assume one fixed dashboard answers every future question.

The first version should focus on deterministic metrics from imported activity data. Race readiness scoring can be added later after the underlying analyzer and trend rows are stable.

The core comparison question is:

```text
Lumberjack
  -> 60-mile training ride
  -> latest 8-hour ride
  -> Marji
```

Then show comparable metrics:

```text
Aerobic decoupling: 4.8% -> 4.5% -> 3.9% -> 4.2%
Median climb rate: 286 -> 301 -> 318 -> 326 m/h
Late ride fade: -14% -> -11% -> -8% -> ?
```

## Source Reference

The Python reference in `scripts/cycling_trends` contains four useful layers:

- single-ride summary and heart-rate zone metrics;
- hourly durability metrics;
- valley-to-crest climb detection and climb summaries;
- cross-ride rows grouped by week, month, and ride type.

Bike should not run this Python in production. The logic should be converted into Rust and rebuilt from the normalized activity data already stored in `activities.derived_data_json`.

## Implementation Status

Status as of 2026-07-28: the report runner foundation, backend report registry, minimum server-side filters, standalone report responses, and compare-rides speed trend view are implemented. The full spec is not done.

Implemented:

- `/training/reports` has a report menu with `ride_summary`, `endurance`, `climbing`, `fatigue`, `compare_rides`, and `aggregate_trends`.
- The UI has explicit start and end date controls and preserves `report`, `range`, `start_date`, `end_date`, and `activity_ids` in the URL.
- `/api/training/reports` accepts `report`, `start_date`, `end_date`, `boundary`, `activity_ids`, `min_duration_seconds`, and `min_distance_meters`.
- A Rust report registry now declares the initial six report definitions with stable ids, supported filters, required data quality, result sections, metrics, and metric direction.
- `/api/training/reports/definitions` exposes the backend-owned report registry, and the UI report menu consumes it with a local fallback for initial render or request failure.
- The backend validates report ids instead of silently falling back to aggregate trends for unknown `report` values.
- Minimum duration and minimum distance filters are applied server-side before generating report results and compare-ride candidate lists.
- Standalone report responses exist for ride summary, endurance, climbing, fatigue, and compare rides.
- The backend reads normalized route/chart points from `activities.derived_data_json` for standalone analyzer-style computations.
- The backend computes basic ride summary totals, hourly durability rows, valley-to-confirmed-crest climb rows, climb summaries, late ride speed/HR changes, and a compare-rides metric table.
- Compare rides sorts selected rides chronologically and shows first-to-latest trend deltas so benchmark ride speed changes are visible over time.
- Compare rides includes moving speed, standalone Z2 speed when heart-rate zone bounds and sample distance support it, and median 60-second post-climb HR recovery.
- The climbing report includes average cadence, average power, 30-second/60-second HR recovery, seconds to drop 10/15 bpm, and whether the summit immediately enters a descent when source data supports those fields.
- The existing bucket charts remain available through the aggregate trends report.
- Focused backend tests cover registry contents, report id validation, minimum filter validation, climb detection defaults, and compare-rides chronological speed trends.

Missing or incomplete:

- Report filters are still incomplete. The API now supports explicit activity ids plus minimum duration and minimum distance, but it does not yet support ride focus, route family, keyword, or automatic matching criteria beyond client-side candidate shortcuts.
- The aggregate trends report still depends on `activity_training_analyses` for several metrics instead of being fully produced by the standalone reports analyzer.
- The analyzer code is still embedded in `api/src/controllers/reports.rs`; it has not been extracted into a dedicated Rust `activity_trends` analyzer module.
- Ride summary does not yet include coasting time, cadence fields, power fields, or compact per-ride summaries.
- Endurance does not yet expose a deterministic fatigue index, terrain-sensitive labeling, or benchmark-route guidance.
- Fatigue hourly rows do not yet include climb rate or coasting minutes, and the fatigue index is still a minimal efficiency-drop marker rather than the full deterministic scoring described below.
- Compare rides does not yet include HR zone distribution, route-family-aware speed interpretation beyond a route-sensitive label, route-family matching, ride-type matching, or race-effort matching.
- Aggregate trends has not yet expanded beyond chart points into overall, weekly, monthly, ride-focus, route-family, and label rollups with the required aggregate fields.
- Tests still need broader coverage for non-climbing analyzer helpers, server-side filter query behavior, frontend filter rendering, and report response contracts before these reports should be treated as complete.

Completion criteria:

- The UI should avoid showing per-report completion badges until the backend registry can report meaningful availability/completeness from the same source of truth used by the API.
- A report is complete only when its required fields below are computed by the standalone analyzer, exposed through the API, rendered in the UI, and covered by focused tests.
- Race readiness remains explicitly out of scope for the current deterministic reports work.

## Report Runner

`/training/reports` should behave like a report runner:

- choose a report from a menu;
- choose a date range;
- optionally choose ride filters such as ride focus, route family, minimum duration, minimum distance, activity ids, or keyword;
- generate results on demand;
- show the generated timestamp and input criteria;
- preserve report criteria in the URL so a report can be revisited or shared;
- allow each report to define its own result shape.

The menu is backed by a small report registry in Rust. Each report definition declares:

- stable id;
- display name;
- short purpose;
- supported filters;
- required data quality;
- result sections;
- metrics included;
- whether higher, lower, or neutral is better for each metric.

This registry is the extension point for future Bike racing metrics. Adding a new report should not require rewriting the whole reports endpoint or UI. It should mean adding a report definition, analyzer functions when needed, response serializer, and UI renderer for the declared result sections.

## Current Report Menu

The current menu includes these report types:

- Ride summary: overall volume, intensity, climbing, stopped time, and data quality for the selected range.
- Endurance: aerobic decoupling, hourly efficiency, moving average HR, hourly speed, and fatigue index.
- Climbing: climb summaries plus raw climb table.
- Fatigue: hour-by-hour durability, stop frequency, HR, speed, climb rate, and efficiency.
- Compare rides: selected or automatically matched rides shown side by side.
- Aggregate trends: weekly, monthly, ride-focus, and route-family rollups for the selected date range.

Race readiness should appear in this menu later, after the deterministic report analyzer and trend rows are extracted, tested, and stable.

## Date Range Behavior

Every ad-hoc report must accept explicit `start_date` and `end_date` inputs. Preset ranges can fill those fields, but the backend contract should use concrete dates.

Useful presets:

- last 7 days;
- last 30 days;
- last 90 days;
- last 6 months;
- year to date;
- last 12 months;
- custom.

The API should reject invalid ranges and very large ranges only when the report would be too expensive to generate on demand.

## Standalone Analyzer

The reports analyzer must be standalone. It should not depend on `activity_training_analyses`, fitness freshness caches, segment analytics caches, or a new report cache as its source of truth.

For each request, the backend should load activities in the selected date range, read the normalized activity detail data from `activities.derived_data_json`, run the selected analyzer, and return the generated result.

The analyzer should compute:

- ride summary metrics;
- hourly durability rows;
- detected climb rows;
- climb summary statistics;
- late-ride fade metrics;
- trend-row fields used for comparison.

This keeps report development independent from ingestion and background processing. Import and reprocess workflows should not need to know which ad-hoc reports exist.

If performance becomes a problem later, caching can be added as an internal optimization, but it must not change report semantics. A cached implementation would still need to produce the same result as running the standalone analyzer against the selected activity data.

## Ride Summary Report

Show aggregate summary cards for the selected date range and, when useful, one compact summary per selected ride.

Required fields:

- elapsed time;
- moving time;
- stopped time;
- coasting time when cadence is available;
- distance;
- elevation gain;
- climbing density;
- average speed;
- average HR;
- max HR;
- HR zone distribution;
- available power/cadence fields.

The summary should expose data quality flags when HR, elevation, distance, or speed is missing enough to weaken derived metrics.

## Endurance Report

The endurance report tracks aerobic durability over the selected date range.

Required per-ride fields:

- aerobic decoupling;
- first-half efficiency;
- second-half efficiency;
- hourly efficiency;
- hourly moving average HR;
- hourly speed;
- late ride speed change;
- late ride HR change;
- fatigue index.

Aerobic decoupling should compare first half vs second half efficiency using speed per HR where HR and distance are present. The report must label this as terrain-sensitive for MTB rides and encourage comparing similar ride types or named benchmark routes.

Hourly endurance rows are mandatory. A single first-half vs second-half value is not enough to show when fatigue begins.

## Climbing Report

Use valley-to-confirmed-crest climb detection, not only grade-block accumulation. The Python defaults are a good starting point:

- minimum gain: 20 m;
- minimum duration: 90 s;
- minimum distance: 300 m;
- summit confirmation drop: 5 m;
- valley reset drop: 8 m.

The main climbing report should summarize climbs before showing the raw table.

Required summary rows:

- longest climb;
- fastest vertical rate;
- median climb;
- 95th percentile climb;
- first-half median climb;
- second-half median climb;
- best climb;
- worst climb.

The raw climb table should include:

- climb number;
- start time;
- summit time;
- duration;
- distance;
- gain;
- average grade;
- vertical rate;
- average speed;
- average HR;
- peak HR;
- average cadence when available;
- average power when available;
- 30-second and 60-second HR recovery when available;
- seconds to drop 10 bpm and 15 bpm when available;
- whether the summit immediately enters a descent;
- first-half or second-half label.

## Fatigue Report

The fatigue report should show when performance starts to fall apart inside long rides within the selected date range.

Instead of only first-half vs second-half, group the ride by hour:

```text
Hour 1 -> Hour 2 -> Hour 3 -> ...
```

Required hourly fields:

- average HR;
- max HR;
- average speed;
- distance;
- ascent;
- climb rate;
- moving minutes;
- stopped minutes;
- stop frequency;
- coasting minutes when cadence is available;
- efficiency.

The first useful fatigue index can be deterministic:

- compare each hour against the best or median early-hour baseline;
- penalize speed decline at similar or higher HR;
- penalize climb-rate decline;
- penalize increasing stopped time or stop frequency;
- avoid scoring when data quality is too weak.

## Compare Rides Report

This is the primary product differentiator.

Users should be able to compare:

- explicitly selected rides;
- latest rides above a duration or distance threshold;
- rides in the same route family;
- rides with the same ride type;
- race efforts against training rides.

The comparison table should show one row per metric and one column per ride:

- aerobic decoupling;
- median climb rate;
- late ride fade;
- median 60-second HR recovery after climbs;
- stopped time percentage;
- climbing density;
- moving speed;
- Z2 speed;
- Z2 time;
- HR zone distribution;
- total distance;
- total elevation;
- elapsed time;
- moving time.

Each row can include a first-to-latest delta so the rider can see what changed over time. The UI should interpret the delta only when higher or lower is clearly better. For example, lower decoupling is generally better; higher stopped time is generally worse; moving speed is route-sensitive and should remain neutral unless route family matches.

## Aggregate Trend Reports

Keep the current `/training/reports` bucket charts as one report type, but expand the response beyond chart points.

Required aggregate groups:

- overall selected range;
- weekly;
- monthly;
- by ride focus;
- by route family;
- by manual or inferred ride label once labels exist.

Required aggregate fields:

- ride count;
- elapsed hours;
- moving hours;
- distance;
- elevation gain;
- climbing density;
- median aerobic decoupling;
- median vertical rate;
- median late ride fade;
- median 60-second HR recovery;
- stopped minutes;
- HR zone distribution.

## Race Readiness Later

Race readiness should be built on top of the standalone analyzer outputs, not mixed into the current deterministic report analyzer work.

Future score dimensions:

- aerobic durability;
- climbing endurance;
- technical durability;
- fueling execution;
- recovery;
- ultra pacing.

The first readiness version should return component scores and the evidence behind each score. The UI should not show a single weighted score until the components are stable and tested.

## Implementation Plan And Progress

Done:

- Define the backend report registry and response envelope.
- Add explicit `start_date`, `end_date`, `report`, `activity_ids`, `min_duration_seconds`, and `min_distance_meters` inputs to the reports API.
- Reuse normalized route/chart samples from `activities.derived_data_json`.
- Compute ride summary totals, hourly durability rows, climb detection rows, climb summaries, late-ride changes, and compare-rides metric rows.
- Use valley-to-confirmed-crest climb detection with the current defaults.
- Expose ride summary, endurance, climbing, fatigue, compare-rides, and aggregate trends through `/api/training/reports`.
- Keep the existing bucket charts available as the aggregate trends report.
- Replace the reports UI with a report menu, date range controls, minimum duration/distance filters, generated results, summary cards, trend tables, comparison controls, and charts.
- Add compare-rides chronological trend deltas for speed, endurance, climbing, recovery, stopping, and Z2 metrics when enough data exists.

Partially done:

- Analyzer logic exists, but it is still embedded in `api/src/controllers/reports.rs` instead of a dedicated `activity_trends` module.
- Helper functions exist for mean, percent change, moving/stopped detection, climb detection, and Z2 speed, but time-weighted means, interpolation, rolling medians, coasting detection, and reusable trend-row construction are incomplete.
- Trend rows exist for compare rides, but route-family-aware interpretation and automatic matching are not implemented.
- Aggregate trends still relies on `activity_training_analyses` for some metrics.

Next actionable slices:

- Extract the standalone analyzer into a dedicated Rust module with pure structs/functions and focused tests for non-climbing helpers.
- Add route-family and ride-focus filters/matching so compare-rides speed trends can distinguish true improvement from route differences.
- Add HR zone distribution and route-family-aware speed interpretation to compare rides.
- Fill fatigue gaps: climb rate, coasting minutes, and a deterministic fatigue index that accounts for speed, HR, climb rate, stopped time, and data quality.
- Fill ride summary gaps: coasting time, cadence fields, power fields, and compact per-ride summaries.
- Expand aggregate trends into overall, weekly, monthly, ride-focus, route-family, and label rollups produced by the standalone analyzer.
- Add frontend tests for report definitions, filters, and compare-rides trend rendering.

Defer race readiness, training-plan compliance, manual ride labels, weighted scoring, and report caching until the standalone analyzer is trustworthy.

## Open Decisions

- Whether route-family grouping should stay title-derived or move to user-managed labels.
- Whether stop frequency should count zero-speed intervals, paused elapsed gaps, or both.
- Whether GPX/TCX files without cadence should omit coasting or infer it from speed only.
- Whether power-based efficiency should be added now or after HR/speed/climb metrics ship.
- How much of this belongs on `/training/reports` versus `/xc`.
- Whether report-level caching is ever needed as an internal optimization for large date ranges.

## Code Anchors

- Python reference: `scripts/cycling_trends`
- Current Rust training summary logic, for reference only: `api/src/activity_training_analysis.rs`
- Current reports API: `api/src/controllers/reports.rs`
- Activity derived data: `api/src/activity_details.rs`
- Reports UI: `ui-next/app/training/reports/page.tsx`, `ui-next/components/reports`
