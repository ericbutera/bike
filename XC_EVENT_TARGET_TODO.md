# XC Event Target TODO

Track updates needed to make `/xc` answer: "Am I on track for my next XC event, and what am I missing?"

This page should stay event-agnostic. Marji Gesick, Traverse City Trails Festival, Lumberjack 100, and future races are event targets with different demands, not separate page modes.

## Product Direction

- [x] Keep `/xc` centered on XC event readiness, not on one named race.
- [x] Treat the saved event target as the source of race-specific demands.
- [x] Preserve the current activity-driven approach: prior rides should feed progress, trends, and advice automatically.
- [x] Separate "training block cumulative progress" from "ready for event day." A completed volume bar must not imply race readiness by itself.
- [x] Make the first screen answer status quickly, then show the missing work behind that status.

## Event Target Inputs

Current inputs cover training start, target date, distance, and climbing. Add inputs only where they materially improve event-specific advice.

- [x] Add optional event name.
- [x] Add optional target elapsed finish time.
- [ ] Add optional expected stop budget, or derive it from target elapsed time vs training moving pace when unavailable.
- [x] Add optional event type/profile: XC marathon, technical singletrack, endurance gravel/MTB mix, ultra MTB, or custom.
- [ ] Add optional terrain specificity: technicality, climb pattern, and support level.
- [ ] Add optional course route/GPX target later if the app can compare ride profiles against the actual course.
- [ ] Consider quick presets/examples without hard-coding the page around them:
  - [ ] Marji Gesick MG100: 100 mi, 13,000 ft, optional 12h target.
  - [ ] Traverse City Trails Festival: 40 mi, 2,000 ft.
  - [ ] Lumberjack 100: 100 mi, 7,000 ft.

## Status Overview

- [x] Add a top-level status: `On track`, `Watch`, or `Falling behind`.
- [x] Show the reason for the status in one short sentence.
- [x] Include a "missing most" summary so the page immediately says whether the limiter is endurance volume, climbing, pace, durability, intensity, recovery, or data quality.
- [x] Make status event-aware by comparing current training against the saved target distance, climbing, finish-time goal, and days remaining.
- [x] Avoid using Z2 speed alone as the readiness verdict.

## Progress Metrics

- [x] Distance progress must show both quantity and percentage.
  - Example: `42 mi / 100 mi`, `42%`, `58 mi remaining`.
- [x] Climbing progress must show both quantity and percentage.
  - Example: `5,200 ft / 13,000 ft`, `40%`, `7,800 ft remaining`.
- [ ] Show projected total at current pace for distance and climbing.
- [x] Show required weekly distance and climbing from today.
- [x] Show current block average vs required average.
- [x] Keep best single-ride distance and best single-ride climbing, but label them as benchmarks rather than block completion.

## Readiness Gates

- [x] Add explicit gates that feed the status overview:
  - [x] Weekly endurance volume.
  - [x] Weekly climbing volume.
  - [x] Longest recent ride distance vs target distance.
  - [x] Biggest recent climbing day vs target climbing.
  - [x] Climb density match: current training ft/mi vs event ft/mi.
  - [ ] Z2 speed trend.
  - [ ] Climbing vertical-rate trend.
  - [x] Aerobic decoupling.
  - [x] Recovery/freshness when available.
  - [ ] Data quality: heart-rate zones, missing HR, missing elevation, stale backfill.
- [x] Make each gate show current value, target value, percentage, and actionable gap.
- [ ] Make gate thresholds conservative and explainable in code comments/tests.

## What Am I Missing

- [x] Add a ranked "What am I missing?" panel.
- [x] For each deficit, show:
  - [x] Metric gap.
  - [x] Why it matters for the target event.
  - [x] Next useful workout or ride.
  - [x] Priority: high, medium, low.
- [ ] Examples:
  - [ ] `Climb durability: need 7,800 ft more in the block; add a long hilly endurance ride.`
  - [ ] `Event specificity: current rides average 55 ft/mi, target is 130 ft/mi. Choose hillier trail routes.`
  - [ ] `Pace durability: Z2 speed is flat and decoupling is high. Repeat a comparable endurance route with steadier fueling.`

## Next Ride Guidance

Current state: `/xc` renders `Next ride guidance` as up to three deterministic recommendation cards from the XC progress API. The rules currently look at missing comparable rides, recent Z2 volume, recent climbing volume, decoupling, and recovery/freshness. They do not yet understand the active event target.

- [x] Keep `Next ride guidance` as the action layer for the page.
- [x] Drive it from the same readiness gates and ranked deficits used by "What am I missing?"
- [x] Make each recommendation event-aware:
  - [x] Name the limiter it addresses.
  - [x] Show the target-specific gap it helps close.
  - [x] Suggest a concrete next ride shape.
  - [x] Explain what the ride is useful for.
- [x] Include enough prescription detail to act without over-planning:
  - [x] Duration or distance range.
  - [x] Climbing target or climb density target.
  - [x] Intensity focus: Z2, tempo, threshold, punch/VO2, recovery, or mixed.
  - [x] Terrain focus: technical singletrack, sustained climbs, punchy climbs, rolling endurance, or trainer/road fallback.
  - [x] Fueling or pacing note when durability/decoupling is the limiter.
