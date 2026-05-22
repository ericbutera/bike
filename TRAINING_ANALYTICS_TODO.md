# Bike Training Analytics TODO

This file tracks the XC and DH training analytics work for Bike.
It is the persistent implementation plan and progress log for this feature area.

## Scope

- XC goals/progress should be activity-driven and mostly automatic.
- DH goals/progress should be segment-driven and opt-in via manual segment mode.
- Two new screens are required:
  - XC goals / progress
  - DH goals / progress
- XC should track aerobic durability and climbing, not just flat Z2 pace.
- DH should only use segments explicitly marked as `dh`.

## Product Decisions

### DH

- Add a segment mode field with at least `xc` and `dh` values.
- Manual segment tagging is acceptable for v1.
- Only marked DH segments participate in DH analytics.
- Initial DH targets are segment-specific for a small curated set such as FMR and Breaking the Law.

### XC

- XC analysis is not segment-specific in v1.
- XC analysis should automatically identify long endurance rides and comparable climbing endurance rides.
- XC progress should include both aerobic and climbing goals.
- XC comparisons should be route-family-aware or heuristic-driven so unrelated technical trail rides do not contaminate endurance trends.

## Workstreams

## Current Code Anchors

- Activity parsing and derived ride data: `/Users/eric/code/sass/bike/api/src/activity_details.rs`
- Training profile and heart rate zones: `/Users/eric/code/sass/bike/api/src/training_profile.rs`
- Fitness/fatigue/form analytics: `/Users/eric/code/sass/bike/api/src/analytics.rs`
- Activity API response shape: `/Users/eric/code/sass/bike/api/src/controllers/activities.rs`
- Segment API and edit flows: `/Users/eric/code/sass/bike/api/src/controllers/segments.rs`
- Frontend activity/segment query types: `/Users/eric/code/sass/bike/ui-next/lib/queries.ts`
- Existing training profile UI: `/Users/eric/code/sass/bike/ui-next/app/account/page.tsx`

### 1. Segment Mode Support

- [x] Add `mode` to `segments` storage and API.
- [x] Expose segment mode editing in segment detail or segment settings UI.
- [x] Default existing segments safely, with DH only used when explicitly chosen.
- [x] Add tests for segment mode persistence and authorization.

### 2. Per-Activity Analysis Cache

- [x] Add an activity analysis read model/cache table.
- [x] Store deterministic metrics derived from FIT/TCX/GPX activity data.
- [x] Rebuild analysis on import, reprocess, and delete paths.
- [x] Add tests for cache lifecycle behavior.

### 3. XC Classification And Metrics

- [x] Define ride focus classification for `xc_endurance`, `mixed_xc`, `dh_session`, and `other`.
- [x] Implement Z2 metrics: time, distance, pace, HR range adherence.
- [x] Implement decoupling for comparable endurance rides.
- [x] Implement climbing metrics: elevation gain, climb time, sustained climb count, or similar heuristic.
- [x] Define route-family/comparability heuristics for XC trend lines.
- [ ] Add tests covering edge cases like wind/noisy rides and short rides.

### 4. DH Segment Analytics

- [x] Build rollups for DH-marked segments only.
- [x] Track PR, recent best, rolling top-3 average, and repeat fade within a session.
- [x] Track session-level summaries for multi-lap downhill work.
- [ ] Add tests for segment inclusion/exclusion by mode.

### 5. API Surfaces

- [x] Add XC goals/progress endpoint(s).
- [x] Add DH goals/progress endpoint(s).
- [x] Add schema/types for deterministic metrics and recommendations.
- [x] Keep APIs model-friendly so a future self-hosted LLM can summarize cached metrics.

### 6. UI Screens

- [x] Add XC goals/progress screen in `ui-next`.
- [x] Add DH goals/progress screen in `ui-next`.
- [x] Show trends, recent benchmarks, and progress toward goals.
- [x] Keep XC and DH clearly separated in the navigation and mental model.

### 7. Recommendation Layer

- [x] Start with deterministic rules for next-ride guidance.
- [x] Use fitness/fatigue/form plus XC/DH-specific metrics.
- [ ] Only add LLM-generated summaries after deterministic metrics are stable.

## Phase Plan

### Phase 1

- Segment mode support
- Per-activity analysis cache
- Initial XC metrics
- Initial DH rollups

### Phase 2

- XC goals/progress screen
- DH goals/progress screen
- Deterministic recommendations

### Phase 3

- Self-hosted model integration for narrative summaries
- Optional coaching-style explanations and trend callouts

## Current Status

