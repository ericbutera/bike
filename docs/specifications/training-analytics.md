# Training Analytics Specification

Bike treats training analytics as deterministic product behavior, not as a loose coaching experiment. The purpose of this feature area is to turn imported activity and segment data into stable XC and DH progress views that can be trusted across revisions.

## Product Intent

The training system answers different questions for different riding styles:

- XC answers: "Is my recent riding building enough endurance, climbing durability, and aerobic durability for cross-country or endurance MTB?"
- DH answers: "Are my explicitly marked downhill segments getting faster, more repeatable, and less faded within a session?"

XC analytics are activity-driven. DH analytics are segment-driven. The two should share navigation and terminology where useful, but their data models and readiness logic must remain separate.

## Activity Training Analysis

Every imported, regenerated, or reprocessed activity should have a deterministic training analysis cache when enough activity data exists. This cache is derived from the normalized FIT, TCX, or GPX data already stored for the activity.

The analysis cache stores:

- ride focus: `xc_endurance`, `mixed_xc`, `dh_session`, or `other`;
- route-family and comparison buckets for trend grouping;
- Zone 2 time, distance, and average speed;
- climbing time, climbing gain, and sustained climb count;
- aerobic decoupling for comparable XC endurance rides.

The cache is rebuilt when activity data changes. Import, reprocess, delete, and segment-regeneration paths must keep this derived state consistent with the source activity records.

## XC Analytics

XC analysis is not segment-specific in the current product. The system should infer useful XC progress from rides automatically, especially long endurance rides and climbing-endurance rides.

XC trend logic should avoid comparing unrelated rides as if they were the same workout. Route-family and distance/elevation buckets exist so that noisy comparisons, technical trail rides, trainer rides, and short rides do not distort endurance trends.

XC progress should emphasize:

- weekly endurance volume;
- weekly climbing volume;
- Zone 2 speed trend;
- climbing vertical-rate trend;
- aerobic decoupling;
- recent comparable ride count;
- recent long-ride and big-climb benchmarks.

Z2 speed alone must not be treated as the readiness verdict. It is one useful signal inside a broader durability and specificity picture.

## DH Analytics

DH analytics only use segments whose mode is explicitly set to `dh`. A segment defaults to XC behavior and must not appear in DH progress unless the rider intentionally marks it as downhill.

For DH-marked segments, the system tracks:

- personal record;
- recent best;
- rolling top-three average;
- top-three gap from personal record;
- repeat fade within a session;
- session summaries for multi-lap downhill work.

DH recommendations should reward consistency and repeatability, not just one fast effort. A rider with high fade should get different guidance than a rider with stable repeat laps.

## Recommendation Rules

Training recommendations are deterministic. They should be generated from current metrics, recent trends, and the latest usable fitness/fatigue/form snapshot.

Recovery and freshness are hard gates. When fatigue is high or form is meaningfully negative, recommendations should favor recovery or maintenance before suggesting more volume, harder work, or benchmark attempts.

LLM-generated summaries may be added later, but they must summarize stable deterministic metrics. They must not become the source of truth for readiness, priorities, or suggested workout shape.

## UI Contract

The Training navigation groups XC, DH, Segments, Fitness, and Reports together. XC and DH are peers, not hidden subfeatures of segments or reports.

The `/xc` screen should present progress, trends, event target context, readiness, and recommendations for XC riding. The `/dh` screen should present DH segment rollups, session trends, and downhill-specific recommendations.

The Reports page is a supporting trend view. Core XC readiness signals that affect guidance should appear directly on `/xc`; the rider should not need a separate report page to understand readiness.

## Code Anchors

- Activity parsing and derived ride data: `api/src/activity_details.rs`
- Activity training cache: `api/src/activity_training_analysis.rs`
- Fitness/fatigue/form analytics: `api/src/analytics.rs`
- Training API responses: `api/src/controllers/training_goals.rs`
- Training reports: `api/src/controllers/reports.rs`
- XC UI: `ui-next/components/XcGoalsProgressPanel.tsx`
- DH UI: `ui-next/components/DhGoalsProgressPanel.tsx`

## Open Gaps

- Add edge-case XC tests for noisy rides, short rides, and route-family comparability boundaries.
- Add focused DH tests proving only `dh` segments participate in DH analytics.
- Decide whether `/xc` and `/dh` should expose the latest fitness/fatigue/form snapshot directly or keep it recommendation-only.
- Add climb-density and temperature context where they explain durability and event specificity.
- Decide whether HRV status should become optional recovery context.
- Treat Garmin training-effect fields as supplemental metadata, not as core planning inputs, unless the product decision changes.
