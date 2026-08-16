# Activity Ingestion Specification

Bike imports activities from manual files, server-side archive jobs, and Strava sync. All ingestion paths should converge on the same normalized activity pipeline so derived metrics, segments, analytics, and UI behavior remain consistent regardless of source.

## Product Intent

The rider should be able to bring historical and new activity data into Bike without caring which provider or file type produced it. Imports may be slow, but the UI should show queued/running/completed state instead of requiring the browser request to stay open.

Raw inputs should be retained or traceable enough that parser improvements can replay previous imports without asking the rider to upload or reconnect again.

FIT is the gold-standard activity source format for Bike. FIT should be treated as the highest-fidelity source because it can preserve device-native records, developer fields, laps, sessions, events, MTB dynamics, and vendor/device metadata that TCX, GPX, and provider summary APIs either omit or flatten. TCX and GPX are compatibility inputs, not canonical storage targets. Any ingestion path that receives FIT bytes must retain those exact bytes. Any path that cannot get FIT bytes must retain the highest-fidelity raw provider payload it can get and label any generated TCX/GPX representation as a lossy compatibility artifact.

Mountain-bike telemetry such as grit, flow, jumps, hang time, jump distance, lap/session details, event records, and developer fields must not be discarded when the source format contains them. If the normalized Bike model does not yet expose a field, the import should still preserve enough source data to add that field later and reprocess existing activities.

## Supported Sources

Manual activity upload supports FIT, TCX, and GPX files. FIT should be encouraged first in UI copy and documentation. The upload request stores the original file bytes, creates an activity import record, queues worker processing, and returns accepted status with the import record.

Large Garmin Connect and Strava exports should use the archive import flow instead of the browser upload form. Archive import accepts a shareable HTTPS archive URL. The API creates an archive import job and the worker downloads and scans the ZIP server-side. Archives may contain `.fit`, `.tcx`, `.gpx`, gzip-wrapped activity entries such as `.fit.gz`, and nested ZIP parts such as Garmin Connect `DI-Connect-Uploaded-Files/*.zip`.

Strava sync imports activities through the connected Strava account. OAuth connection, manual re-sync, and webhook-triggered sync should feed the normal per-activity normalization path. Strava's public activity streams endpoint provides typed streams such as time, distance, lat/lng, altitude, velocity, heart rate, cadence, watts, temperature, moving, and grade; it is not an original FIT download endpoint. Strava stream ingestion is therefore provider-derived raw data, not equivalent to a device FIT file.

## Normalization

Every imported activity must pass through the same core processing graph. The implementation uses `petgraph` in `api/src/activity_import_pipeline.rs` to model the graph and runs it in topological order, rather than relying on ad hoc call order in each importer.

The current graph nodes are:

1. `raw_stored`: retain enough source metadata and file data to reprocess later;
2. `activity_parsed`: parse the retained source through the reusable activity parser entrypoint;
3. `activity_saved`: insert or update the normalized activity summary and derived detail;
4. `segments_built`: rebuild segment efforts from normalized route points;
5. `segment_analytics_built`: rebuild analytics affected by changed segment efforts;
6. `activity_analytics_built`: rebuild per-activity analytics;
7. `training_analysis_built`: rebuild training analysis from the normalized activity data.

The graph dependencies are encoded in code and validated by tests. Manual upload worker processing, Strava sync imports, archive imports, single-activity reprocessing, and user-level reprocessing should call the graph executor for actual activity data processing. Import-specific code may still handle discovery, download, decompression, authentication, locking, queueing, and provider event logging outside the graph, but it must not parse or normalize activity data with separate techniques.

The `segments_built` stage delegates to the segment processing contract. Activity ingestion owns the import DAG, artifact selection, source replay, and traceability; segment processing owns segment definitions, route matching, effort regeneration, and segment analytics.

The processing graph is observable. `GET /api/activity-imports/processing-graph` returns the canonical DAG as nodes, edges, and Mermaid `flowchart TD` text derived from the same graph definition used by the executor. `GET /api/activity-imports/{id}/trace` overlays a specific import's current stage and `activity_processing` integration events on that graph so operators can see which stages completed, failed, or remain pending.