- [x] Product direction agreed for separate XC and DH systems.
- [x] DH will be opt-in via manual segment mode.
- [x] XC will stay activity-driven in v1.
- [x] Segment mode schema/API/UI support landed with `xc` default and manual `dh` opt-in.
- [x] Per-activity analysis cache landed with initial Z2 and climbing metrics.
- [x] Initial XC ride-focus classification and comparability heuristics landed in the cache.
- [x] Aerobic decoupling now lands in the activity analysis cache for comparable XC endurance rides.
- [x] XC and DH goals/progress API surfaces are now available for UI work.
- [x] XC goals/progress screen is now available in `ui-next` and can track a saved event target.
- [x] DH goals/progress screen is now available in `ui-next` with session trend and segment benchmark views.
- [x] Deterministic next-ride guidance now uses fitness, fatigue, and form alongside XC/DH-specific metrics.
- [x] Reference report-script review narrowed the next credible training-page signals to climb density, temperature context, and optional HRV readiness; a separate reporting page is not needed for v1.

## Landed In This Slice

- Added `segment.mode` storage with `xc` default and `dh` manual opt-in.
- Exposed segment mode on segment responses and updates.
- Added segment mode editing to the segment detail screen.
- Added focused backend and frontend validation for segment mode support.
- Added `activity_training_analyses` as a deterministic per-activity cache table.
- Rebuild activity training analysis on import, reprocess, and delete paths.
- Exposed optional `training_analysis` on activity detail responses.
- Added initial XC metrics: Z2 time/distance/speed plus climbing time/gain/count.
- Added focused backend validation for import-path rebuilds, activity responses, and metric computation.
- Added `ride_focus` classification for `xc_endurance`, `mixed_xc`, `dh_session`, and `other`.
- Added route-family and comparison-bucket heuristics to the activity training analysis cache.
- Classified DH sessions using DH-marked segment efforts during cache rebuilds.
- Exposed the new XC classification/comparability fields through activity detail response types.
- Added `aerobic_decoupling_percent` to the activity training analysis cache and detail response.
- Compute aerobic decoupling from first-half vs second-half Z2 efficiency for comparable XC endurance rides.
- Added focused backend validation for decoupling computation, controller response shape, migration wiring, and frontend type exposure.
- Added `/api/training/xc-progress` and `/api/training/dh-progress` with deterministic goal cards, recommendations, and UI-ready progress data.
- Added DH rollups for PR, recent best, rolling top-3 average, repeat fade, and session summaries directly from DH-marked segment efforts.
- Added typed frontend query hooks for the new training endpoints in `ui-next`.
- Added a dedicated `/xc` screen with XC goal cards, weekly trend charts, durability trend, and recent-ride benchmarks.
- Added XC navigation wiring and a focused panel test against `useXcGoalProgress`.
- Added persisted XC event target fields on user preferences and surfaced season-best distance/climbing progress toward that target on `/xc`.
- Added upload processing-state UX so the import screen disables new uploads while reprocessing or another activity job is active.
- Added a dedicated `/dh` screen with DH goal cards, recent-session trend tracking, per-segment benchmark cards, and recent session summaries.
- Grouped XC and DH under shared training navigation so the two systems read as a pair instead of isolated links.
- Added a focused DH panel test against `useDhGoalProgress`.
- Added fitness/fatigue/form-aware recommendation rules to `/api/training/xc-progress` and `/api/training/dh-progress`, including recovery-vs-ready guidance based on the latest usable freshness snapshot.
- Added focused backend validation for the new XC and DH recommendation cases.
- Reviewed ad hoc FIT, GPX, and HRV scripts under `data/scripts` and scoped the stable next signals to climb density, temperature context, and optional HRV readiness instead of adding a separate reports surface.

## Next Recommended Slice

1. Add edge-case XC test coverage for noisy rides, short rides, and route-family comparability boundaries.
2. Add focused DH tests for segment inclusion and exclusion by `mode`.
3. Decide whether the training APIs should expose the current fitness/fatigue/form snapshot directly to the XC and DH screens, or keep freshness recommendation-only for v1.
4. Add climb-density and temperature context to the XC analysis pipeline and surface them on `/xc` where they help explain durability and event specificity.
5. Decide whether to ingest HRV status as optional recovery context for training recommendations and summary cards on `/xc` and `/dh`.
6. Evaluate Garmin training-effect fields as supplemental activity metadata only, not as core XC/DH planning inputs.
7. Review the ad hoc report scripts for metrics that belong on activity-level detail pages instead of the XC and DH training pages.
