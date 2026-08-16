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

## To Convert

The following current code paths use TCX as canonical data or lose source fidelity and need conversion before Bike is fully FIT-first:

- `api/src/strava.rs`: `process_strava_sync` calls `get_activity_streams`, then `build_activity_upload`, which creates a `.tcx` filename, sets `format = "tcx"`, sets a TCX MIME type, and passes generated XML into `persist_activity_upload`. This makes a lossy synthetic TCX look like the retained source.
- `api/src/strava.rs`: `get_activity_streams` currently requests only `time,distance,latlng,altitude,velocity_smooth,heartrate,cadence,watts`. It omits other available Strava streams such as `temp`, `moving`, and `grade_smooth`, and does not retain the raw stream payload, stream `original_size`, or stream `resolution`.
- `api/src/strava.rs`: `build_tcx_document` serializes Strava summary and stream data into `TrainingCenterDatabase` XML. This flattens provider payloads into TCX, drops fields that have no TCX mapping, interpolates elapsed time/distance when streams are missing, creates a single lap, and cannot represent FIT-only data such as event records, developer fields, MTB dynamics, detailed device metadata, or full session/lap semantics.
- `api/src/activity_import_pipeline.rs`: `persist_activity_upload`, `store_activity_upload_import`, `process_stored_activity_import`, and `reprocess_activity_from_import` trust `activity_imports.format` and `activity_imports.storage_path` as the replayable source. For Strava imports those values point at generated TCX, so reprocessing and "source file" download replay the lossy artifact instead of provider raw data or FIT.
- `api/src/controllers/activities.rs`: `download_activity_source_file` returns the bytes from `activity_imports.storage_path` with `activity_import.original_filename`. For Strava-synced rides this currently downloads generated TCX while the UI labels the action "Download source file".
- `migration/src/m20260512_000014_add_activity_source_correlation_id.rs`: historical Strava correlation backfill extracts IDs from `.tcx` filenames such as `Morning_Mountain_Bike_Ride_18468904796.tcx`. Future data should store provider correlation IDs independently from filenames and formats.
- `api/src/activity_details.rs` and `api/src/fit_support.rs`: FIT parsing exists, but the normalized derived data currently keeps common route/chart/lap fields only: distance, elevation, speed, heart rate, cadence, power, calories, ascent/descent, and timing. FIT fields for grit, flow, jumps, hang time, jump distance, event records, developer data, device metadata, and raw message coverage are not persisted in the normalized model.
- `api/src/activity_summary.rs` and `api/src/activity_details.rs`: TCX and GPX parsers remain valid compatibility parsers, but they cannot be used as the fallback representation for sources that were richer than TCX/GPX.
- `api/src/archive_import.rs`: archive import correctly stores original supported entry bytes after gzip decoding, but it treats FIT, TCX, and GPX as peers. It should prefer FIT when the same activity appears multiple times in an archive and should record source-quality metadata for duplicate cleanup.
- `ui-next/components/ActivityImportsPanel.tsx`, `ui-next/components/ActivityStream.tsx`, and related tests: upload copy presents FIT, TCX, and GPX as equal choices. UI copy should steer riders toward FIT for full telemetry and present TCX/GPX as fallback formats.
- `ui-next/components/activity-detail/ActivityHeaderActions.tsx` and `ui-next/components/__tests__/ActivityDetailPanel.test.tsx`: "Download source file" assumes the retained import is original. The UI should distinguish original FIT/source, provider stream archive, and generated compatibility exports.
- `api/src/controllers/segments.rs`, `ui-next/components/SegmentsPanel.tsx`, and `docs/specs/segment-processing.md`: segment import is GPX/TCX-only. This is acceptable for route-only segment definitions, but it should stay separate from activity source fidelity and should not imply TCX is preferred for activity ingestion.

## FIT-First Plan

1. Add source artifact metadata.

   Extend persistence so an activity import can have one or more artifacts with fields for `artifact_kind` (`original`, `provider_payload`, `generated_export`), `format`, `storage_path`, `mime_type`, `size_bytes`, checksum, and fidelity ranking. Keep the existing `activity_imports.storage_path` as a migration bridge until callers are moved. Add explicit source-quality labels such as `fit_original`, `tcx_original`, `gpx_original`, `strava_streams`, and `generated_tcx`.

2. Preserve Strava raw provider data instead of pretending it is TCX.

   Change Strava sync to store the raw activity summary/detail payload and raw stream response as `provider_payload` artifacts. Request all useful public stream keys: `time`, `distance`, `latlng`, `altitude`, `velocity_smooth`, `heartrate`, `cadence`, `watts`, `temp`, `moving`, and `grade_smooth`. Treat generated TCX, if still needed for compatibility, as a derived export artifact that can be regenerated from the provider payload.

3. Prefer true FIT files wherever Bike can obtain them.

   Manual upload and archive import should preserve FIT bytes unchanged. Archive import and duplicate cleanup should rank duplicate sources by fidelity, with original FIT above original TCX, original GPX, Strava streams, and generated exports. A later Garmin integration should ingest official FIT files if available and should not rely on reverse-engineered Garmin endpoints.

4. Expand FIT parsing into a richer normalized model.

   Extend `fit_support.rs` to parse and retain session/lap/event/device/developer records and MTB fields including grit, flow, jump count, hang time, and jump distance when present. Extend `ActivityDerivedData` or add telemetry-specific tables for fields that do not belong in route/chart points. Keep unknown FIT/developer fields in a replayable raw-message sidecar if the typed model does not yet understand them.

5. Move import processing to a source-aware parser and graph executor.

   Import processing now has a graph-shaped executor backed by `petgraph`, but source selection is still format/import-row based. Replace format-only dispatch with artifact-aware dispatch: parse original FIT first, original TCX/GPX second, provider payloads third, and generated exports only as a last resort for legacy rows. Reprocessing should choose the richest available artifact and update normalized records without changing the original artifact.

6. Migrate legacy Strava synthetic TCX imports.

   Mark existing `strava_sync` imports whose `format = "tcx"` and filename ends with a Strava activity ID as `generated_tcx`/legacy. Preserve them for replay until a new Strava sync can fetch raw provider payloads. When possible, re-sync connected accounts to attach Strava raw summaries and streams to existing activities by `source_correlation_id`.

7. Fix source-file download semantics.

   Change the source-file endpoint to return the highest-fidelity original artifact by default. If no original file exists, return a clear 404 or a provider payload download endpoint, and expose generated TCX as a separate "Download generated TCX" action. The UI should not call synthetic TCX an original source file.

8. Rebuild downstream analytics from richer data.

   Once richer FIT/provider data is retained, reprocess affected activities, rebuild derived route/chart/lap data, segment efforts, activity analytics, training analysis, and fitness freshness. Add validation reports showing which activities are backed by original FIT, original TCX/GPX, Strava streams, or legacy generated TCX.

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
