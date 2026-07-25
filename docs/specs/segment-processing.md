# Segment Specification

Segments let a rider define a route slice, match it against uploaded activities, compare repeated efforts, and optionally use the segment as input to DH analytics or Garmin IQ sync.

## Product Intent

Segment workflows must feel responsive. Creating or editing a segment should validate and persist the segment quickly, then queue background matching. Long route-matching jobs should be observable and restartable enough to debug, but they should not make the request path or UI feel stuck.

The system favors simple, reliable processing over clever spatial optimization until profiling proves the simple path is insufficient.

## Segment Sources

A segment can be created from a GPX or TCX upload, or built from a route slice on an existing activity.

Manual segment import requires explicit route coordinates. FIT remains supported for activity uploads, but FIT is not the current manual segment-import path.

Doogal Segment Explorer is a practical source for GPX exports when a rider wants to import an existing public route segment. The app should not require Strava API keys or OAuth just to create manual segments.

The segment builder records the source activity id and route-point start/end indexes so the UI can return to and edit the chosen route slice.

## Segment Ownership And Visibility

Segments are user-owned. List, detail, edit, delete, matching, and summaries are scoped to the authenticated user unless a future product decision explicitly introduces cross-user leaderboards.

Existing cross-user leaderboard semantics should not be inferred from implementation details. User-owned manual segments are the current contract.

## Segment Mode

Every segment has a mode:

- `xc` is the default and safe fallback;
- `dh` is an explicit opt-in used by downhill analytics.

Only `dh` segments feed DH progress. A segment must never appear in DH analytics because of title, shape, speed, or any other heuristic alone.

Segments can also be starred. Starred state is a user-facing organization signal and should not alter matching semantics.

## Matching And Processing

Segment upload and segment-builder requests should:

1. validate the input;
2. persist the segment or route changes;
3. enqueue effort regeneration;
4. return the segment with queued task id/status when available.

The request path should not scan activities or call route matching inline. CPU-heavy matching belongs in worker tasks.

When matching completes, the worker should rebuild segment analytics and refresh affected activity analytics so the rest of the app sees consistent derived data.

Incomplete analytics should feel normal in the UI. A newly created segment may initially show queued or pending matching instead of efforts.

## Segment Responses

The segment list should be lightweight. It should include summary information such as title, mode, starred state, distance, effort count, best duration, current-user PR, builder source, and processing state. It should not load full route blobs or full effort route slices just to render the list.

The segment detail page may load route points and effort details because the rider is inspecting one segment.

Upload and builder responses should avoid expanding all efforts immediately after creation. The created segment plus queued processing state is enough for user feedback.

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
- segment builder create/edit flow from activity route points;
- race/comparison view for selected efforts.

Queued matching should be presented as expected progress, not as an error.

Older activities uploaded before route-point persistence or before a parser fix may need activity regeneration before they can match newly imported segments.

## Code Anchors

- Segment API and duplicate detection: `api/src/controllers/segments.rs`
- Route matching helpers: `api/src/segment_support.rs`
- Segment analytics: `api/src/analytics.rs`
- Worker regeneration task: `worker/src/tasks/processors/regenerate_segment_efforts.rs`
- Segment list UI: `ui-next/components/SegmentsPanel.tsx`
- Segment detail UI: `ui-next/components/SegmentDetailPanel.tsx`
- Segment builder UI: `ui-next/components/SegmentBuilderWorkspace.tsx`

## Open Gaps

- Poll and display full queued/running/succeeded/failed task state for segment processing.
- Page worker-side candidate activity scans.
- Query only required activity columns in all worker-side matching paths.
- Consider a compact stored dedupe key for segment routes.
- Add handler-level tests proving upload and builder paths enqueue matching instead of executing it inline.
- Add worker tests for regeneration and analytics completion.
- Add UI tests for queued/running/completed states.
