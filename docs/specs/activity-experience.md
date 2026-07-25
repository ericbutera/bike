# Activity Experience Specification

The activity experience is the primary read surface for Bike. It should let a rider scan recent training, inspect a single activity deeply, correct classification, and trigger regeneration when old parser output needs to be refreshed.

## Product Intent

Activities should feel like trustworthy records, not temporary import artifacts. The list is optimized for scanning and filtering. The detail page is optimized for inspecting telemetry, route, zones, matched segments, and derived training signals.

## Activity List

The home page lists authenticated user activities. It supports pagination and filtering through the API query surface.

List rows should include enough information to recognize the ride without loading full detail:

- title;
- sport and activity type;
- source and original format where available;
- start time and location;
- distance, moving time, elevation, speed, heart rate, cadence, calories, and relative effort where available;
- route preview points;
- achievement highlights.

Route preview data should be compact. The list should not fetch every detailed route or chart point for every activity.

## Activity Detail

The activity detail page can load full derived data because the rider is inspecting one activity. Detail responses may include:

- route points;
- chart points;
- laps;
- heart-rate zones;
- matched segment efforts;
- achievement highlights;
- optional training analysis;
- regeneration capability.

Telemetry display should tolerate missing data. Activities without heart rate, cadence, elevation, power, or route points should show the available record instead of implying the import failed.

## Activity Types

Bike supports a normalized activity type separate from raw provider sport labels. The rider can update activity type when provider data is too broad or wrong.

Training analytics may depend on activity type, so type updates should keep derived state consistent.

## Regeneration

Activities imported from stored files can be regenerated. Regeneration reprocesses the original import, updates derived activity data, finalizes affected imports, and refreshes analytics that depend on the activity.

Regeneration is intended for parser improvements, older activities uploaded before route persistence, and recovery from previous parser bugs.

## Delete Behavior

Deleting an activity should remove the activity and its derived state consistently. Segment summaries, analytics caches, fitness freshness, and training analyses should not retain stale contributions from deleted rides.

## Maps And Charts

Maps and charts should use the normalized route and chart data. They are display features, not sources of truth.

The route preview path and detailed map path have different payload expectations. A list preview can simplify route points; a detail page can render the full route.

## Code Anchors

- Activity API: `api/src/controllers/activities.rs`
- Derived activity data: `api/src/activity_details.rs`
- Activity lifecycle: `api/src/activity_lifecycle.rs`
- Activity training analysis: `api/src/activity_training_analysis.rs`
- Activity list UI: `ui-next/components/ActivityStream.tsx`
- Activity detail UI: `ui-next/components/ActivityDetailPanel.tsx`
- Route map UI: `ui-next/components/MapLibreRouteMapClient.tsx`

## Open Gaps

- Continue distinguishing unavailable telemetry from failed parsing in UI copy.
- Keep list payloads compact as derived data grows.
- Add focused regression coverage when activity type changes affect training analytics or segment summaries.
- Consider 3D terrain map support where it improves route inspection rather than becoming decorative.
