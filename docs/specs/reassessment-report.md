# Reassessment Report Specification

The reassessment report answers one focused question: "Does the rider's recent evidence support keeping their current XC event goal live compared with their spring baseline?"

The report is a decision aid, not a generic trend dashboard. It should show the verdict first, then the specific evidence behind that verdict: endurance progression, climbing density, long-ride pace, and whether recent fitness is materially better than the spring baseline.

## Product Intent

The rider should be able to open the reassessment report from `/training/reports` and quickly understand whether the active XC event goal is supported, risky, needs more evidence, or impossible to assess from the available data.

The report should be blunt about uncertainty. A strong long ride should not hide missing fitness data. A faster short ride should not imply event readiness. A better CTL number should not outweigh a lack of event-specific long-ride and climbing evidence.

The primary answer must use four required signals:

- endurance progression;
- climbing density;
- long-ride pace;
- recent fitness vs spring baseline.

The UI should make it clear which signals are supported by data, which are risky, which need more evidence, and which are missing.

## Target Source

The saved XC event goal is the canonical target. The report should read the active goal from user preferences, including:

- event name;
- target distance;
- target elevation gain;
- target elapsed finish time;
- target date;
- event profile.

The report may provide documented defaults for a known race only as a fallback when no saved target exists, and the response must clearly label that fallback as an assumption. Hard-coded race constants must not silently override the rider's configured goal.

If the saved goal has no target finish time, the report should decline to produce a target-comparison verdict for desired finish pace and required climbing density. It should still estimate current course ability when the saved goal has distance/elevation and recent benchmark rides provide pace or climbing-density evidence.

Course distance and climbing can vary by year and source. The report should not pretend the target is more precise than the configured goal or imported course data supports.

## Windows

The reassessment window is the saved XC training start date through today. The report should not expose generic preset ranges because reassessment is tied to the active training block, not an arbitrary report range.

The spring baseline is March 1 through May 31 of the relevant year. If today is before June, the baseline should use the prior spring.

When the saved training start overlaps the spring baseline, current training comparisons should start the day after the spring baseline ends. A ride must not count as both current evidence and spring-baseline evidence.

The backend should load enough data to cover both the training block and spring baseline. The UI should display the concrete window dates and explain that the reassessment uses the saved training start through today.

If there is no current training-block benchmark after the spring baseline, the report may show the last known benchmark value separately, but it must not treat that stale value as current evidence. If fitness data exists for both the last benchmark date and the report end date, the report may also show a conservative fitness-adjusted projection as context. Projections are display-only and must not change verdict status.

## Long Rides

A long ride is an activity with at least:

- 3 hours elapsed time; or
- 35 miles distance.

These thresholds identify benchmark rides for reassessment. They are not proof of event readiness by themselves.

Benchmark ride rows should include:

- ride title and date;
- elapsed time;
- moving time when available;
- distance;
- elevation gain;
- elapsed pace;
- moving pace as context when available;
- climbing density;
- aerobic decoupling;
- late speed change;
- fatigue index.

The report must not use "longest elapsed ride" as a proxy for every best long-ride metric. It should choose the appropriate evidence for each signal:

- endurance progression: longest or farthest recent long ride, plus durability/fatigue context;
- long-ride pace: best relevant elapsed pace among qualifying long rides, preferably terrain-comparable when route context exists;
- climbing density: densest qualifying long ride or best event-specific climb-density benchmark;
- spring comparison: the same metric definition in recent and spring windows.

When these values come from different rides, the UI should make that inspectable through the benchmark table or supporting labels.

## Signal Semantics

### Endurance Progression

Endurance progression should answer whether recent long rides are materially larger or more durable than spring long rides.

Useful evidence includes:

- best recent long-ride duration;
- best recent long-ride distance;
- recent long-ride count;
- change from spring in duration or distance;
- fatigue index and late-speed fade on long rides.

The signal should not be `on_track` from distance or duration alone when fatigue evidence is poor.

### Climbing Density

Climbing density should compare vertical load per hour against the target event requirement and against spring.

The target density is:

```text
target elevation gain / target finish time
```

Use elapsed target time for event arithmetic. Use moving time for training-ride density when reporting a ride's work rate, and label that distinction where it matters.

The signal should use an aggregate climbing-density value across benchmark long rides in the window, not the single densest ride. Individual ride densities can remain inspectable in the benchmark table.

The signal should be conservative when elevation or time data is missing.

### Long-Ride Pace

Long-ride pace should compare qualifying long rides against the target pace:

```text
target distance / target finish time
```

Use elapsed time, not moving time, for the pace verdict. The event clock does not pause for breaks, stopped time, mechanicals, food stops, or route delays. Moving pace may be shown as supporting context, but it must not determine whether the target finish pace is supported.