Raw storage intentionally precedes activity parsing for all retained file imports. This means fingerprint duplicates can leave a duplicate import row that points at the duplicate raw source and the existing activity. Provider-correlation duplicates, such as already-seen Strava activity IDs, may still short-circuit before raw storage when no new source artifact would be retained.

The activity import pipeline is guarded by Clippy size and complexity lints. `activity_import_pipeline.rs` denies oversized functions, excessive argument lists, and excessive cognitive complexity, with thresholds configured in `clippy.toml`. Pipeline changes should split graph node behavior into named helpers instead of growing the executor match arms.

Provider-specific fields may be kept as metadata, but user-facing activity behavior should come from the normalized model.

The canonical processing order should be:

1. retain original FIT bytes when available;
2. retain original TCX/GPX bytes when FIT is unavailable;
3. retain provider raw JSON/streams when no activity file is available;
4. build normalized Bike summary/detail records from the richest retained source;
5. build generated export formats only as explicit derived artifacts.

The import schema should distinguish original source artifacts from generated compatibility artifacts. Downloading an activity source file should return the retained original source by default. If only a generated artifact exists, the API and UI should identify it as generated/lossy rather than "original".

Activity imports carry an explicit `import_version`. The version describes the replay contract for the import record and its retained artifacts, not the parser implementation version. Existing historical imports are version 1. New artifact-aware imports that can retain multiple source artifacts and parse native provider payloads are version 2. Backward-compatible parser improvements should keep the same import version; incompatible source-shape or replay-semantics changes must increment the version and preserve readers for older versions.

## Current FIT-First Implementation

Activity imports persist explicit source artifacts in `activity_import_artifacts`. The legacy `activity_imports.format`, `activity_imports.storage_path`, and related columns remain as a compatibility bridge, but processing and download behavior should prefer artifact metadata.

Manual upload and archive import store supported FIT/TCX/GPX bytes as `original` artifacts with source-quality labels such as `fit_original`, `tcx_original`, or `gpx_original`. Archive import sorts supported entries by fidelity before processing, so FIT representations are attempted before TCX and GPX when an archive contains duplicate representations.

Strava sync stores the raw provider summary and stream payload as a versioned `provider_payload` artifact with `source_quality = strava_streams`. The parser can normalize that retained provider payload directly into Bike summary/detail data. Strava still builds a generated TCX as a `generated_export` artifact with `source_quality = generated_tcx`, not as an original source, but that TCX is now a compatibility/export artifact and legacy fallback rather than the normal parsing input. Strava stream requests include `time`, `distance`, `latlng`, `altitude`, `velocity_smooth`, `heartrate`, `cadence`, `watts`, `temp`, `moving`, and `grade_smooth`, and retained stream payloads include Strava stream metadata such as `original_size`, `resolution`, and `series_type` when provided.

The activity-processing graph chooses the richest parsable artifact for normalization: original FIT first, original TCX/GPX next, Strava provider payload next, and generated TCX only as a last-resort compatibility artifact. The source-file endpoint returns retained original artifacts only. Provider payload and generated export downloads should remain separate concepts.

## Remaining Gaps

- `api/src/strava.rs`: `build_tcx_document` still serializes Strava summary and stream data into `TrainingCenterDatabase` XML as a compatibility export/fallback. This generated TCX is correctly labeled as lossy and should not regain priority over retained provider payload parsing.
- `api/src/activity_details.rs` and `api/src/fit_support.rs`: FIT parsing exists, but the normalized derived data currently keeps common route/chart/lap fields only: distance, elevation, speed, heart rate, cadence, power, calories, ascent/descent, and timing. FIT fields for grit, flow, jumps, hang time, jump distance, event records, developer data, device metadata, and raw message coverage are not persisted in the normalized model.
- `migration/src/m20260512_000014_add_activity_source_correlation_id.rs`: historical Strava correlation backfill extracts IDs from `.tcx` filenames such as `Morning_Mountain_Bike_Ride_18468904796.tcx`. Future data should store provider correlation IDs independently from filenames and formats.
- `ui-next/components/ActivityImportsPanel.tsx`, `ui-next/components/ActivityStream.tsx`, and related tests: upload copy presents FIT, TCX, and GPX as equal choices. UI copy should steer riders toward FIT for full telemetry and present TCX/GPX as fallback formats.
- `ui-next/components/activity-detail/ActivityHeaderActions.tsx` and `ui-next/components/__tests__/ActivityDetailPanel.test.tsx`: the detail action now says "Download original source", but the UI still does not expose provider-payload or generated-export downloads as separate actions.
- `api/src/controllers/segments.rs`, `ui-next/components/SegmentsPanel.tsx`, and `docs/specs/segment-processing.md`: segment import is GPX/TCX-only. This is acceptable for route-only segment definitions, but it should stay separate from activity source fidelity and should not imply TCX is preferred for activity ingestion.

