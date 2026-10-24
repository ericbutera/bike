# Segment Specification

Segments let a rider define a route slice, match it against uploaded activities, compare repeated efforts, and optionally use the segment as input to DH analytics.

## Product Intent

Segment workflows must feel responsive. Creating a segment should validate and persist it quickly, then queue background matching. Long route-matching jobs should be observable and restartable enough to debug, but they should not make the request path or UI feel stuck.

The system favors simple, reliable processing over clever spatial optimization until profiling proves the simple path is insufficient.

## Segment Sources

A segment can be created from a GPX or TCX upload, or built from a route slice on an existing activity.

Manual segment import requires explicit route coordinates. FIT remains supported for activity uploads, but FIT is not the current manual segment-import path.

Doogal Segment Explorer is a practical source for GPX exports when a rider wants to import an existing public route segment. The app should not require Strava API keys or OAuth just to create manual segments.

The segment builder records the source activity id and route-point start/end indexes as creation provenance.

### Fixed geometry and creator renaming

- Geometry, start, finish, distance, and builder source are fixed after creation.
  A different route requires a new segment.
- Only the creator may rename a segment. Renaming trims and validates the title
  and updates metadata without matching, replacing efforts, recalculating ranks
  or standings, rebuilding activity analytics, or changing earned awards.
- Mode and starred state remain creator metadata controls with no matching effect.
- The API rejects route fields on metadata updates and provides no saved-route
  replacement endpoint. The builder creates new segments only; old builder edit
  links show that the saved route is locked.
- Creating a duplicate returns the existing segment without renaming it or
  replacing its stored builder source. Renaming uses the creator metadata action.
- This is the first stream 1 slice of PR 100. LOAD05, LOAD06, and LOAD19 remain
  follow-up work for stable effort persistence, lock order, and historical matching;
  no data conversion or backfill is needed to retire route replacement.

## Segment Ownership And Visibility

- Segments are global, shared route definitions. The stored creator ID does not
  restrict access to global standings.
- Any authenticated athlete can read a segment's saved overall achievements,
  including athletes without an effort on that segment. A missing segment is 404.
- Personal achievement reads scope saved positions to the authenticated athlete
  and requested segment. Comparison, yearly-best, and analysis queries scope
  efforts and source activity metadata to that athlete. Other athletes' efforts,
  recording samples, and activity details never become personal results.
- Any authenticated athlete may read their own personal achievements and
  comparison results for an existing shared segment. No efforts returns an
  empty personal list, while overall standings still include all athletes.
