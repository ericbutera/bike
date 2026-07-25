# XC Event Readiness Specification

The `/xc` page answers one question: "Am I on track for my next XC event, and what am I missing?"

The page is event-agnostic. Marji Gesick, Traverse City Trails Festival, Lumberjack 100, and future races are targets with different demands, not separate page modes. The saved event target is the source of race-specific context.

## Product Intent

The rider should understand status quickly, then be able to inspect the work behind that status. A completed volume bar must not imply race readiness by itself. Training block progress and event-day readiness are related, but they are not the same thing.

The first screen should communicate:

- overall readiness: `On track`, `Watch`, `Falling behind`, or `Missing data`;
- the short reason for that status;
- the limiter that is missing most;
- distance and climbing progress against the saved target;
- actionable next-ride guidance.

## Event Target

An XC event target is stored on user preferences for the current product. It includes:

- training start date;
- target date;
- target distance;
- target elevation gain;
- optional event name;
- optional target elapsed finish time;
- optional event profile: XC marathon, technical singletrack, endurance MTB, ultra MTB, or custom.

Changing the training start date queues a historical XC backfill. The backfill rebuilds activity training analyses in batches and exposes queued, running, and completed state so the UI can explain why readiness may still be catching up.

Future event-target history may move this data out of preferences and into a dedicated table. Until that product decision changes, the single active preference record is canonical.

## Progress Metrics

Distance and climbing progress must always show quantity and percent when a target exists. The rider should see both what has been counted and what remains.

The page should distinguish:

- counted block distance and elevation from the saved training start date through the target date;
- required weekly distance and climbing from today;
- current block averages compared with required averages;
- best single-ride distance and best single-ride climbing as benchmarks.

Best single-ride values are evidence, not completion. They help describe whether the rider has touched event-like demands, but they do not replace weekly consistency.

## Readiness Gates

Readiness is produced by explicit gates. Each gate should show current value, target value, percent where meaningful, actionable gap, and pass/watch/fail style status.

Current readiness gates include:

- longest recent ride distance vs target distance;
- biggest recent climbing day vs target climbing;
- climb density match between training and the event;
- target finish pace when a finish-time goal exists;
- aerobic decoupling;
- recovery/freshness when available.

Weekly endurance volume, weekly climbing volume, Z2 speed trend, climbing vertical-rate trend, and data quality are important readiness concepts even where the current response does not yet expose them as full gates.

Thresholds should be conservative and explainable in code and tests. When data is missing, the page should say that rather than implying confidence.

## Deficits

The "What am I missing?" panel ranks the rider's current deficits. A deficit is only useful if it includes:

- the metric gap;
- why that gap matters for the target event;
- a concrete next useful ride or workout;
- priority: high, medium, or low.

Deficits should be event-aware. For example, a rider targeting a steep 100-mile event should see climb durability and event specificity emphasized differently than a rider targeting a flatter 40-mile event.

## Next Ride Guidance

Next-ride guidance is the action layer of `/xc`. It is generated from the same gates and deficits that drive readiness.

Each recommendation should name the limiter, show the target-specific gap, suggest a concrete ride shape, and explain why that ride is useful. A useful ride shape can include duration or distance range, climbing target or climb density, intensity focus, terrain focus, and a fueling or pacing note.

Recovery is a hard override. If freshness says the rider should absorb load, the page should not recommend a harder benchmark or a bigger volume ride simply because an event gap exists.

Guidance must avoid generic advice when event context exists. "Add more Z2" is weaker than "Ride 3-4 hours on rolling singletrack with 2,000-3,000 ft of climbing to close the long-ride and climb-density gap."

## Ride Benchmarks

Ride benchmarks should carry a training-purpose label separate from ride focus. Ride focus describes the activity classification; training purpose explains why the ride matters to the current target.

Supported training purposes are:

- base endurance;
- climb durability;
- tempo;
- threshold;
- punch/VO2;
- technical fatigue;
- recovery;
- data quality only.

Each benchmark should include a short "useful for" explanation.

## Trends

The `/xc` page should surface the trend signals that directly affect readiness:

- Z2 speed;
- climbing vertical rate;
- weekly elevation gain;
- weekly distance;
- aerobic decoupling;
- time in zones.

Trends should compare recent weeks against the opening block and against target event requirements where that comparison is meaningful.

## UI Contract

The `/xc` hero is a compact status dashboard, not a marketing hero. It should answer status without horizontal scanning on mobile.

The event target panel should make clear that the target can be any XC-style race. Examples can help, but examples must not hard-code the page around one race.

Readiness, missing work, and next ride guidance belong near the top. Deeper charts and benchmark history support the answer after the rider understands status.

## API Contract

`GET /api/training/xc-progress` returns:

- generation timestamp;
- optional active event goal;
- optional readiness summary;
- ranked deficits;
- summary metrics;
- race-result context;
- goal cards;
- deterministic recommendations;
- weekly progress;
- recent ride benchmarks.

Existing fields should stay backward-compatible where practical.

## Code Anchors

- XC progress API: `api/src/controllers/training_goals.rs`
- Event target preferences: `api/src/controllers/user_preferences.rs`
- XC backfill: `api/src/xc_goal_backfill.rs`
- XC UI: `ui-next/components/XcGoalsProgressPanel.tsx`
- Preferences UI: `ui-next/app/account/page.tsx`

## Open Gaps

- Add expected stop budget, or derive it from target elapsed time vs training moving pace.
- Add terrain specificity fields only when they materially improve advice.
- Consider course route or GPX target comparison after the app can compare ride profiles against an actual course.
- Add event-phase awareness: base/build, specificity, and taper.
- Decide whether guidance should be grouped into "Next ride", "This week", and "Do not do yet".
- Decide whether target finish time should use moving time, elapsed time, or both.
- Decide how trainer and road rides count toward technical MTB event specificity.
- Add backend tests for event target math, readiness statuses, ride-purpose classification, and event-aware recommendation branches.
