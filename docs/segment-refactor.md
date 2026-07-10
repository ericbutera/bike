# Segment Processing Refactor TODO

## Goal

Segment upload and segment-builder requests should stay fast and predictable. They should validate and persist the segment, enqueue background processing, return a task/job id, and let the UI show progress/completion from background task state.

Jobs may take a while. The system should favor simple, reliable, easy-to-reason-about processing over clever optimizations, as long as API requests and the UI remain crisp.

## Principles

- Upload handlers should not scan or match activities.
- CPU-heavy route matching should run in worker tasks.
- User-facing responses should include enough job/task information for progress messaging.
- Expensive jobs should be observable and restartable enough to debug safely.
- Use pagination and explicit projections where they keep the system simple.
- Do not introduce complex spatial indexing unless simpler batching and filtering are insufficient.
- Multiple worker instances in production are acceptable, so a single long task occupying one worker is not by itself a blocker.

## TODO

- [x] Make segment upload return immediately after persisting the segment.
  - Applies to `POST /api/segments`.
  - Do not call `replace_segment_efforts_for_segment` in the request path.
  - Enqueue a background task and return the created segment plus task id/status.

- [x] Move segment-builder effort regeneration out of API handlers.
  - Applies to `POST /api/segments/from-activity`.
  - Applies to `PUT /api/segments/{id}/from-activity`.
  - Persist route changes first, enqueue effort regeneration, and return without matching activities inline.

- [ ] Add API/UI task tracking for segment processing.
  - Return a task id when segment matching is queued.
  - Surface queued/running/succeeded/failed state in the segment UI.
  - Make incomplete analytics feel normal in the UI, not broken.
  - Progress: create/update responses now include the queued task id/status and upload/builder toasts say matching is queued. Polling/completion UI remains.

- [x] Simplify upload responses so they do not load all efforts.
  - Avoid `load_segment_response` immediately after upload if it expands efforts and route slices.
  - Return a lightweight segment response or add a dedicated response shape for queued processing.
  - Keep detailed effort loading on the segment detail page.

- [x] Scope segment effort regeneration to the segment owner.
  - Current `replace_segment_efforts_for_segment` scans all activities in the database.
  - It should at least filter activities by `user_id` for user-owned manual segments.
  - Keep cross-user leaderboard semantics explicit if they are ever needed later.
  - Progress: worker regeneration now passes the segment owner id and candidate activities are filtered by that user.

- [ ] Page through candidate activities in the worker.
  - Avoid `activities::Entity::find().all(db)` for regeneration.
  - Process activities in deterministic chunks, ordered by id or started_at/id.
  - Use a checkpoint such as `last_activity_id` if the job is split into multiple tasks.

- [ ] Query only needed activity columns during matching.
  - For matching, load `id`, `user_id`, and `derived_data_json` unless additional fields are required.
  - Avoid loading full activity rows only to deserialize route points.
  - Progress: segment detail and analytics helpers now project the activity fields they need. Worker-side matching still needs this cleanup.

- [x] Keep segment analytics rebuilds in the worker path.
  - Run analytics after effort regeneration completes.
  - Avoid rebuilding activity analytics inline in segment upload/builder handlers.
  - Prefer one simple final analytics rebuild task after matching.
  - Progress: segment create/update now queues regeneration; the worker rebuilds segment analytics and refreshes affected activity analytics after regeneration.

- [x] Reduce full-row analytics reads where simple.
  - `rebuild_segment_analytics_cache` currently loads full activity rows to read `started_at`.
  - Project only the fields needed for summaries.
  - Keep row-by-row rank updates acceptable for now unless profiling shows it is a bottleneck.
  - Progress: analytics now projects activity `id/started_at`, segment `id/title`, and personal-best summary fields instead of full rows where possible.

- [ ] Improve duplicate detection without loading every route blob.
  - Store a segment dedupe key on the segment row, or otherwise query a compact key.
  - Avoid loading every `route_data_json` for a user's segments during upload.
  - Keep current geometric duplicate behavior if possible.
  - Progress: duplicate detection now filters to the target segment distance bucket before reading route JSON, then loads the full segment only after a match. A stored dedupe key remains a possible future cleanup.

- [x] Keep list endpoints lightweight.
  - `GET /api/segments` should avoid selecting route blobs when it only returns summary data.
  - Use summaries for counts/best/latest data.
  - Progress: `GET /api/segments` now uses a lightweight projection and does not select `route_data_json`.

- [ ] Add focused tests around request responsiveness behavior.
  - Upload/builder tests should assert that matching is enqueued, not executed inline.
  - Worker tests should cover regeneration and analytics completion.
  - UI tests should cover queued/running/completed states.
  - Progress: existing segment tests and the segment upload UI test cover the queued messaging. Handler-level enqueue assertions and completion UI tests remain.

## Suggested Implementation Order

1. Add/return task id support for segment effort regeneration enqueueing.
2. Change segment upload and builder handlers to enqueue and return immediately.
3. Add UI status display for queued segment processing.
4. Scope and page worker-side activity scanning.
5. Make response/list/duplicate queries lighter.
6. Revisit analytics query projections after the request path is clean.

## Current Hot Spots

- `api/src/controllers/segments.rs`: upload and builder handlers call effort regeneration inline.
- `api/src/segment_support.rs`: `replace_segment_efforts_for_segment` loads all activities and deserializes all route data.
- `api/src/controllers/segments.rs`: `load_segment_response` expands all efforts and effort route slices.
- `api/src/analytics.rs`: segment analytics rebuilds load all segment efforts and full activity rows.
- `api/src/controllers/segments.rs`: duplicate detection loads all user segments including route data.