- Browser sessions isolate cached personal data across account changes and
  logout; see [authentication](authentication.md#browser-session-data).
- Current standings and immutable activity awards are distinct; see
  [achievements](achievements.md).
- Any authenticated athlete can list and open shared segments, including routes
  they did not create and segments on which they have no efforts. The catalog
  and overall summaries are global; PRs, yearly bests, comparisons, and analysis
  remain scoped to the authenticated athlete. Global best times and badges use
  saved overall results, never rankings inferred from the viewer's effort subset.
- Responses expose `can_manage` from authenticated creator identity. Only the
  creator may rename, change mode/starred state, or delete a
  segment. Other viewers see read-only controls and may download public geometry.
  Creator-only starred state is hidden from other viewers; independent per-user
  favorites remain a separate feature.
- Builder source activity references and original upload filenames are visible
  only to the creator. Public route geometry excludes recording heart rate and
  speed; comparison effort samples remain private to their athlete.
- Shared browsing has distinct creator, contributor, and spectator scenarios in
  Rust and UI tests. Matching follow-up is tracked in the
  [segment backlog](../TODO.md#segments).

## Segment Mode

Every segment has a mode:

- `xc` is the default and safe fallback;
- `dh` is an explicit opt-in used by downhill analytics.

Only `dh` segments feed DH progress. A segment must never appear in DH analytics because of title, shape, speed, or any other heuristic alone.

Segments can also be starred. Starred state is a user-facing organization signal and should not alter matching semantics.

## Matching And Processing

Segment upload and segment-builder requests should:

1. validate the input;
2. persist the fixed segment;
3. enqueue effort regeneration;
4. return the segment with queued task id/status when available.

The request path should not scan activities or call route matching inline. CPU-heavy matching belongs in worker tasks.

When matching completes, the worker should rebuild segment analytics and refresh affected activity analytics so the rest of the app sees consistent derived data.

- `regenerate_activity_segments` works as an individual task as well as a child
  of user regeneration. Individual execution must not require a batch context.
- Individual execution refreshes affected segment caches. A batch records those
  segment IDs for its parent's shared rebuild after all matching children settle.
- Both paths refresh the matched activity's own analytics, including clearing
  stale counts when it loses its last match. A segment cache rebuild alone cannot
  discover an activity removed from all of that segment's current efforts.

Incomplete analytics should feel normal in the UI. A newly created segment may initially show queued or pending matching instead of efforts.

The activity ingestion graph also rebuilds segment efforts after an activity is saved. That pipeline owns import orchestration and trace events; this spec owns the segment route-matching behavior used by that graph stage.

### Activity effort reconciliation

- Activity matching reconciles all processed activity/segment pairs in its existing
  activity transaction. Keep the authenticated activity owner in every query and write.
- Identify a saved traversal by its segment and recording start/end point indexes.
  An unchanged matching result performs no effort writes, retaining its ID, creation
  time, saved ranks and timestamps at this step. An earlier removed traversal must
  not transfer its ID to a later one.
- Attempt numbers follow recording order. Update a retained traversal's attempt number
  or changed metrics in place; clear its ranks until the existing standings stage refreshes them.
- Insert new traversals in bounded batches. Remove only saved IDs no longer present in
  the processed activity; another activity or athlete's efforts remain untouched.
- Preserve the activity transaction and downstream standings calls.
  Existing saved effort rows need no conversion or backfill. Consumers keep using the
  retained IDs, and immutable earned awards are not rewritten.
- Historical matching reuses the same reconciler for one owned activity/segment
  pair at a time. Never delete and reinsert a segment's complete effort set.
  Re-read the activity under its lock before matching; a removed activity is skipped.
  Matching consumes the owner's saved normalized recording artifact through P1's
  typed reference and integrity checks. Missing or corrupt cycling recordings fail
  explicitly; do not fall back to the mixed activity JSON. Non-cycling activities
  reconcile an empty match set without requiring a recording.
  A stale or non-cycling recording removes only that pair's previous traversals.
- Matching and segment deletion acquire activity locks before segment locks, with
  multiple IDs in ascending order. Current-result publication holds only its segment
  lock and commits before refreshing activity summaries.
  If an import changes the deletion's activity set before its segment lock is acquired,
  reject deletion for retry instead of acquiring new activity locks out of order.
- Refresh only the processed activity's old/new segments. On interrupted-refresh
  retries, also recover that rider's saved summaries older than the segment's existing
  activity-change marker; do not invalidate every segment the rider has ever matched.
  Pair effort writes and their change marker commit together. Immutable earned awards
  remain untouched. Removing a pair's last effort also saves that activity's counts
  in the same transaction, so interruption cannot lose the removed activity's refresh.
  No execution receipts or new freshness tables are needed.
- Across-rider historical candidate selection and bounded Temporal paging remain
  LOAD19 follow-ups; this change keeps the historical caller's existing athlete scope.
- P4 acceptance includes the frozen activity 1671 saved-value and retry comparison,
  immutable awards, and replay through the actual ingestion workflow described in PR 100.

### Geographic candidate filtering

- Save compact route bounds in `activity_route_bounds` and `segment_route_bounds`
  from normalized activity routes and fixed segment geometry. Imports and segment
  creation publish bounds with their owning transaction; rename does not rebuild them.
- Define tables, constraints, cascade foreign keys and partial spatial indexes
  through SeaORM migration builders.
- Compose candidate selection with typed SeaORM/SeaQuery expressions. Share the
  PostgreSQL box-overlap expression; keep cycling eligibility, missing-bound
  fallback and bounded paging explicit, with automatically bound parameters.
- Preserve matching behavior with ordinary real-ride fixture regressions and
  specialized geographic boundary cases. Accepted traversals must survive the
  geographic prefilter in both activity-to-segment and segment-to-activity queries.
- GiST indexes over native PostgreSQL boxes narrow candidates in SQL. Select a
  deterministic, bounded ID page before hydrating segment geometry or historical
  activity recordings. Historical candidates include cycling activities across riders.
- Expand the source box by the largest supported endpoint corridor, including a
  one-meter allowance for recording encoding precision. Coarse overlap is not a
  match; the unchanged matcher still checks direction, shape and every traversal.
- Antimeridian crossings and polar corridors use conservative longitude coverage.
  Missing bounds remain candidates. A saved route with fewer than two points
  cannot match. Heatmap exclusions or publication state do not affect these queries.
- Run `backfill_activity_route_bounds` and `backfill_segment_route_bounds` on
  `bike-tasks`, each with input `0`. Each uses 16-ID pages and short per-output
  transactions. Activity bounds require the P1 normalized recording output; run
  `backfill_activity_recordings` first. Missing recording output fails explicitly.
- Backfills save only bounds. They do not match efforts, refresh standings or
  reconsider awards. P14 owns the historical matching workflow; personal
  recording/comparison reads remain owner-scoped.

## Matching Contract

Bike matches stored segment routes against normalized activity route points to create segment efforts. This contract must stay stable across parser, matcher, import, and performance changes.

- A segment effort is directional. A reverse ride must only match a reverse segment route, not the forward segment.
- Matching starts with ordered endpoint candidates. The activity must pass near the segment start before it can pass near the segment end.
- Candidate efforts must remain within the configured segment distance ratio bounds.
- Candidate efforts are scored against sampled route shape points. Compare scores within one traversal; a cleaner later lap must not discard an earlier valid lap.
- Repeated segment efforts in a single activity are allowed. After a match is accepted, later searches continue after that effort's end route point.
- Apply strict, normal fallback and eligible reworked-trail profiles to every traversal. A strict lap must not suppress a different lap needing a more lenient profile, in either recording order.
- Overlapping profile candidates for the same traversal produce one effort with the strictest valid profile's recording bounds. Distinct segments search independently, including overlapping and nested routes.
- Return all traversals in recording order. The existing persistence operation assigns per-segment attempt numbers from that order; speed and matching quality never reorder attempts.
- Short segments must not use the broad reworked-trail fallback. Small connector segments are too easy to confuse with nearby trails when the tolerance grows to handle full-route trail reworks. Strict and normal fallback matching still apply to short segments.

## Reworked Trails

Some public segments represent routes that have drifted from the current trail because of reroutes or trail work. Bike should follow Strava-compatible leniency for these cases:

- Endpoint order and approximate distance remain required.
- Moderate shape deviation is acceptable when the activity still follows the same practical segment corridor.
- The reworked-trail fallback must not replace a stricter match for the same traversal; stricter matches on other laps do not disable it.
- Leniency is for old-vs-current route drift, not for matching unrelated nearby trails or the wrong direction.
- The broad reworked-trail fallback is intended for longer public trail routes where the same route has moved over time. It must not be used to infer efforts on short, isolated connectors, private-property connectors, or trails the rider did not actually enter.

## Regression Fixtures

Favorite segment regression fixtures live in `bike-rs/tests/fixtures/segment_matching`. The test `favorite_segment_fixtures_match_expected_activity_files` parses the real FIT and GPX files and asserts that each listed segment produces at least one match.

Required fixture matches:

- `unmarked_01.fit` must match `Segment - F-BOMB OUT.gpx`.
- `unmarked_01.fit` must match `Segment - Jam Sesh.gpx`.
- `unmarked_01.fit` must match `Segment - Adderall in Reverse.gpx`.
- `unmarked_01.fit` must match `Not-So Holy Grail (EB).gpx`.
- `unmarked_01.fit` must match `Segment - The Holy Grail Single Track.gpx`.
- `unmarked_02.fit` must match `VST Clockwise 2020 (6 to 5 - Saplings).gpx`.
- `unmarked_02.fit` must match `Segment - F-BOMB IN.gpx`.
- `unmarked_02.fit` must match `Segment - Rally Back CBS Rework.gpx`.
- `commons_01.fit` must match `Segment - Breakin The Law.gpx`.
- `commons_01.fit` must match `Segment - FMR.gpx`.
- `commons_01.fit` must match `Segment - FMR Full.gpx`.
- `commons_01.fit` must match `Log Jam.gpx`.
- `city_01.fit` must match `Segment - Breakin The Law.gpx`.
- `city_01.fit` must match `Segment - -g.gpx`.
- `city_01.fit` must match `Segment - The Hick's Descent.gpx`.
- `city_01.fit` must match `Segment - Hickory Hills To Hickory Meadows.gpx`.
- `city_02.fit` must match `East Side Flow.gpx`.
- `city_02.fit` must match `Country Club Boys.gpx`.
- `city_02.fit` must match `Segment - -g.gpx`.
- `city_02.fit` must match `Segment - The Hick's Descent.gpx`.
- `city_02.fit` must match `Segment - Breakin The Law.gpx`.

False-positive regression fixtures are equally important. The short-connector fixture captures a historical bug where a nearby activity route window matched a connector segment even though the activity only passed an adjacent trail corridor. `short_connector_nearby_route.gpx` must not match `short_connector_segment.gpx`, even though the raw reworked-trail fallback profile alone would accept it.

Changes to segment parsing, route point derivation, segment effort generation, or matcher thresholds must run:

```bash
cargo test favorite_segment_fixtures_match_expected_activity_files -- --nocapture
```

Changes that touch matcher internals should also run:

```bash
cargo test segment_support
```

## Saved Analytics Persistence

- LOAD01: write effort overall and personal ranks in batches of at most 500,
  preserving duration-then-effort-ID ordering and independent personal ranks
  for each rider. Update only ranks and the modification timestamp; retain
  effort identity, traversal order, recording bounds and saved award decisions.
- LOAD02: insert segment/rider summaries in batches of at most 500, including
  each rider's effort count and personal-best effort and duration.
- LOAD03: load one saved effort snapshot for effort analytics, personal podiums
  and overall standings. Preserve each domain's distinct rules in
  [achievements](achievements.md); never reload efforts between these calculations.
- Publish all three current-result outputs in the same segment transaction.
  Retain the existing exclusive segment lock and compare the loaded segment's
  activity-change and update timestamps before any writes. Changed or deleted
  input publishes nothing; a failure in any output rolls back all three.
- Standalone personal and overall backfills keep their own segment transaction
  and lock before loading efforts, then use the same domain calculations and
  replacement operations as the combined refresh. Release the segment lock
  before refreshing activity summaries, preserving saved awards and effort IDs.
- No efforts clears rider summaries, personal podiums and overall standings,
  and stores an empty segment summary. Unrelated segments remain untouched.
- These persistence changes introduce no new saved fields or ranking rules.
  Existing rows need no conversion or backfill; explicit refreshes use the same
  `rebuild_segment_analytics` operation as normal worker processing.

## Segment Responses

The segment list should be lightweight. It should include summary information such as title, mode, starred state, distance, effort count, best duration, current-user PR, builder source, and processing state. It should not load full route blobs or full effort route slices just to render the list.

The segment detail page may load route points and effort details because the rider is inspecting one segment.

Upload and builder responses should avoid expanding all efforts immediately after creation. The created segment plus queued processing state is enough for user feedback.

## Segment Downloads

- The actions menu on `/segments/:id` places **Download segment** immediately above **Delete segment**.
- Download exports the segment’s current stored route as a GPX 1.1 track, for both uploaded and builder-created segments.
- The file is named `segment-<id>.gpx` and includes the current segment title, ordered coordinates, and available elevations.
- Export contains only the segment route; it does not include efforts, rider telemetry, or the rest of a source activity.
- Download is disabled when fewer than two route points are available.

## Duplicate Detection

Duplicate detection should preserve geometric duplicate behavior, but it should avoid loading every route blob where possible. Filtering by user and compact distance bucket before reading route JSON is acceptable. A stored dedupe key remains a possible future cleanup if this path becomes too expensive.

## Performance Principles

Segment processing should:

- filter candidate activities by segment owner;
- page candidate activities in deterministic order when the scan grows large;
- query only the columns needed for matching whenever practical;
- avoid selecting route blobs on list endpoints;
- rebuild analytics in worker paths rather than request handlers.

Multiple worker instances in production are acceptable. A single long task occupying one worker is not by itself a product blocker as long as requests remain responsive and jobs remain inspectable.

## UI Contract

The segment UI should support:

- segment list with mode, starred state, summary stats, and processing state;
- upload panel for GPX or TCX segment files;
- segment detail with route, efforts, personal-best context, and editable title/mode/starred state;
- segment builder creation from activity route points;
- race/comparison view for selected efforts.

Queued matching should be presented as expected progress, not as an error.

Older activities uploaded before route-point persistence or before a parser fix may need activity regeneration before they can match newly imported segments.

## Code Anchors

- Segment API and duplicate detection: `bike-rs/api/src/controllers/segments.rs`
- Route matching helpers: `bike-rs/api/src/segment_support.rs`
- Segment analytics: `bike-rs/api/src/analytics.rs`
- Regeneration workflow: `bike-rs/bike-temporal/src/workflows/regenerate_segment_efforts.rs`
- Segment list UI: `bike-ui/components/SegmentsPanel.tsx`
- Segment detail UI: `bike-ui/components/SegmentDetailPanel.tsx`
- Segment builder UI: `bike-ui/components/SegmentBuilderWorkspace.tsx`

## Follow-up tracking

Follow-up status and priority live only in the [Bike TODO](../TODO.md).
This specification is the processing reference for DATA05, DATA16, and SEG01.

## Shared real-ride regressions

- Reuse `city_01.fit`, `city_02.fit` and `commons_01.fit` with the shared segment
  GPXs to cover every segment matched on activity 1671. Add missing segment
  definitions instead of duplicating existing activity recordings.
- Freeze the established City fixture segment membership, every traversal in recording
  order, recording indexes, elapsed-time bounds and durations in
  `city_efforts.json`. Include nonmatching segment definitions as negative cases.
  Expected values come from the pre-change `main` matcher and are cross-checked
  against saved efforts; document any existing differences in the fixture README.
  Never regenerate them from a changed candidate to make a test pass.
- Parse the FIT/GPX files through the normal activity parser and run regressions
  as ordinary unit tests in local checks and CI. They require no production
  service, private snapshot, environment variable or ignored test.
- Connected acceptance loads the same recordings and segment GPXs into the
  existing migrated E2E PostgreSQL database. Execute the production candidate
  queries in both directions, then persist activity matches and compare ordered
  effort windows with `city_efforts.json`.
- Connected cases must exercise multiple ID pages, matches across activity owners,
  missing-bound fallback, noncycling exclusion, saved empty/single-point routes,
  distant routes, and inclusive corridor boundaries. Keep matcher negatives even
  when their coarse boxes overlap. The default E2E gate runs these checks through
  the native fixture command without an ingestion worker.
- GPX contains geometry; its distances are calculated from GPS. Exact provider
  distances and persistence still use the frozen activity 1671 DAG integration
  reference defined in the [performance plan](https://github.com/ericbutera/bike/blob/docs/performance-backlog/docs/specs/performance.md#activity-1671-regression-acceptance).
