# Personal activity heatmaps

Status: implementation authorized, 2026-10-03, for Rust and the shared UI.
Implementation status belongs in [the Bike backlog](../TODO.md#personal-heatmaps).

The [PostGIS evaluation](postgis-evaluation.md) supports deferring PostGIS.
Use ordinary PostgreSQL with compact coordinate projections and indexed
bounding-box filtering for this release. Retain the repeatable experiment for
future comparisons; adopting PostGIS is outside this implementation.

## Goal and scope

Add `/maps` to the shared `bike-ui` so a signed-in user can see everywhere they
have recorded activities. Use the supplied Strava screenshot as the visual
reference: continuous routes over a readable basemap, with frequently traveled
paths becoming more opaque. The 2026-10-03 visual feedback supersedes the original
thickness/palette proposal: thin, fixed-width lines default to the blue used by
Bike's light activity map, and frequency changes opacity only. Display color is
user-selectable through the later color customization below. This specifies Bike's behavior; it does
not assume knowledge of Strava's internal implementation.

The first implementation includes:

1. A database-backed feature flag with the exact key `heatmaps` in Rust,
   initially disabled.
2. One shared `/maps` page, navigation entry, activity-sport filter, date-range
   filter, map controls, legend, and loading/empty/error states.
3. Rust-owned route preparation, historical backfill, filtered heatmap APIs,
   aggregation, caching, and activity lifecycle integration.

The shared UI reads the feature flag before exposing the heatmap page. Enable
it only after activity preparation and the owning verification are complete.

Public/global heatmaps, sharing, route planning, segment clicks, per-path activity
drilldown, exports, road-network map matching, and a new map service are outside
the first release. Existing activity-detail and preview maps retain their behavior.

## Current code and constraints

The current repository provides these integration points:

| Concern                              | Existing owner                                                                                                                   |
| ------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------- |
| Public/admin flags                   | `bike-rs/bike-core/src/platform/feature_flags/`, `bike-ui/lib/featureFlags.ts`, `bike-ui/components/admin/FeatureFlagsPanel.tsx` |
| Flag migration examples              | `bike-rs/migration/src/m20260928_000001_activity_maps_feature_flag.rs`                                                           |
| Sport vocabulary and legacy aliases  | `bike-rs/bike-core/src/activity_sport.rs`, `bike-ui/lib/activitySports.ts`                                                       |
| Activity ownership and stored data   | `bike-rs/bike-core/src/entities/activities.rs`, `bike-rs/bike-core/src/activity_data.rs`                                         |
| Import/reprocessing                  | `bike-rs/bike-core/src/activity_import_pipeline.rs`, existing import/archive worker processors                                   |
| Deletion and duplicate cleanup       | `bike-rs/bike-core/src/activity_lifecycle.rs`                                                                                    |
| Durable background tasks             | `bike-rs/bike-core/src/jobs/`, `bike-rs/worker/src/tasks/`                                                                       |
| UI layout, auth, navigation, queries | `bike-ui/components/Layout.tsx`, `RequireAuth.tsx`, `Navigation.tsx`, `bike-ui/lib/queries.ts`                                   |
| Interactive maps and basemaps        | `bike-ui/components/MapLibreRouteMapClient.tsx`, `bike-ui/public/map-styles/`                                                    |
| Authenticated binary proxy precedent | `bike-ui/app/activity-map-images/[variant]/[styleVersion]/route.ts`                                                              |

Activities currently store complete route samples inside `derived_data_json`,
alongside other derived data. Heatmap requests must not deserialize every
historical activity's full samples or call every activity-detail endpoint.

Rust's local Compose uses `postgres:17`; the inspected migrations do not install
PostGIS. Prefer plain PostgreSQL and Rust processing initially. The backlog's
ordinary workload is at most about ten new activities per day, with occasional
historical imports. The history-derived fixture contains 1,189 activities and
4,854,345 route samples; the implementation measurements below describe its
complete backfill and tile reads.

## Product behavior

### Activity and date filters

1. Default to **All activities / All time**. Offer the existing sport choices:
   run, walk, hike, mountain bike, indoor trainer ride, and road ride. Reuse
   `ActivitySport` and its stored aliases; road ride includes legacy `ride`.
   All activities includes other stored sports when they have eligible routes.
   `activity_type` remains the separate Training/Race classification.
2. An eligible route has at least two distinct, valid real-world coordinates
   forming a continuous piece of track. Exclude GPS-free activities and simulated
   indoor/virtual routes. Recommended first-release policy: indoor trainer rides
   contribute no geography even when a provider supplies virtual coordinates;
   their filter displays an explanatory empty state.
3. Offer All time, This year, Last 90 days, and Custom start/end dates. Custom
   bounds may be open-ended. Resolve relative presets to explicit dates when
   selected so a saved URL is reproducible.
4. Dates include activities by **activity start time**, not individual GPS-point
   timestamps. An overnight activity contributes its entire eligible route when
   its start falls in the range.
5. Show inclusive calendar dates in a visible timezone, defaulting to the browser
   timezone. Persist its IANA identifier in the URL. Convert start-of-day and
   start-of-the-day-after-end to RFC 3339 instants; API predicates are
   `started_at >= from` and `started_at < to`. Use timezone-aware calendar
   arithmetic across daylight-saving changes, rather than adding 24 hours.
6. Persist `sport`, `start`, `end`, and `tz` in the URL; optionally persist a
   validated center/zoom after map movement settles. Back/forward restores the
   view. Invalid dates, unknown sports, or reversed ranges show a filter error
   and issue no heatmap request. The API also validates inputs independently.

### Counting and appearance

The unit of heat is a **distinct stored activity covering an area of a path**.
Ten laps in one activity count once at that location. Ten separate activities
count ten times. Sampling frequency, GPS-point count, elapsed time, direction,
and repeated import attempts must not increase the weight. Existing activity
deduplication remains responsible for identifying duplicate source activities.

Exact coordinate equality does not identify the same path: GPS recordings vary.
The proposed renderer measures coverage on a small spatial grid with a bounded
GPS tolerance. It is an approximation of geographic overlap, not a canonical
road/trail identifier. Nearby parallel paths and crossings can merge at low zoom;
they should separate at useful local zoom wherever the GPS data allows.

Render zero coverage as transparent. A single activity remains visible; repeated
coverage increases opacity only. Use a fixed one-pixel centerline in each
512-pixel tile, colored `#0060df` on both themes. Legend buckets are
**1 / 2–4 / 5–9 / 10–24 / 25+**, with alpha **100 / 155 / 195 / 225 / 255**.
Twenty-five or more activities are fully opaque; additional activity counts do
not widen or recolor the stroke. Keep legend samples at the same thin width.
The browser can recolor the base blue tiles using a selected display palette;
this does not change the count-to-alpha mapping or painted geometry.

Keep the scale fixed for a given zoom/style version across tiles and filters.
Do not normalize each tile independently or make a path hotter just because a
neighboring busy path left the viewport. Counts are exact for the chosen coverage
grid; counting tolerance does not add painted visits or stroke width.

### Page layout and map lifecycle

Use existing `Layout` and `RequireAuth`. Add a Maps navigation item when signed
in and `heatmaps` is enabled. The page has compact filter controls above a large
map, a collapsible control panel on mobile, a frequency legend, and a summary of
mapped activities and preparation progress. Use the existing Route light/Fiord
basemaps, attribution, theme behavior, pan/zoom, and a **Fit activities** control.

Fit eligible filtered bounds on first load if no camera is saved. Preserve the
camera during filter changes; fitting again is explicit. A filter with no routes
shows an empty message and keeps a usable map. Far-apart trips still appear when
the user fits all activities.

Reuse basemap and lifecycle helpers where appropriate, but build a dedicated
heatmap component rather than adding history/aggregation concerns to the
single-activity map component. Dynamically load MapLibre in the browser. Preserve
the map instance and camera on theme changes with `map.setStyle()`; reattach the
heatmap source/layer after the new style loads. Place heat beneath readable
basemap labels where supported. Clean up sources, events, requests, and the map.

Required states: flag loading/error, disabled feature, preparing history, ready,
partial preparation, no eligible routes, no filter matches, tile/API failure,
expired authentication, and unavailable WebGL. Distinguish no routes in the
selected dates from routes outside the current viewport. Keep previous data only
while clearly labeled as updating; once filters change, never present old heat as
the new result. Provide Retry for recoverable errors. Controls need accessible
labels, keyboard operation, visible focus, and a textual summary.

### Heatmap request and data flow

1. **Open and filter.** The signed-in rider's `/maps` page checks the `heatmaps`
   feature flag. The UI keeps sport and date filters in the URL and requests Rust
   metadata for that owner and filter set. The API returns preparation counts,
   a data revision, and bounds for ready routes. Viewport movement does not
   change these counts.
2. **Prepare routes.** The migration creates one projection status row for each
   existing activity. A database trigger marks an activity's projection pending
   when its route, sport, start time, source, or owner changes. When the feature
   is enabled, a reconciler leases up to 16 pending activities every 15 seconds
   and queues one durable job containing their activity IDs and generations.
   The job carries identities only, not route samples. The worker prepares its
   entries sequentially: it reads one activity's stored `derived_data_json`,
   checks route eligibility, simplifies the route for four zoom bands, and
   stores bounded coordinate chunks with spatial bounds in PostgreSQL. A usable
   route becomes `ready`; missing or excluded geometry becomes `skipped`; a
   preparation error becomes `failed`. Original activity data remains the
   source of truth.
3. **Draw the map.** For zoomed-out views, the Rust zones endpoint returns one
   center per ready activity for MapLibre to cluster. For visible route detail,
   the UI requests private PNG tiles through its server route. Rust filters
   projection chunks by owner, sport, date, zoom band, and tile bounds, then
   reads them in pages of at most 64 chunks. The renderer rasterizes each
   activity's coverage once before combining activities into per-pixel visit
   counts, so repeated loops within one ride count once. It returns transparent
   512-pixel tiles for MapLibre to overlay on the basemap. Heatmap tiles use this
   Rust path; the separate map renderer serves route-preview images.
4. **Keep results current.** Publishing a projection or changing/deleting an
   activity advances that owner's data revision. Tile and zone requests must
   present the revision returned by metadata, so stale results are rejected and
   the UI can refresh. Database ownership and API authentication scope results
   to one user. Tile cache keys include owner, revision, filters, tile, and style.

### Reading preparation status and resource use

The summary below the map comes from the authenticated user's metadata request
and counts activities matching the selected sport/date filters. “148 activities
with routes · Preparing 1,141 activities” means 148 matching projections are
ready with routes and 1,141 are still pending. These are activity counts, not
route-segment or GPS-point counts. Pending rows have not been checked for usable
geometry, so some may finish as skipped. The UI reports failures separately;
skipped activities are not included in the displayed ready or pending counts.
“Preparing” describes the pending status, not the number of jobs currently
running: pending rows may still be waiting for the reconciler to queue them.
One queued job can represent up to 16 pending activities, so the job count is
lower than the number of activities being prepared.

Opening `/maps` does not download all route history into browser memory. A
worker job carries only activity IDs and generations, then prepares its batch
sequentially while holding one activity's route at a time. This keeps batch
size from multiplying route memory. A backlog increases wait time and eventual
database storage; preparation memory depends on the largest individual route
and the number of worker replicas processing jobs concurrently. At overview
zoom, the API also materializes one small center point per ready activity for
clustering; that response grows with the owner's route count but does not include
full routes.
Tile rendering is capped at two concurrent renders per API process, with a
10-second deadline and a 64 MiB per-process tile cache. These limits do not
establish a multi-user capacity guarantee.

The local history backfill below processed 1,189 activities one at a time; its
19.27 MiB memory measurement is for tile requests, not the worker backfill. Peak
worker memory and concurrent multi-user load have not been measured. Production
deployment and backfill verification remain open in MAPS08.

## Recommended architecture and performance strategy

Build denormalized route projections asynchronously from durable dirty records.
Keep the original activity/source as truth, and make the heatmap projection
disposable and rebuildable. Prepare reusable geometry and indexed bounds once
per activity; aggregate only the requested tiles and filters. Activity import
does not wait for heatmap preparation. Do not pre-render images for every
possible date/sport combination.

Recommend **transparent raster overlay tiles for the first release**, generated
in Rust over the existing MapLibre basemap. The browser receives bounded images,
and a per-activity binary coverage mask gives a direct way to count a looping
activity once. This is an engineering recommendation, subject to a small visual
and storage/latency spike before selecting the final projection layout.

| Strategy                                                                | Strength                                                                                              | Cost or limitation                                                                                                                   | Decision                                                                                       |
| ----------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------- |
| Download all raw routes and overlay them in the browser                 | Small prototype                                                                                       | Full-history payload/memory, repeated parsing, overlapping opacity saturates, loops overcount, no explicit frequency-based width     | Reject for the production page                                                                 |
| Prepared geometry, indexed spatial bounds, on-demand raster aggregation | Arbitrary dates, bounded browser payload, direct per-activity coverage counts, no new spatial service | Cold dense tiles still require processing their complete filtered contributors; raster detail depends on resolution                  | Selected baseline                                                                              |
| Persist compressed per-activity coverage masks                          | Reuses rasterization and makes aggregation cheaper                                                    | More projection storage; zoom/tolerance/version choices must be managed                                                              | Add for measured expensive tiles/zoom levels if baseline misses targets                        |
| Daily sport/tile count rollups                                          | Faster long-range queries                                                                             | Extra rebuild/deletion bookkeeping and boundary-day handling                                                                         | Later optimization when history measurements justify it                                        |
| Weighted vector tiles                                                   | Crisp lines; future hit testing; MapLibre can style width/opacity from counts                         | Requires a credible common-path/edge aggregation algorithm; snapping can distort trails, raw independent lines do not solve counting | Alternative if the raster spike fails visual requirements or path interaction becomes required |
| PostGIS and a tile server                                               | Spatial querying and database MVT generation                                                          | Database-image/extension/backup/deployment changes; does not itself solve GPS alignment or distinct-activity aggregation             | Defer pending a demonstrated need                                                              |

MapLibre's [large-data guidance](https://maplibre.org/maplibre-gl-js/docs/guides/large-data/)
describes simplification, smaller payloads, and server tiling. Its
[source specification](https://maplibre.org/maplibre-style-spec/sources/#raster)
supports raster tiles directly. Its
[heatmap weights](https://maplibre.org/maplibre-style-spec/layers/#heatmap-weight)
apply to individual points. From that behavior, feeding raw GPS samples into
the built-in heatmap would weight recording density instead of distinct
activities. These capabilities support the design choices above; they do not
establish Bike's actual performance.

### Asynchronous projection preparation

Use the existing durable Rust worker. Saving or changing relevant activity data
records a dirty projection durably in the same transaction as the source
mutation. A reconciler leases up to 16 pending `(activity_id, generation)`
identities and inserts one durable task for the batch every 15 seconds; queue
payloads do not include route samples. Activity persistence and import success
do not wait for heatmap preparation. Reconciliation recovers dirty records if a
process stops between the source mutation and job execution.

The worker processes one activity at a time within each batch:

1. Load the owned activity's complete ordered route once. Validate coordinate
   ranges/finite values, remove duplicate adjacent points, and preserve track
   breaks. Audit whether current stored routes preserve source breaks; use
   elapsed-time/distance checks to split discontinuities rather than drawing
   bridges over missing GPS, source track boundaries, or implausible jumps.
   If source break information is required but absent, recover it from the
   retained source during projection preparation, not during tile requests.
2. Split antimeridian crossings and handle Web Mercator latitude limits without
   introducing world-spanning lines. Preserve unchanged analytical route samples.
3. Create compact coordinate-only geometry at a few zoom/detail levels. Simplify
   by a bounded screen-space error, rather than dropping every Nth sample.
4. Partition geometry into spatial chunks with indexed bounding boxes and a
   halo covering simplification error, GPS tolerance, and maximum painted width.
   Avoid indexing every tile inside a large route bounding rectangle.
5. Atomically replace the activity's projection and publish its revision only
   if the source version still matches. A concurrent edit or deletion makes a
   stale worker result discardable. Release full samples before the next activity.

If one activity fails, record its failed projection and continue through the
rest of the batch. Return a task error after processing the batch so the durable
worker retries it; already published or stale entries safely become no-ops on
retry because publication is generation-checked.

Selected physical records:

| Record                | Required contents and invariants                                                                                                                                                                         |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `heatmap_projections` | Activity/owner, desired generation, pending/ready/skipped/failed status, projection version, queue lease and ready-route bounds. Source changes immediately hide the previous published chunks.          |
| `heatmap_chunks`      | One current generation, zoom band and chunk index; up to 256 normalized Web Mercator points encoded as little-endian float64 pairs. Publication replaces all chunks atomically under a generation guard. |
| `heatmap_user_states` | Owner and monotonic dataset revision. Source triggers and successful publication invalidate old tile URLs, including deleted activities.                                                                 |

Native PostgreSQL `box` values and a GiST expression index implement chunk
bounding-box overlap without an extension or tile-membership table. Four bands
cover zooms 0–7, 8–11, 12–15 and 16–18; simplification error is at most one-quarter
pixel at each band's highest zoom. Tile reads join owned activities for filters
and fetch at most 64 compact chunks per page, ordered by activity so a single
coverage mask spans all of its chunks. They never load `derived_data_json`.

The counting grid adapts to latitude and zoom to retain approximately five
meters of GPS tolerance at detailed zooms. Original pixel centerlines are
painted separately, so GPS tolerance does not become a painted halo. The gutter
is at least 16 pixels and expands to cover that counting tolerance and the
maximum painted width. No road-network snapping is performed.

Join current `activities` for sport/date/ownership eligibility instead of copying
mutable filter values into every geometry chunk. Candidate indexes include
`activities(user_id, started_at, id)` and
`activities(user_id, sport, started_at, id)`. Choose chunk sizes/detail bands and
confirm index plans with the actual corpus before fixing schema/index counts.
Foreign keys/cascades remove projections when an activity or user is deleted.

Source versions cover route geometry, real-world eligibility, sport/start time,
and algorithm version. A title or Training/Race edit alone need not rebuild heat.
Retries replace the same projection; they never append another contribution.
Duplicate imports contribute through the retained canonical activity only.
Reprocessing and duplicate cleanup must invalidate old geometry. Future sport,
date, or privacy edits must use the same invalidation boundary.

Cheap dirty-state bookkeeping runs in Rust even when the flag is off. Normal
projection jobs can be paused while disabled; an explicit admin backfill can
prepare data before launch. Turning the feature on reconciles pending records.
The flag controls availability, not whether deletion/invalidation stays correct.

### Tile-time aggregation

1. Authenticate, check the flag and requested dataset revision, and normalize
   filters before cache lookup or database reads. Select only owned, eligible,
   current projection chunks intersecting the buffered tile. Apply sport/date
   predicates in SQL before materialization. Do not use activity-list pagination.
2. Stream/page candidates grouped by activity. Rasterize all that activity's
   intersecting chunks into one binary mask, including its repeated laps; OR the
   chunks before adding the mask once to the tile's integer count field.
3. Rasterize continuous segments, not isolated GPS samples. Start the spike with
   512-pixel XYZ tiles, local zooms 12–18, and approximately 5–10 meters of GPS
   tolerance at local zoom. Define the tolerance in ground meters and convert
   using latitude/zoom; do not treat projected meters as constant ground distance.
   Reduce detail and choose an appropriate screen-space tolerance at overview
   zooms 0–11. Publish supported zooms, tile size, and algorithm version explicitly.
4. Map counts to alpha on fixed-width blue centerlines. Counting tolerance stays
   separate from painted geometry; it must not widen the stroke. Process the tile
   gutter before cropping so adjacent tiles have no seams. Repeated paths must
   become more opaque without increasing painted pixel coverage.
5. Encode a transparent PNG, recheck the revision before publishing/caching, and
   return it. An empty successful tile is a transparent PNG. Failed/over-budget
   work returns a retryable error, never a misleading empty tile.

Overview counts are computed from each activity's coverage at that overview
resolution. Summing fine-grid cells into a coarse cell would count one activity
many times. Similarly, future daily rollups must use the same per-activity masks,
grid, tolerance, and zoom as a direct query. Sum complete UTC-day rollups and
calculate partial boundary days from activities for arbitrary timestamp ranges.
Do not subtract contributions from painted/colorized images.

### Caching and resource bounds

Cache tiles by app/database scope, authenticated owner, dataset revision,
normalized sport/from/to, z/x/y, resolution, and algorithm/palette version. Keep
credentials out of keys. Use a byte-bounded in-process LRU first, bounded render
concurrency, and coalesce simultaneous misses for the same tile. There is no
initial requirement for Redis, another service, or persistent rendered-tile storage.

Browser/proxy responses are `private, no-cache`, vary on Cookie/Authorization,
and use ETags. Authenticate, check flag/revision, and authorize even when replying
304 or serving cached content. Disable shared CDN caching for private overlays.
Keep Next fetches uncached and strip upstream cache headers that could make them
public. Logout/user changes remove the map and cached user queries.

An activity change invalidates the owner's revision immediately, including
deletion while preparation is pending. Do not serve stale projection generations
for changed activities. Projection publication advances the revision again.
Previously issued tile URLs with an old revision return 409 so the UI refreshes
metadata and switches the source; they cannot bypass a later deletion/flag disable.
Clients handle revision races with one coalesced metadata refresh, avoiding a
refresh loop per failing tile.

Keep output dimensions and render buffers fixed; page geometry rows and process
one activity at a time. Complete filtered history contributes, including an
all-time overview; do not silently truncate after an arbitrary activity limit.
Enforce a measured deadline and concurrency cap. If dense cold tiles exceed it,
use prepared masks/rollups at that zoom or return an explicit retryable failure
until the optimization is available. A deadline is not a way to call partial
results complete.

## Proposed API and UI wiring

Rust controllers remain HTTP adapters. A heatmap service owns orchestration and
policy; projection/entity methods own database queries and atomic publication.
Geometry/raster behavior belongs to named domain types/functions. Extend the
existing job system rather than introducing another queue.

| Endpoint                                                                 | Purpose and response                                                                                                                                                                                                                                                        |
| ------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `GET /api/maps/heatmap?sport=&from=&to=`                                 | Small JSON metadata response in the existing API envelope: normalized filters, dataset revision, filtered eligible/ready/pending/failed activity counts, nullable ready-route bounds, preparation state, supported zooms/tile size, legend/style version, tile URL template |
| `GET /api/maps/heatmap/tiles/{z}/{x}/{y}.png?sport=&from=&to=&revision=` | Owner-private transparent raster overlay for the same normalized filters and current revision; `image/png` success, ETag, bounded bytes                                                                                                                                     |

Omitted sport/from/to mean all sports/open bounds. Unknown sports, invalid
timestamps, `from >= to`, unsupported zoom, out-of-range tile coordinates, and
malformed revisions return the existing structured 400 error. Authentication
failure returns 401; authenticated access with `heatmaps` disabled returns 404.
A stale revision returns 409. Temporary preparation/render failures return 503
with Retry-After where useful. Metadata can describe a partially prepared dataset
with 200; tiles contain ready contributions and the UI clearly labels partial
coverage. Fully unprepared history shows preparing, rather than an empty success.

Counts describe the full selected date/sport corpus, not activity-list pagination
or the current viewport. Track ineligible/GPS-free activities as completed,
skipped projections. Pending records have unknown route eligibility until
prepared; do not claim an exact eligible total while those records remain.
Report skipped counts when needed for the empty-state explanation. Bounds come
from ready projections and handle wrapped longitude without zooming out to the
whole world unnecessarily.

Use React Query and the generated client for metadata. Include authenticated
user identity and normalized filters in query keys and clear caches on logout.
Poll metadata only while preparation is pending or when returning to the page;
normal refresh and revision handling discover newer imports. The map's tile
loader manages visible-tile requests, cancellation, and its bounded tile cache;
individual tile bytes need not live in React state or React Query.

Serve tiles through a same-origin Next route such as
`/heatmap-tiles/{z}/{x}/{y}` which forwards session/Bearer credentials and trace
context to the configured API and preserves private cache semantics. It performs
no aggregation and calls no activity-detail endpoint. Keep the API origin
configured server-side. Never place tokens or a selectable user ID in tile URLs.
The API derives ownership from auth; a client-supplied revision is not permission.

Register Rust Utoipa endpoints/schemas and regenerate the canonical OpenAPI plus
the shared TypeScript client with the existing mise tasks. Keep the Rust source-route inventory and distributed contracts current.
Do not add contract-harness dependencies to component CI/deployment.

## Implementation detail

Implementation status, priority, and rollout ownership live only in the
[Bike TODO](../TODO.md#personal-heatmaps). This specification holds the scope,
architecture, rendering and lifecycle constraints, and acceptance criteria for
those entries.

## Verification and rollout criteria

Use existing owning suites and the UI browser tests. Prefer a small fixture
with two shared-route activities, one repeated loop, a neighboring path, and a
different owner/date/sport, plus one original ride for the projection happy path.

1. Counting: one looping/reversed/densely sampled activity produces weight one;
   two separate overlapping activities produce weight two. Retry/reimport does
   not increase heat. Frequency visibly changes opacity, with constant width/color
   and identical painted coverage for repeated identical geometry.
2. Filtering: sport aliases/subtypes and inclusive date UX produce the exact
   selected activities, including timezone/DST and an overnight start-time case.
   Missing/simulated GPS contributes no line; date/sport filters apply before
   geometry loading.
3. Ownership and cache: another owner's routes cannot appear in metadata, tiles,
   cache hits, or conditional responses. Disabled/unauthenticated access cannot
   read old tiles. Deleting/reprocessing an activity removes old contributions;
   a racing worker cannot resurrect them.
4. Preparation: activity import succeeds independently of heatmap preparation;
   dirty records survive retries/crashes; a bounded batch prepares activities
   sequentially, and historical backfill uses the same per-activity builder.
   Partial history is labeled and advances to ready after publication.
5. UI: shared navigation/page gates, filter URL restoration, fit/camera retention,
   theme switching and attribution work. Exercise a Rust-backed happy path with
   real tiles; use owning UI tests for loading/empty/error states.
6. Performance: measure current history and a proportionate larger fixture,
   including its busiest local tile and an all-time overview. Record activity/
   vertex counts, bytes read/returned, query plans, peak memory, cold/warm timing,
   worker preparation time/storage, and browser responsiveness. Proposed initial
   targets: metadata p95 <300 ms, warm tile p95 <200 ms, cold tile p95 <1 s,
   useful first overlay <2 s excluding basemap fetches, and no filter-driven
   main-thread pause >100 ms. These are targets, not measurements. A suggested
   starting cache cap is 64 MiB per API process; select render concurrency and
   input page sizes from measured memory. Record device/network and sample count.

Keep the flag off while shipping/backfilling. Backfill by scalar activity-ID
pages, one route at a time, with persisted progress and retryable failures. The
existing low-volume worker is sufficient unless measurements demonstrate
otherwise. Show pending/failed counts without making page requests parse history.
Enable the feature after the criteria pass; Disable the flag to roll
back availability, retaining original activities and recoverable projections.
Any required production storage/config/dashboard changes belong in Pulumi.

Use the existing worker/task logs and metrics for projection duration, pending/
failed work, tile generation latency, cache hit/bytes, and render failures.
Do not label metrics with user IDs, coordinates, date ranges, or tile keys, or
log private route geometry. Avoid adding new infrastructure for hypothetical load.

## Rendering design choices

The MAPS03 rendering spike in the [Bike TODO](../TODO.md#personal-heatmaps)
owns validation of these design choices: raster overlays use continuous thin
blue lines with progressive opacity; GPS tolerance and LOD preserve nearby
trails and tile edges; per-activity geometry meets cold-tile targets before
adding masks or rollups; indoor/virtual routes remain excluded; dates use
activity start time with an explicit timezone. Exact road identities and hover
counts remain outside the current scope.

## Implementation evidence, 2026-10-03

This section records local measurements, rather than production performance or
the deferred vanilla/PostGIS comparison. The isolated PostgreSQL 17 container
used the existing `history-1x` experiment fixture: 1,189 activity records with
4,854,345 route samples. Identifiers/metadata are synthetic; routes retain the
source history's recording density, repeat visits and travel distribution.
The private fixture and screenshot are ignored artifacts, not checked-in GPS data.

The explicit owner backfill prepared 1,118 routes and skipped 71 ineligible or
missing-GPS activities in 79.3 seconds using debug Rust. It holds one source
activity at a time and pages 16 scalar identities; a second run prepared zero.
A real worker subsequently prepared a changed activity at its new generation.
That temporary activity was deleted after the check. All native migrations
applied successfully to the disposable database.

Verification passed five geometry/raster tests, two real-PostgreSQL integration
tests, 20 focused UI/proxy tests, TypeScript, the Next production build, Rust
Clippy for core/API/worker and static contract/route checks. The Rust-backed
Playwright flow exercised real tiles, sport/date URL filters, private conditional
responses and the same MapLibre canvas across filters/themes. It passed with both
a fake basemap provider and the real OpenFreeMap basemap. A separate real-API
probe with local admin bypass disabled returned authenticated `200`/`304` and
anonymous `401`, including an anonymous request with a known ETag.

### Tile and storage measurements

Machine: Apple M1 Max, 32 GiB RAM, macOS; Rust 1.97.1 debug build; PostgreSQL 17
in Docker over localhost. `heatmap_performance` selects the world tile and the
centers of the fixture's two largest regions at zooms 7, 14 and 18. Each case
uses five fresh application-cache renders and five immediate cache hits.
PostgreSQL/OS caches remain warm. These are single-request medians, not p95,
concurrent load, browser startup timings or disk-cold measurements.

| Case                | Fresh application cache, median ms | Cache hit, median ms | PNG bytes |
| ------------------- | ---------------------------------: | -------------------: | --------: |
| World               |                            183.589 |                3.533 |     7,512 |
| Region R2, zoom 7   |                            165.081 |                4.348 |    19,650 |
| Region R2, zoom 14  |                            397.090 |                5.442 |    70,708 |
| Region R2, zoom 18  |                             68.742 |                4.826 |    31,623 |
| Region R15, zoom 7  |                             50.059 |                3.749 |     6,487 |
| Region R15, zoom 14 |                            212.435 |                4.430 |    20,801 |
| Region R15, zoom 18 |                            238.929 |                5.786 |    12,424 |

Native `/usr/bin/time -l` around the already-compiled diagnostic measured 7.80 s
wall time, 4.82 s user CPU, 0.11 s system CPU and 20,201,472 bytes maximum RSS
(19.27 MiB). This covers the diagnostic process's 35 fresh and 35 cached tile
requests plus metadata calls. It excludes compilation, the long-running API,
worker, browser and PostgreSQL process resources. A warmed in-process cache with
many entries can grow to the configured 64 MiB cache cap in addition to process
and render memory.

`pg_total_relation_size` after preparation reported:

| Record                           | Indexed relation bytes |
| -------------------------------- | ---------------------: |
| `activities` in this native copy |            215,007,232 |
| `heatmap_projections`            |                327,680 |
| `heatmap_chunks`                 |             99,418,112 |
| `heatmap_user_states`            |                106,496 |

There are 28,240 chunks containing 104,699,264 encoded coordinate bytes before
PostgreSQL compression/index overhead. The four bands contain respectively
1,140 / 2,063 / 8,988 / 16,049 chunks. Tile requests read bounded coordinate
pages and return 512-pixel PNGs; they do not fetch raw activity JSON. This native
activity copy is not a byte-for-byte replacement for the experiment's raw schema,
so these sizes should not be presented as its PostgreSQL/PostGIS storage comparison.

| Idea                                                                                                 | Test                                                                                                     | Result                                                                                                                                                                                                       |
| ---------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Approximate five-meter GPS tolerance on a coarse counting grid, while painting original centerlines. | Compare the same seven tile cases and add a regression for tracks three meters and fifteen meters apart. | The three-meter pair shares frequency without a wide painted halo; the fifteen-meter pair remains separate. Region R15 zoom 18 improved from 852.412 to 238.929 ms; suite user CPU fell from 7.83 to 4.82 s. |
| Keep MapLibre tile-template braces literal.                                                          | Real-browser tile requests through the shared proxy.                                                     | URL construction had encoded `{z}/{x}/{y}` and produced `400`; constructing the absolute template without encoding fixed actual `512×512` tile requests.                                                     |
| Give the map a sized wrapper despite global MapLibre CSS.                                            | Inspect the production-build screenshot.                                                                 | The map now occupies 65% of viewport height with a minimum height, instead of extending with its parent.                                                                                                     |

The panel now distinguishes loading and errors while checking feature availability
from a disabled feature. Two regressions cover request suppression and flag retry;
the browser flow waits for the actual feature-flag response before choosing its
enabled/disabled path, and can require the enabled flow explicitly.

The local evidence supports this initial implementation at current-history scale.
The 3x/concurrent renderer sweep, PostgreSQL CPU/RSS/query-plan profiling,
release-build p95 and instrumented browser responsiveness remain performance
work before claiming all proposed rollout budgets. PostGIS remains deferred.

### Repeating the focused checks

Use an isolated native Bike database and its owner ID. The migration and backfill
write only to the explicitly selected database; the tile diagnostic is read-only.
Backfill may run while `heatmaps` is disabled. Enable the flag through the existing
admin controls in the selected test database before running tile diagnostics.

```sh
# Run from bike; DATABASE_URL points to the chosen isolated database.
mise --cd bike-rs run migration:up
mise --cd bike-rs exec -- cargo run -p bike-core --example heatmap_backfill -- <user-id>
mise --cd bike-rs exec -- cargo run -p bike-core --example heatmap_performance -- <user-id> ../experiments/postgis/fixtures/private-cases.json

# Use the existing Playwright setup and running Rust API, worker and shared UI.
# Uses the fake basemap by default; does not reinstall dependencies on each run.
PLAYWRIGHT_TARGET=rust HEATMAP_REQUIRE_ENABLED=1 mise --cd bike-ui run test:e2e:heatmaps

# Optional real-provider visual check; the screenshot stays in ignored artifacts.
PLAYWRIGHT_TARGET=rust HEATMAP_REQUIRE_ENABLED=1 HEATMAP_REAL_BASEMAP=1 mise --cd bike-ui run test:e2e:heatmaps
```

Keep the existing [Compose experiment harness](../../experiments/postgis/README.md)
for repeatable fixture/resource comparisons and its idea/test/result record.
The new native tile diagnostic measures the implemented renderer separately;
it does not replace that harness or its reports. Production flags remain disabled.

### Rendering and zoom follow-up, 2026-10-03

User feedback replaces the original variable-width orange/red treatment with
fixed-width `#0060df` blue strokes and opacity-only frequency. The renderer is now
`heatmap-v2`; tile URLs include its style version so already loaded v1 images are
replaced. Prepared coordinate projections remain valid and need no rebuild.
The measurements above describe v1; this correction does not claim a new resource
comparison.

The disappearing overlay had a lifecycle bug: `isStyleLoaded()` becomes false
while zoom tiles are outstanding, but those requests do not cause another
`style.load` event. Waiting for that event stranded incoming dataset revisions.
The component now tracks actual style changes separately and applies changed
tile templates while tiles are loading. Unchanged metadata no longer calls
`setTiles()` and reloads the source. Raster tiles use a 300 ms crossfade while
keeping the existing map and camera.

Five Rust geometry/raster tests passed, including identical painted coverage for
one versus 150 repetitions, solid blue opacity, GPS tolerance and tile borders.
Twenty focused UI/helper/proxy tests passed, including four map lifecycle
regressions; TypeScript and Rust core/API Clippy passed. Two Rust-backed browser
checks passed on the user's Compose stack: real-basemap filters/themes/private
tiles, plus a fake-basemap zoom check that blocks new real overlay requests and
asserts existing blue pixels remain visible before releasing them. The native
local history finished preparing: 859 ready, 214 skipped, zero pending/failed.

Repeat both browser checks with a bounded known-route filter:

```sh
PLAYWRIGHT_TARGET=rust HEATMAP_REQUIRE_ENABLED=1 HEATMAP_REAL_BASEMAP=1 HEATMAP_PREVIEW_FILTERS='sport=road_ride&start=2026-08-01&end=2026-08-31&tz=America%2FDetroit' mise --cd bike-ui run test:e2e:heatmaps
```

The zoom check deliberately uses a fake basemap to ensure its pixel assertion
measures the real personal overlay. Without `HEATMAP_PREVIEW_FILTERS`, that
additional check is skipped; choose dates/sports with local routes for another
dataset. No production enablement or PostGIS change is part of this correction.

### Color customization, 2026-10-03

The compact toolbar includes a DaisyUI color picker with Orange, Sunset, Blue,
Pink, Purple, and Contrast presets. Blue remains the default. Selection is saved
in browser local storage and updates the legend along with the overlay. Storage
being unavailable does not prevent selection for the current visit.

The existing raster layer uses MapLibre's GPU hue, contrast, and brightness
properties to recolor already loaded tiles. Changing color preserves the camera,
source, count opacity, and fixed stroke width. It does not fetch another tile set
or add palette variants to backend caches. Theme changes restore the selected
color on the new raster layer. The legend uses the same color transform as the
raster shader to remain consistent with the displayed lines.

Eleven focused panel/map tests and TypeScript passed. A local Playwright check
rendered all six presets against the real Rust API and basemap, observed zero
heatmap tile requests during selection, restored the choice after a reload, and
reported no browser errors.

## Primary technical references

- [MapLibre large-data performance guide](https://maplibre.org/maplibre-gl-js/docs/guides/large-data/): simplification, small payloads, and server tiling.
- [MapLibre raster/vector source specification](https://maplibre.org/maplibre-style-spec/sources/): XYZ raster sources, bounds, tile sizes, and zoom limits.
- [MapLibre layer specification](https://maplibre.org/maplibre-style-spec/layers/): point heatmap weighting and count-driven line width/opacity for the vector alternative.
- [PostGIS ST_AsMVT](https://postgis.net/docs/ST_AsMVT.html): database vector-tile encoding, if a later measured need justifies PostGIS.

References reviewed on 2026-10-03. Rendering/aggregation policy and performance
targets in this document are proposals, not results from these references.
