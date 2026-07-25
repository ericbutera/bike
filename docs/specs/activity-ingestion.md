# Activity Ingestion Specification

Bike imports activities from manual files, server-side archive jobs, and Strava sync. All ingestion paths should converge on the same normalized activity pipeline so derived metrics, segments, analytics, and UI behavior remain consistent regardless of source.

## Product Intent

The rider should be able to bring historical and new activity data into Bike without caring which provider or file type produced it. Imports may be slow, but the UI should show queued/running/completed state instead of requiring the browser request to stay open.

Raw inputs should be retained or traceable enough that parser improvements can replay previous imports without asking the rider to upload or reconnect again.

## Supported Sources

Manual activity upload supports FIT, TCX, and GPX files. The upload request stores the file, creates an activity import record, queues worker processing, and returns accepted status with the import record.

Large Garmin Connect and Strava exports should use the archive import flow instead of the browser upload form. Archive import accepts a shareable HTTPS archive URL. The API creates an archive import job and the worker downloads and scans the ZIP server-side. Archives may contain `.fit`, `.tcx`, `.gpx`, gzip-wrapped activity entries such as `.fit.gz`, and nested ZIP parts such as Garmin Connect `DI-Connect-Uploaded-Files/*.zip`.

Strava sync imports activities through the connected Strava account. OAuth connection, manual re-sync, and webhook-triggered sync should feed the normal per-activity normalization path.

## Normalization

Every imported activity should pass through the same core flow:

1. accept or discover a raw activity source;
2. store enough source metadata and file data to reprocess later;
3. normalize activity summary fields;
4. derive route, chart, lap, zone, and telemetry detail when the input supports it;
5. rebuild dependent activity analytics and training analysis;
6. enqueue or refresh segment matching where needed.

Provider-specific fields may be kept as metadata, but user-facing activity behavior should come from the normalized model.

## Deduplication

Activity imports must deduplicate against existing user data before creating duplicate activities. Provider identifiers are preferred when available. File checksums and activity fingerprints based on user, start time, duration, and distance are fallback signals.

Duplicate handling should be source-aware. A Strava activity and a Garmin file that represent the same ride should not become two user-visible rides just because they arrived through different paths.

## Processing State

Long-running import work should be represented explicitly. The upload and import UI can disable new uploads while a reprocess, archive import, Strava sync, or other user-scoped activity job is active.

Manual upload locks are released quickly because the file upload request only queues worker work. Long-lived locks are reserved for background jobs that would conflict with another ingestion or reprocessing path.

Archive jobs expose `queued`, `running`, `succeeded`, and `failed` style state with counters for imported, duplicate, unsupported, skipped, and failed entries. Error samples should be short enough for UI display and debugging.

The upload UI should show recent archive-import jobs so the rider can track progress without holding the original HTTP request open.

## Failure Behavior

Invalid files should fail with field-level validation where possible. Empty uploads, unsupported extensions, malformed multipart data, and payloads above the upload limit should produce clear user-facing errors.

Worker failures should mark the import or archive job failed without losing the source record. A later reprocess path should be able to recover when the parser or source data issue is fixed.

## Code Anchors

- Upload and archive API: `api/src/controllers/activity_imports.rs`
- Activity import pipeline: `api/src/activity_import_pipeline.rs`
- Archive importer: `api/src/archive_import.rs`
- FIT support: `api/src/fit_support.rs`
- Activity summary normalization: `api/src/activity_summary.rs`
- Worker processors: `worker/src/tasks/processors/process_activity_import.rs`, `worker/src/tasks/processors/activity_archive_import.rs`, `worker/src/tasks/processors/strava_sync.rs`
- Upload UI: `ui-next/components/ActivityImportsPanel.tsx`

## Open Gaps

- Keep improving duplicate detection across mirrored Garmin and Strava sources.
- Keep large archive imports observable without overloading a single response or UI table.
- Preserve raw source replay as parser support grows.