This metric is route-sensitive. A fast road or gravel ride should not be treated as equal to technical singletrack or rough MTB terrain unless route or event-profile context supports that comparison.

The signal may show improvement from spring, but improvement alone should not imply support if the current pace remains far below target. When no target finish time is saved, the signal should still compare recent elapsed pace with the spring benchmark instead of reporting missing ride data.

### Fitness Vs Spring Baseline

Fitness delta should answer whether recent fitness is materially better than spring baseline.

The report should use fitness/freshness daily rows when available. It should compare average recent fitness against average spring fitness and also expose latest recent fitness as context.

Material improvement should require both an absolute and relative gain so low-baseline noise does not produce a false positive.

If either window lacks fitness data, this signal is `missing_data`.

## Verdict

The overall verdict is derived from the four signal statuses:

- `on_track`: strong evidence across endurance, climbing density, long-ride pace, and fitness delta, with no required signal missing.
- `plausible_but_risky`: enough evidence to keep the goal live, but at least one signal is thin, marginal, or route-sensitive.
- `needs_more_evidence`: current evidence needs more proof before it supports the target.
- `missing_data`: required ride or fitness data is missing enough that a defensible reassessment cannot be made.

The report must not return `on_track` when any required signal is `missing_data`.

A missing target finish time is not by itself missing ride data. When recent rides, spring baseline rides, fitness rows, and saved course distance/elevation exist, the report should compute current ability estimates and return the evidence verdict that follows from recent-vs-spring signals.

The report should return `missing_data` when recent long rides are absent, spring comparison data is absent, or fitness data is absent for either comparison window. If partial data is still useful, the UI may show the partial signal cards, but the top-level verdict must remain conservative.

## UI Contract

The reassessment report view should include:

- a verdict panel with target event name, target finish time, and verdict detail;
- target cards for pace, climbing density, and distance;
- four signal cards for endurance progression, climbing density, long-ride pace, and fitness delta;
- a recent-vs-spring comparison section with concrete dates;
- improvement cards for fitness, elapsed long-ride speed, long-ride distance, and climbing density;
- a benchmark long-ride table;
- notes that explain important assumptions and missing data.

The UI should not hide the baseline or target values used for a verdict. Every signal card should show recent, spring, and target values where those values apply. When a target metric is `n/a` because the saved goal lacks finish time, the UI should label that as a missing finish target rather than implying ride evidence is absent.

## API Contract

`GET /api/training/reports?report=reassessment` returns this standalone report response.

The response should include:

- generation timestamp;
- selected report range;
- canonical target values and target-source metadata;
- current ability estimate from recent benchmark rides;
- recent window metrics;
- spring baseline window metrics;
- improvement metrics;
- four signal responses;
- top-level verdict;
- benchmark ride rows;
- notes.

Target-source metadata should distinguish at least:

- `saved_goal`;
- `missing_goal`.

The report should remain backward-compatible where practical, but adding target-source metadata is required before the report should be considered complete.

## Implementation Requirements

The backend should:

- load the saved XC goal from user preferences;
- use the saved target values when present;
- load activities covering both recent and spring windows;
- load fitness/freshness rows covering both windows;
- compute signal-specific best values instead of reusing the longest ride for pace and climbing density;
- use elapsed long-ride speed for the pace signal and moving speed only as context;
- keep threshold logic deterministic and covered by tests;
- make missing data affect the top-level verdict conservatively.

The frontend should:

- render the report from the backend response, not duplicate verdict logic;
- show target-source assumptions;
- show concrete dates for both windows;
- keep benchmark rows inspectable enough to explain why a signal got its status.

## Tests

Backend tests should cover:

- report id parsing and registry definition;
- saved-goal target values overriding defaults;
- fallback target metadata when no saved goal exists;
- recent/spring window selection around dates before and after June;
- long-ride qualification by duration and by distance;
- independent best values for endurance, pace, and climbing density;
- long-ride pace verdicts using elapsed time even when moving time makes the ride look faster;
- missing fitness data forcing a conservative top-level verdict;
- no `on_track` verdict when any required signal is missing;
- verdict thresholds for supported, risky, and not-supported cases.

Frontend tests should cover:

- the report appears in the menu;
- the report route requests the reassessment report id;
- target-source assumptions are visible;
- signal cards display recent, spring, and target values;
- missing-data states render without implying support.

## Code Anchors

- Reports API: `api/src/controllers/reports.rs`
- User goal preferences: `api/src/controllers/user_preferences.rs`
- Existing XC goal loading pattern: `api/src/controllers/training_goals.rs`
- Reports UI: `ui-next/components/reports/ReportsClient.tsx`
- Report definitions UI fallback: `ui-next/components/reports/reportDefinitions.ts`
- Frontend API types: `ui-next/lib/queries.ts`

## Open Gaps

- Decide whether technical terrain specificity should use route family, event profile, activity type, or explicit user labeling.