## Native Strava Provider Parsing

Implemented in `api/src/strava_provider_payload.rs` and the artifact-aware parser path in `api/src/activity_parser.rs`.

1. Define a stored Strava provider payload shape.

   Done. `StoredStravaProviderPayload` includes a version, provider name, Strava activity ID, activity summary fields, streams keyed by stream type, and stream metadata such as `original_size`, `resolution`, and `series_type`.

2. Extend the parser interface to be artifact-aware.

   Done for the activity-processing graph. `parse_activity_artifact` dispatches between file artifacts (`fit`, `tcx`, `gpx`) and provider artifacts such as `strava_streams` without requiring provider data to masquerade as a file format.

3. Implement `parse_strava_provider_payload`.

   Done. The parser builds an `ActivityDraft` from Strava summary fields and `ActivityDerivedData` from Strava streams, including route points, chart points, and a full-activity lap. Stream arrays map directly without serializing through generated TCX first.

4. Change artifact selection priority once native parsing exists.

   Done. The activity-processing graph chooses artifacts in this order: original FIT, original TCX/GPX, Strava provider payload, and generated TCX only as a legacy fallback. Provider payload selection is covered by regression tests so generated TCX cannot regain priority over retained provider data.

5. Keep generated TCX as optional export or legacy fallback only.

   Done for parsing priority. New Strava imports still retain generated TCX as a compatibility export/fallback, but native provider payload parsing is the normal Strava path and generated TCX stays labeled as lossy/generated.

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

Every activity-processing graph stage should emit an `activity_processing` integration event with `event_type = stage_completed` and payload fields including `import_id`, `activity_id` when known, `source`, and `stage`. Terminal processed, duplicate, and failed outcomes should emit `import_processed`, `import_duplicate`, or `import_failed`. Tests should cover stage event emission so traceability does not silently regress.

## Code Anchors

- Upload and archive API: `api/src/controllers/activity_imports.rs`
- Activity import pipeline: `api/src/activity_import_pipeline.rs`
- Shared activity parser entrypoint: `api/src/activity_parser.rs`
- Archive importer: `api/src/archive_import.rs`
- FIT support: `api/src/fit_support.rs`
- Activity summary normalization: `api/src/activity_summary.rs`
- Activity detail normalization: `api/src/activity_details.rs`
- Source-file download: `api/src/controllers/activities.rs`
- Strava sync and synthetic TCX generation: `api/src/strava.rs`
- Worker processors: `worker/src/tasks/processors/process_activity_import.rs`, `worker/src/tasks/processors/activity_archive_import.rs`, `worker/src/tasks/processors/strava_sync.rs`
- Upload UI: `ui-next/components/ActivityImportsPanel.tsx`
- Activity source download UI: `ui-next/components/activity-detail/ActivityHeaderActions.tsx`

## Open Gaps

- Keep improving duplicate detection across mirrored Garmin and Strava sources.
- Keep large archive imports observable without overloading a single response or UI table.
- Preserve raw source replay as parser support grows.
- Decide whether Strava raw payload downloads should be exposed to riders or treated only as an internal replay artifact.
- Define the normalized storage shape for FIT developer fields and MTB dynamics before adding UI and analytics that depend on grit, flow, jumps, hang time, or jump distance.
