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
- The worker locks the segment, reads saved efforts, and replaces its standings
  in one transaction. Empty segments clear previously saved rows. New efforts
  can displace rows; deleted efforts promote the next eligible athlete/attempt.
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

- Current personal and overall positions are distinct from awards earned by an
  activity. Historical award persistence remains a separate implementation slice.
- Earned activity achievements are historical and immutable. Their original
  segment award decisions are never modified or updated.

See [segment processing](segment-processing.md) for efforts and
[background work](background-work.md) for Temporal execution.
