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

## Overall achievements and earned awards

- Overall achievements are a separate implementation slice: the global all-time
  top ten contains unique athletes, each represented by their fastest effort.
- Current personal positions are distinct from awards earned by an activity.
  Previously earned awards remain attached to their effort when current
  standings change.
- This slice saves current personal positions and their backfill. Global
  standings, historical award persistence, period filters, and API/UI consumer
  changes remain separate work.

See [segment processing](segment-processing.md) for efforts and
[background work](background-work.md) for Temporal execution.
