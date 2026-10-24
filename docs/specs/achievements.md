# Achievements

## Personal achievements

- Each rider who has an effort on a segment has their own all-time PR.
- Save the rider's three fastest individual efforts, keyed by
  `(segment_id, user_id, position)`: 1 is PR, 2 is second, and 3 is third.
- Riders with fewer than three efforts have only the positions they can fill.
  Repeated attempts in the same activity remain separate eligible efforts.
- Faster duration wins. Equal durations use the smaller effort ID first,
  preserving the existing deterministic ordering.
- Positions describe current all-time results. New efforts can displace older
  ones; removing an effort promotes the next qualifying effort.
- Day, week, month, and year selections show fastest times in that date window.
  They do not create or redefine PRs or personal podium achievements.

## Saved personal output

- `segment_personal_achievements` stores segment, rider, position, effort ID,
  and duration. Uniqueness protects each position and each selected effort.
- The worker reads saved segment efforts, calculates each rider's top three,
  and replaces the segment's personal achievement rows in one transaction.
  An empty result clears any previous rows.
- Imports, reprocessing, segment regeneration, and activity deletion refresh
  affected segment analytics. That operation also refreshes personal achievements
  through the same named domain operation as backfill.
- Personal reads return the stored positions for the rider and segment while
  processing catches up. Return an empty list when no positions are saved.
  Reads do not check processing versions, source timestamps, or checksums.
- Never scan source efforts, recalculate positions, hash inputs, or write output
  during a read.
- `backfill_personal_achievements` processes all segments in 16-segment ID pages.
  Await each operation, then Continue-As-New after a full page. Rerunning the
  backfill replaces the same rows without creating duplicates.

## Personal API and UI

- Authenticated `GET /segments/{id}/personal-achievements` returns the current
  rider's saved rows ordered by position, with `position`, `effort_id`, and
  `duration_seconds`. Use shared segment access, with personal output always
  scoped to the authenticated rider.
- Return `[]` when no personal positions are saved and 404 for a missing segment.
  Do not accept a rider ID from the caller.
- Segment detail displays an all-time personal achievements panel with PR,
  second and third labels and saved durations. Show only saved positions,
  including repeated attempts from one activity, with links selecting each effort.
- Show loading, error and empty states within the panel. Available saved results
  remain visible during refresh and do not depend on comparison samples.
- Effort date filters never change these all-time personal positions. The panel
  describes current standings, not historical awards earned by activities.

## Overall achievements

- Save each segment's current global all-time top ten in
  `segment_overall_achievements`, keyed by `(segment_id, position)`.
- Each athlete occupies one position, represented by their fastest individual
  effort. Faster duration wins; equal durations use the smaller effort ID first.
  Positions are consecutive from 1 through at most 10, including tied times.
- Store athlete, effort ID, and duration with each position. Uniqueness protects
  one athlete per segment and one selected effort. Foreign keys cascade deletion.
