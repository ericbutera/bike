# Activities specification

- [Activity list](activity-list.md) owns the recent activities feed, its cards,
  activity type filter, and page navigation.
- [Activity details](activity-details.md) owns the detail page, including matched
  segment colors, status dots, hover isolation, and persistent multiple selection.
- [Supported activities](supported-activities.md) owns the activity vocabulary
  and eligibility rules. List and detail visibility must follow that policy.

## Normalized recording storage

- Parsing retains its original immutable source through the existing import and
  object-store APIs. Save normalized recording context and the complete ordered
  chart and route samples as a versioned, content-addressed artifact.
- Preserve sample precision, missing sensor values and recording order. The
  artifact excludes editable metadata and laps; those remain in their existing
  storage until their owning cutover.
- `activity_recordings` holds one current reference per activity with typed
  `object_path`, `object_sha256`, `object_bytes` and `format_version` columns.
  Save that row in the same transaction as imported activity data, for retained
  files and native Strava imports. Repeating identical recording data reuses the
  artifact and reference; changed samples or recording evidence get a new reference.
- Recording objects live under a rider-specific namespace. The owning model
  authorizes reads through the activity owner before loading and verifying the
  artifact. Shared segments do not grant access to another rider's recording.
- Keep existing JSON consumers usable during conversion. Normalized reads return
  the saved artifact directly, without reparsing sources or rebuilding missing
  output. Missing output remains missing; malformed or unavailable objects fail.
- Run `backfill_activity_recordings` on `bike-tasks` with JSON input `0` to
  convert existing saved recording data. Select missing outputs in ID pages of
  at most 16, await each write before advancing, and Continue-As-New after a full
  page. Use the same artifact producer as imports, lock the current owned activity,
  and preserve metadata, laps, efforts and awards. Missing legacy recording data
  is an error, while explicitly saved empty recordings are valid outputs.

## Typed metadata and laps

- P2 saves description, manual location, private notes and recorded gear ID/name
  in `activity_metadata`, keyed by activity. Existing activity summary columns
  continue to own title, sport and Training/Race.
- Save every lap in `activity_laps` with its original lap number, title, offsets,
  duration, distance, elevation, speed, sensor metrics and calories. A separate
  zero-based position preserves source order, even when lap numbers repeat.
- Retained-file imports, duplicate reconciliation and native Strava imports save
  these rows in the activity transaction. Manual metadata edits synchronize the
  typed metadata in the same transaction. Preserve existing import/edit precedence,
  optional values, empty text clears and gear identifiers.
- `backfill_activity_metadata` on `bike-tasks`, input `0`, pages missing metadata
  in groups of 16 and calls the same producer under the activity lock. Empty
  metadata/laps are valid saved outputs; backfill does not reparse, rematch,
  calculate awards or rewrite recordings.
- Model reads authorize the activity owner and return saved metadata and ordered
  laps. Missing output stays missing. B2 owns the API/UI read cutover; keep
  existing consumers usable until then, with no conversion or repair on GET.
