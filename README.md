# Bike Project

## Requirements

Garmin Connect + Strava clone:

- Sync Garmin Connect data & Strava data to this project's database
- Manual upload of activities: fit, tcx, and gpx formats
- Web UI
- API
- Worker

Feature set:

- Multi-user support
- Multi-activity support (bike, run, swim, etc)
- Segments
- Leaderboard
- Achievements
- Heart rate zones
- Power zones (if power meter data is available)
- Training plans (grab bag, etc)

Tech Stack:

- use rss project as a starting point for the web UI and API
- kaleido library (use scaffold template)
- postgres

Fitness:

- Cardiac drift
- Z2 endurance speed

Data:

- Use <https://www.doogal.co.uk/SegmentExplorer> to export segments

## Manual Segment Workflow

- For now, segments are imported manually. No Strava API keys or OAuth setup are required.
- Export a segment route as GPX or TCX. The easiest path today is to use Doogal Segment Explorer with the segment you care about and download a GPX export.
- Upload that GPX or TCX file from the Bike home page under the manual segment import panel.
- Bike stores the segment route, matches it against the route points already persisted on your uploaded activities, and builds a comparison page with repeated efforts.
- Older activities uploaded before route-point persistence or before a parser fix may need the regenerate action on the activity detail page before they can match newly imported segments.
- FIT remains supported for activity uploads, but segment imports currently require GPX or TCX because the manual segment flow needs explicit route coordinates.

## Bulk Archive Import

- Large Garmin Connect and Strava exports should not go through the browser upload form.
- Paste a shareable HTTPS export URL into the upload UI and Bike queues a worker task that fetches the ZIP server-side.
- The server-side archive importer scans `.zip` files for `.fit`, `.tcx`, `.gpx`, and gzip-wrapped activity entries like `.fit.gz`, including nested ZIP parts such as Garmin Connect `DI-Connect-Uploaded-Files/*.zip`, then runs the normal per-activity normalization flow.
- The upload UI shows recent archive-import jobs so riders can track `queued`, `running`, `succeeded`, and `failed` states without holding the original HTTP request open.
- Activity imports and manual segment uploads both deduplicate against existing user data before creating new rows.

---

## Implementation Plan

### 1. Project Shape

- Mirror the `rss` project layout: `api`, `worker`, `migration`, and `ui-next` under `bike/`.
- Reuse `kaleido` for shared app scaffolding where it still fits: auth, config, database wiring, job plumbing, and common UI/API patterns.
- Keep Postgres as the normalized system of record.
- Store raw provider payloads and uploaded activity files outside the hot relational path so they can be reprocessed later.
- Expose the usual human-facing commands through a repo-local `Taskfile.yml`: `ui-next:*`, `api:*`, `worker:*`, `migration:*`, `test`, `openapi:react-query`, and deploy tasks.

### 2. Core Architecture

- `api`: auth, user profiles, connected account management, activity/segment/leaderboard read APIs, and admin sync endpoints.
- `worker`: provider sync jobs, FIT/TCX/GPX parsing, stream normalization, segment matching, leaderboard recomputation, achievement evaluation, and training-plan generation.
- `migration`: schema for users, connected accounts, raw imports, normalized activities, streams, segments, leaderboards, and plans.
- `ui-next`: connected accounts, activity list/detail, segment pages, leaderboards, achievements, and training calendar/plan views.

### 3. Base Data Strategy

- Build the ingestion pipeline around a vendor-neutral flow: connect account or upload file, persist the raw payload or file, normalize into internal activity tables, then compute derived data such as zones, segment efforts, PRs, achievements, and leaderboards.
- Treat raw FIT data as the best canonical artifact when available because it preserves richer telemetry than summary APIs: power, cadence, heart rate, laps, and device metadata.
- Always keep both the raw source and normalized rows so parser improvements can replay old imports without asking users to reconnect or re-upload.
- Deduplicate across providers using `(provider, external_id)` first, then a fallback fingerprint on user, start time, duration, distance, and file checksum.

### 4. Garmin and Strava Acquisition Plan