- [x] Respect recovery/freshness as a hard override before recommending bigger volume or harder work.
- [ ] Add event-phase awareness:
  - [ ] Base/build period: prioritize volume consistency and aerobic durability.
  - [ ] Specificity period: prioritize event-like terrain, climb density, long MTB rides, and race execution.
  - [ ] Taper period: prioritize freshness, openers, and confidence checks.
- [x] Avoid generic guidance like "add more Z2" when the saved target makes a more specific recommendation possible.
- [ ] Consider grouping guidance into `Next ride`, `This week`, and `Do not do yet` when multiple deficits compete.
- [ ] Keep recommendation cards compact in the UI, with optional expanded details for the reasoning.
- [x] Ensure guidance can still fall back gracefully when event target fields, heart-rate zones, elevation data, or freshness data are missing.

## Ride Benchmark Classification

- [x] Add a training-purpose label to each ride benchmark.
- [x] Suggested labels:
  - [x] Base endurance.
  - [x] Climb durability.
  - [x] Tempo.
  - [x] Threshold.
  - [x] Punch/VO2.
  - [x] Technical fatigue.
  - [x] Recovery.
  - [x] Data quality only.
- [x] Include a short "useful for" explanation in the benchmarks table or expanded row.
- [x] Keep ride focus (`xc_endurance`, `mixed_xc`, etc.) separate from training purpose.

## Trends Over Time

- [x] Bring the useful report signals into `/xc` instead of requiring the separate reports page for core readiness.
- [x] Show weekly trend lines for:
  - [x] Z2 speed.
  - [x] Climbing vertical rate.
  - [x] Weekly elevation gain.
  - [x] Weekly distance.
  - [x] Aerobic decoupling.
  - [x] Time in zones.
- [x] Highlight whether the recent trend is improving, flat, or declining.
- [x] Compare recent weeks against the opening block and against the target event requirements.

## Recommendation Rules

- [x] Replace generic v1 recommendation text with event-aware recommendations.
- [x] Keep deterministic rules first; add narrative summaries only after rules are stable.
- [ ] Incorporate coaching-style principles:
  - [ ] Base endurance matters, but finish-time goals require tempo/threshold/punch work too.
  - [ ] Long MTB rides should become more event-specific as race day approaches.
  - [ ] Technical events need technical fatigue resistance, not just road/trainer Z2 speed.
  - [ ] Recovery should gate big volume, hard intervals, and benchmark attempts.
  - [ ] Taper should be represented near event day.
- [x] Make recommendations choose among base endurance, climbing durability, tempo, threshold, punch/VO2, technical skills, fueling practice, recovery, and taper.

## API And Data Model

- [x] Extend the XC progress API response with a readiness summary.
- [x] Add structured deficit/gap objects for the "What am I missing?" panel.
- [x] Add event target fields as needed through user preferences or a dedicated event target table.
- [ ] Consider supporting multiple saved targets over time, with one active target.
- [x] Keep existing fields backward-compatible where possible.
- [ ] Add computed fields for event climb density, required weekly pace, projected block totals, and target finish speed.

## UI Work

- [x] Rework the `/xc` hero into a compact status dashboard.
- [x] Update the event target panel copy so it says this target can be any race.
- [x] Add quantity plus percent to distance and climbing progress.
- [x] Add the ranked missing-work panel near the top.
- [x] Add readiness gates with clear pass/watch/fail states.
- [x] Add training-purpose labels to ride benchmarks.
- [ ] Keep charts readable on mobile; the page should still answer status without horizontal scanning.

## Testing

- [ ] Backend tests for event target math:
  - [ ] Distance remaining.
  - [ ] Climbing remaining.
  - [ ] Required weekly pace from today.
  - [ ] Projected totals at current pace.
  - [ ] Climb density comparison.
  - [x] Target finish-time speed.
- [ ] Backend tests for readiness status:
  - [ ] On track.
  - [ ] Watch.
  - [ ] Falling behind.
  - [ ] Missing data.
  - [ ] Recovery overrides quality work.
  - [ ] Taper window.
- [ ] Backend tests for ride training-purpose classification.
- [ ] Backend tests for event-aware next ride guidance:
  - [ ] Endurance deficit produces a base endurance ride.
  - [ ] Climbing deficit produces a climbing durability ride.
  - [ ] Event-specificity deficit produces terrain/climb-density guidance.
  - [ ] Finish-time deficit produces tempo/threshold guidance when recovery allows.
  - [ ] High fatigue produces recovery guidance even when event gaps exist.
- [x] Frontend tests for quantity plus percent rendering.
- [x] Frontend tests for event-agnostic target copy and example race values.
- [x] Frontend tests for ranked missing-work recommendations.
- [x] Frontend tests for next ride guidance showing purpose, gap, and concrete ride shape.

## Open Questions

- [ ] Should event targets be a single preference record or a real table with history?
- [ ] Should presets be local static examples or user-created saved event templates?
- [ ] Should target finish time use moving time, elapsed time, or both?
- [ ] How should trainer/road rides count toward technical MTB event specificity?
- [ ] How close to the event should the page shift from build advice to taper advice?
- [ ] Should power data become first-class for tempo/threshold/punch classification when available?
