# Project Overview Specification

Bike is a multi-user activity and training app for cycling data. It imports activity data, normalizes it into a Bike-owned model, matches user-owned segments, and produces deterministic training analytics for XC and DH riding.

## Product Boundary

The current Bike product includes:

- authenticated multi-user accounts;
- manual FIT, TCX, and GPX activity uploads;
- server-side archive imports from shareable export URLs;
- Strava OAuth sync, manual re-sync, and webhook ingestion;
- normalized activity list/detail views with telemetry, route, laps, zones, and matched segments where data exists;
- manual GPX/TCX segment import and activity-route segment builder flows;
- segment efforts, personal-best context, summaries, and comparison/race views;
- user-managed heart-rate zones, estimated FTP, and unit preferences;
- fitness/fatigue/form analytics;
- XC and DH training progress screens with deterministic recommendations;
- Garmin IQ device pairing and segment sync to the watch;
- admin metrics, backfill, task, and integration-event tooling.

The current product does not treat training plans, Garmin cloud activity sync, Garmin workout push, multi-target event history, or power-zone planning as shipped behavior. Those ideas may become future features, but specs should not describe them as current functionality until implementation lands.

## Architecture

Bike follows the repo shape used by the surrounding workspace:

- `api`: Rust Axum API for auth integration, preferences, Strava and Garmin IQ connections, activity import endpoints, activity/segment/training read APIs, and admin operations.
- `worker`: background processors for activity import, archive import, Strava sync, segment effort regeneration, analytics rebuilds, XC backfills, and email notifications.
- `migration`: SeaORM migrations for Bike-owned schema.
- `ui-next`: Next.js frontend for rider and admin workflows.
- `garmin-iq`: Garmin Connect IQ companion app and watch-facing sync docs.

Kaleido supplies shared scaffolding for auth, database wiring, background jobs, feature flags, metrics, and common UI/API patterns. Postgres is the normalized system of record.

## Data Strategy

Bike uses a vendor-neutral activity pipeline. A source may be a provider connection, uploaded file, archive entry, or future import path, but it should converge on normalized activity records and derived data.

The source data should remain replayable enough to support parser improvements. Raw FIT data is especially valuable because it can preserve richer telemetry than provider summaries, including heart rate, cadence, power, laps, and device metadata.

Derived outputs such as heart-rate zones, segment efforts, activity analytics, fitness/fatigue/form, and activity training analysis are caches or read models. They should be rebuilt when source activity or segment data changes.

## Integrations Strategy

Strava is the first live cloud activity integration. It uses OAuth with refresh tokens, imports through worker jobs, handles webhooks, records integration events, and respects user-scoped activity import locks.

Manual Garmin export import is a first-class ingestion path. Official Garmin cloud activity sync is additive if it becomes available; Bike should not depend on private or reverse-engineered Garmin Connect endpoints for core behavior.

Garmin IQ is a watch companion integration for pairing a device and syncing compact segment data to the watch. It is separate from Garmin cloud activity acquisition.

## Product Risks

Garmin cloud access can be uncertain, so manual FIT/TCX/GPX import must remain a durable path.

Strava rate limits should be handled with webhook-driven incremental sync, resumable worker jobs, and visible integration history.

Duplicate activities are likely when Garmin activities are mirrored into Strava. Deduplication must combine provider ids, file checksums, timing, distance, duration, and user scope.

Parser drift and upstream payload quirks are expected. Retaining replayable source data is the mitigation.

Segment correctness should favor transparent user-owned matching and personal comparison over trying to reproduce every Strava leaderboard edge case.

## Code Anchors

- App wiring: `api/src/lib.rs`, `api/src/controllers/mod.rs`
- Worker entry points: `worker/src/tasks/processors`
- Migrations: `migration/src`
- Frontend routes: `ui-next/app`
- Local tasks: `Taskfile.yml`
- Compose dev environment: `compose.yaml`

## Open Decisions

- Whether training plans become a first-class feature or remain outside the current product.
- Whether official Garmin cloud sync is worth adding if API access is approved.
- Whether power zones become first-class planning inputs when enough activities contain power data.
- Whether future segment leaderboards should remain personal/user-owned or introduce explicit cross-user competition semantics.