- `Strava`: make this the first live integration. Use OAuth 2.0 with refresh tokens per user, backfill with `GET /athlete/activities`, hydrate details from activity, laps, streams, zones, gear, and segment endpoints, register webhooks early, and design the worker around Strava's rate limits with cursors and throttled jobs.
- `Garmin`: preferred path is the Garmin Connect Developer Program Activity API. If access is approved later, add Garmin Training API and Courses API support for pushing workouts, plans, and routes back to devices.
- `Garmin` must not block the MVP. Support manual Garmin FIT/TCX/GPX imports from day one, feeding the same raw-import and normalization pipeline as official provider syncs.
- Avoid making private or reverse-engineered Garmin Connect endpoints part of the core design because that will be brittle operationally.
- Practical rollout: phase 1 is Strava OAuth sync plus manual Garmin import; phase 2 adds official Garmin cloud sync if approval lands.

### 5. Data Model

- `users`
- `connected_accounts`: provider, provider athlete id, scopes, refresh/access token material, sync cursors, connection status
- `raw_imports`: provider, external id, raw JSON pointer, raw file pointer, checksum, import status, imported timestamp
- `activities`
- `activity_streams`: time, distance, lat/lng, altitude, speed, heart rate, cadence, power, temperature, moving flags
- `activity_laps`
- `gear`
- `segments`
- `segment_efforts`
- `athlete_zones`: heart rate and power definitions per user
- `leaderboard_entries`
- `achievements`
- `training_plans`, `plan_workouts`, `scheduled_workouts`

### 6. Delivery Phases

1. Foundation: scaffold from `rss` into `bike/api`, `bike/worker`, `bike/migration`, and `bike/ui-next`; strip RSS-specific code paths; add repo-local tasks and OpenAPI client generation; stand up auth, users, background jobs, and database migrations.
2. Ingestion foundation: build a provider abstraction around `connect`, `backfill`, `fetch detail`, `store raw`, `normalize`, and `reprocess`; implement FIT/TCX/GPX parser support first; implement Strava OAuth, token refresh, initial backfill, and webhook handling; add a manual Garmin upload/import UI and API.
3. Core read side: ship activity list and detail pages, charts for pace/speed, heart rate, cadence, power, and elevation, per-sport filters, gear association, and user zone definitions with per-activity summaries.
4. Segments and competition: add an internal segment model based on route geometry, extract segment efforts from activity streams, build per-segment leaderboards scoped by sport and activity type, and award achievements and PR badges.
5. Training features: add a training calendar, plan assignment, simple plan templates first, then more structured progressions, workout compliance, and load summaries; only add Garmin workout push if official Garmin API access exists.
6. Hardening: add resync tooling, raw replay, admin dashboards, privacy controls, delete and export flows, backfill benchmarks, concurrency tuning, and rate-limit monitoring.

### 7. MVP Cut

- Multi-user accounts
- Multi-activity support for ride, run, and swim
- Strava sync
- Garmin file import
- Activity detail with telemetry streams
- Basic internal segments and personal bests
- Heart rate and power zones
- Minimal leaderboards
- Training-plan storage without Garmin push

### 8. Main Risks and Mitigations

- Garmin access risk: treat manual FIT/TCX/GPX import as a first-class ingestion path so official Garmin sync is additive, not foundational.
- Strava rate limits: use webhooks, incremental cursors, throttled workers, and replayable jobs.
- Duplicate activities from Garmin-to-Strava mirroring: dedupe on provider ids first, then timestamps, distance, duration, and file checksum.
- Parser drift and bad upstream payloads: keep raw files and raw JSON to support reprocessing.
- Segment correctness: ship an internal leaderboard model first instead of trying to perfectly clone every Strava edge case on day one.

### 9. First Concrete Build Order

1. Clone the `rss` app shape into `bike`.
2. Add users, auth, and connected-accounts schema.
3. Add raw import storage and FIT parsing.
4. Add Strava OAuth, backfill, and webhooks.
5. Add manual Garmin upload/import.
6. Build activity list/detail UI.
7. Add derived metrics, zones, and segments.
8. Add achievements and leaderboards.
9. Add training plans.
10. Add official Garmin Connect sync if access is approved.