- The worker follows the [saved analytics persistence contract](segment-processing.md#saved-analytics-persistence)
  to replace standings transactionally. Empty segments clear previously saved
  rows. New efforts can displace rows; deleted efforts promote the next eligible athlete/attempt.
- Existing segment analytics refreshes overall standings after imports,
  reprocessing, regeneration, and activity deletion using the same operation
  as `backfill_overall_achievements`.
- The backfill accepts an integer segment-ID cursor starting at zero, processes
  all segments in 16-segment ID pages, awaits each replacement, and uses
  Continue-As-New after a full page. Repeating it replaces the same saved rows.
- `GET /segments/{id}/overall-achievements` requires authentication and the
  [shared segment access policy](segment-processing.md#segment-ownership-and-visibility). Return the saved rows ordered by position,
  including athlete ID, profile display name, effort ID, and duration; missing
  output returns `[]`. Profile names enrich saved positions without filtering
  or recalculating them; a missing name leaves the position intact.
- Reads return whatever is stored while workers catch up. Never scan efforts,
  recalculate rankings, check freshness, or write during this read.
- Segment detail shows an "Overall top ten" table with position, athlete, and
  time, including loading, failure, and empty states. This section always shows
  all-time standings independently of any effort-list date selection.
- This slice excludes date-window standings, historical earned awards, and
  changes to existing effort ranks or activity achievement highlights.

## Earned awards

- The existing `rebuild_activity_achievements` stage records an activity's awards
  once, after its segment processing has refreshed live personal and overall
  results. Strava calls the same operation after refreshing segment results.
  Use those saved results directly; do not reconstruct past standings
  from activity dates, timestamps, or processing versions.
- An effort earns its saved overall position when it is the effort selected in
  `segment_overall_achievements`, and its saved personal position when selected
  in `segment_personal_achievements` for that rider. Other efforts earn no award.
  Keep the existing display priority: KOM, overall top ten, PR, personal podium.
- Load live award inputs with one overall-results query and one personal-results
  query across the activity's distinct segment IDs. Both queries filter by rider
  in the database. Do not query standings separately for every segment or load
  other riders' overall rows just to discard them.
- Save the existing highlight fields: segment ID and title, attempt number,
  overall and personal positions, and the live personal-best duration. Preserve
  repeated attempts in recording order.
- `activity_award_outputs` stores the activity ID and rider once its decision
  is saved. An output with no award rows means no award, including activities
  with no efforts. No output row means unevaluated.
- `activity_earned_awards` stores one row per earned attempt, with typed columns
  for activity ID, recording-order position, display position, segment ID and
  title, attempt number, overall and personal positions, and personal-best duration.
  Its key is `(activity_id, position)`; each segment attempt and each display
  position occurs once per output. Store no JSON, version, timestamp, or
  processing-state fields in these tables.
- The award-writing stage saves `display_position` for new decisions and legacy
  conversion: KOM first, then overall top-ten positions ascending, PR, and personal
  second/third. Equal category/rank retains numeric recording order. Segment titles
  are display labels and never affect ordering. Recording `position` and attempt
  numbers remain unchanged.
- Activity responses expose `earned_awards` as an always-present array of typed
  `activity_earned_awards` rows, including `activity_id`, recording `position`, and
  `display_position`. List, detail, and update responses use the same row shape;
  an activity with no saved award rows returns `[]`. Remove the old
  `achievement_highlights` response field and highlight DTO rather than keeping
  compatibility aliases.
- API award queries order by saved `display_position`. The activity list loads
  awards once for the page and groups the unchanged rows by activity. It does
  not recalculate priority or sort by ranks/titles in application code. The UI
  uses the generated award-row contract, preserves received order, and formats
  labels. Retries and regeneration preserve both stored orders.
- Retries, reprocessing, rematching, segment edits/deletion, and standings
  rebuilds leave the saved rows unchanged. Deleting the activity cascades through
  its output to its awards. Historical rows do not reference replaceable effort
  rows or deleted segments.
- Activity analytics rebuilds derive counts from these rows and refresh effort
  counts. Activity API awards read these rows with the authenticated rider's
  ownership scope. Neither operation decides awards from mutable effort ranks.
- A refresh of multiple segments completes their current-result operations first,
  then refreshes affected activity summaries once with deduplicated activity IDs.
  Release segment transactions before acquiring activity-summary locks. An activity
  shared by several affected segments must not reload its saved awards per segment.
  If a segment operation fails, preserve the previous summaries until the batch
  succeeds on retry; saved award decisions remain unchanged.
- Before making a first live decision, the award operation converts any existing
  `activity_analytics.achievement_highlights_json` into typed award rows, preserving
  its values, order, and explicit empty outcome. Clear that legacy JSON in the
  same transaction as recording the output. Failed conversions leave it available
  for retry. Already saved outputs always win.
- Analytics refreshes select only the previous activity IDs and owners, then
  upsert scalar counts without reading, deserializing or replacing legacy JSON.
  The JSON column remains conversion input only; it is not an API source.
- Frontend activity award chips use `earned_awards` only. An empty award array
  yields no chips; never fall back to current effort ranks or the removed highlight
  response field.
- Once an activity has a typed award output, NULL legacy JSON must not change
  its awards, including empty outcomes, API reads, refreshes or retries. Preserve
  unconverted input until conversion commits. Retiring the remaining conversion
  reader and legacy column after rollout is required by
  [PR 100](https://github.com/ericbutera/bike/pull/100) before the overall migration
  is complete.
- `backfill_earned_awards` accepts an integer activity-ID cursor starting at zero.
  It selects activities without saved outputs in 16-activity ID pages, awaits the
  existing award operation for each, and uses Continue-As-New after a full page.
  Each operation also refreshes that activity's analytics. Repeating the backfill
  skips saved decisions, including empty ones.
- Missing legacy JSON uses the ordinary first decision from current saved live
  results. Converting retained JSON preserves only the last stored outcome;
  historical outcomes already overwritten by mutable recalculation cannot be
  recovered. The backfill does not reparse original recordings or rematch segments.

See [segment processing](segment-processing.md) for efforts and
[background work](background-work.md) for Temporal execution.
