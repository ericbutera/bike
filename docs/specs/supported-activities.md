# Supported Activities Specification

Bike is a cycling platform. Recognizing a sport label, retaining an original,
processing a ride, and admitting its route to a heatmap are separate decisions.

This specification owns support tiers and the activity inventory. The
[ingestion specification](activity-ingestion.md) owns retention and processing;
the [heatmap specification](heatmaps.md) owns geographic
contribution. Task status stays in [the Bike backlog](../TODO.md).
The original inventory reflects inspected code on 2026-10-06. Summary retention
and central vocabulary were updated on 2026-10-14 and await rollout. Expansion
work below remains proposed; it is separate from retaining activity summaries.

## Support tiers

- **Supported cycling:** admitted to Bike's cycling domain and shared ride
  workflows through supported sources. Available telemetry is displayed;
  cycling segments, fitness, training, and reports apply according to their
  prerequisites. This does not promise every metric, dedicated subtype
  analytics, or heatmap contribution.
- **Semi-supported:** recognized by file parsing and existing list/detail
  surfaces, without a complete sport-specific product contract. Non-cycling
  activities are excluded from cycling segments, fitness, training, and reports.
  A visible filter does not establish full support or live sync.
- **Recognized only:** retain the summary, provider label, and available source;
  dedicated UI, metrics, and analytics remain incomplete.
- **Unsupported/unknown:** retain the summary, available input, and metadata
  without guessing cycling. Unknown labels do not establish product support.

`sport` identifies the discipline. `activity_type` remains the separate
Training/Race classification. Recording environment and recorder evidence are
independent of both. Indoor cycling is supported but contributes no real-world
heatmap geometry.

## Central activity vocabulary and retention

- Backend `ActivitySport` owns the filter vocabulary and cycling classification.
  `ActivityType` owns Training/Race. Alias normalization and database cycling
  query values live with `ActivitySport`; provider adapters reuse this policy.
- Utoipa exports both enums in OpenAPI. The existing client generator uses
  `openapi-typescript --enum-values` to produce `api.ts`, including string union
  types and runtime value arrays. Frontend filters and classification controls
  consume those arrays. Display labels/descriptions remain frontend concerns.
- The current filter vocabulary includes Run, Walk, Hike, Mountain bike,
  Indoor trainer ride, Road ride, Swim, and Workout. Adding a filter sport requires
  changing the backend enum and normalization, then regenerating the contract.
- Retain every incoming activity summary regardless of filter membership. Preserve
  unrecognized sport labels and the complete provider source; an unknown sport
  must never be guessed to be cycling or rejected by an enum deserializer.
- Only cycling contributes to cycling training, fitness, segments, and reports.
  Creating, updating, or deleting a non-cycling summary cannot invalidate cycling
  fitness. Changing a ride into a non-cycling summary removes its previous cycling
  contribution and clears old cycling analysis.

## Supported cycling

| Activity                            | Current normalized sport                                                                        | Source support and limitations                                                                                                                                                         |
| ----------------------------------- | ----------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Ride / generic cycling              | `road_ride`; legacy rows may contain `ride`                                                     | FIT/TCX/GPX and Strava `Ride`. Generic `ride` is included by the Road ride filter; it does not prove road terrain or outdoor recording.                                                |
| Road ride                           | `road_ride`                                                                                     | Shared ride imports, list/detail, and cycling workflows. No separate road-training product is implied.                                                                                 |
| Mountain bike                       | `mountain_bike`                                                                                 | Strava `MountainBikeRide`; normalized FIT cycling sub-sport tokens `mountain`, `downhill`, `ebikemountain`, and `ebikedownhill` map here. Recognized file labels also normalize here. |
| Indoor trainer / stationary cycling | `indoor_trainer_ride`                                                                           | Recognized indoor labels, FIT indoor cycling/spin, and Strava cycling with `trainer = true`. Cycling telemetry and training can be retained without GPS.                               |
| Virtual cycling, including Zwift    | `indoor_trainer_ride` when explicit virtual sport/normalization signals exist                   | Strava `VirtualRide`, FIT virtual cycling, and virtual recorder evidence. Recording context must still reject virtual geometry when the stored sport remains generic.                 |
| Gravel ride                         | `ride` for `GravelRide`                                                                         | Strava delivery and shared cycling workflows; no dedicated gravel filter or analytics. Preserve raw subtype metadata.                                                                 |
| E-bike ride                         | `ride` for `EBikeRide`                                                                          | Strava delivery and shared cycling workflows; assisted rides have no separate filter or comparison cohort.                                                                            |
| E-mountain-bike ride                | `ride` for Strava `EMountainBikeRide`; supported FIT mountain sub-sports map to `mountain_bike` | Source-dependent normalization; no separate assistance classification. Consistent e-bike subtype preservation is not implemented.                                                      |
| Handcycle                           | `ride` for recognized `handcycle`                                                               | Bike's cycling predicate and provider parser recognize it. Native Strava and compatible retained file inputs can use shared workflows, without dedicated metrics/filter.               |
| Velomobile                          | `ride` for recognized `velomobile`                                                              | Same native Strava recognition as handcycle; no dedicated metrics/filter.                                                                                                              |

The UI offers Mountain bike, Indoor trainer ride, and Road ride as distinct
cycling filters. Other recognized cycling forms group into generic `ride`,
which the Road ride filter includes. That compatibility grouping does not make
gravel, assisted cycling, handcycling, and velomobiles all road rides.

Native Rust Strava workflows accept cycling labels recognized by Bike, including
handcycle and velomobile. Non-cycling responses retain source metadata and skip
recording streams and cycling-derived work. See [Strava integration](strava-integration.md).

Zwift, Garmin Connect, TrainerRoad, and TrainerDay are sources or recording
platforms, not sport tiers. File/archive compatibility does not imply a
dedicated cloud connector for each. FIT sub-sport mapping does not establish
support for every session or vendor metric. DH analytics use explicitly marked
downhill segments; a mountain-bike label alone does not establish a DH session.

Shared cycling analytics currently admit these forms. Before claiming
subtype-specific training or fair comparisons, define rules for assisted
cycling, virtual distance/elevation, handcycling, and velomobiles in the owning
analytics specifications. Cycling-family membership alone does not validate
every XC metric for each subtype.

## Semi-supported and recognized non-cycling activities

| Activity                                    | Current behavior                                                                                                                                                                         | Missing for full support                                                                                                                                                            |
| ------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Hike | `hike`/`hiking` normalize to `hike`; Hike filter exists. Strava summaries appear in the feed; compatible files can retain GPS for generic detail display. | No hiking segments, training, reports, or documented live hiking import coverage. |
| Walk | `walk`/`walking` normalize to `walk`; Walk filter exists. Strava summaries appear in the feed with description and gear when supplied. | No walking-specific metrics or analytics; the 2026-10-14 summary change awaits rollout. |
| Run, including trail and virtual run labels | Labels normalize to `run`; Run filter exists. Strava summaries appear in the feed. Compatible files can expose generic telemetry, laps, and routes. | No running pace/cadence presentation, segments, training, or reports. Trail/virtual distinctions are not separate list sports. |
| Swim | `swim`/`swimming` normalize to `swim`; Swim filter exists. Strava summaries appear in the feed; generic FIT session/lap fields may survive parsing. | No dedicated pool lengths, stroke/rest model, swim pace, or analytics. |

Hike is semi-supported, alongside walk and run. None is supported in cycling
analytics. Non-cycling file imports currently still parse detail and execute
parts of the graph. Segment processing clears cycling efforts and the training
stage removes cycling analysis for non-rides; this is not the cheap deferred
path proposed under ACT05.

Current heatmap filters expose Run, Walk, and Hike, but policy v5 excludes
non-cycling activities at preparation and on every read surface. These filters
therefore produce no cycling heatmap contribution. Known virtual/indoor rides
and activities with unavailable authentic sources are also excluded. MAPS12
still proposes withholding unknown recording provenance; that stricter rule
is not implemented. A future map for another sport needs its own admission
rules.

Native Rust Strava promotes all summaries into the activity feed, including
walks, hikes, runs, swims, and other sports. Preserve distance, duration, elevation,
description, and gear ID/name when provided. Non-cycling and manual activities skip
provider streams and retain no invented GPS or sensors. Cycling calculations remain
limited to cycling sports. Archives/uploads remain the original-file path for
non-cycling recordings; sport-specific analytics are still outside this contract.

## Proposed expansion and implementation lift

These are relative scope assessments based on current code, not delivery
estimates. Existing ownership, artifacts, jobs, parsers, and detail components
can be reused. Sport-specific semantics and training are separate work;
changing a dropdown or allowlist is insufficient.

### Shared foundation: deferred retention

ACT05 retains originals and minimal owned summaries from mixed-sport imports,
records an explicit deferred outcome, and skips GPS arrays, generated exports,
segments, training, analytics, and heatmap jobs. An idempotent owned promotion
path parses retained originals when support is explicitly enabled. Archive
progress and traces distinguish deferred inputs from rejected/failed inputs.

This is **moderate ingestion/lifecycle work** shared by future sports: metadata
inspection before full detail decoding, lifecycle/schema changes, archive
counters and trace UI, replay rules, and mixed-sport tests. Measure retained
bytes, scan time, peak memory, full decodes, and downstream jobs. Raw storage
and metadata scanning still cost resources. Summary/provider-ID retention alone
cannot guarantee future GPS recovery. The ingestion spec owns the full contract.

### Hiking and walking

Reliable file-based support is **small to moderate work after deferred
retention**: declare summary/detail requirements; validate sport, GPS, and
elevation through promised formats; hide cycling actions/metrics; define pace
if wanted; cover missing GPS/heart rate, replay, ownership, and deletion.
Cycling analytics/maps remain isolated. Hiking/walking segments, training, or
cloud delivery are additional scope.

### Running

| Scope                                 | Relative lift                                  | Required work                                                                                                                                                                                                                                                                                                                                                   |
| ------------------------------------- | ---------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| File-based running history/detail     | Moderate after the shared foundation           | Promote retained runs; preserve road/trail/treadmill/virtual claims where available; add pace with proper units and zero/missing-value handling; clarify moving/elapsed time; define running cadence semantics instead of relabeling cycling RPM. Reuse maps, charts, and laps with appropriate labels/actions. Update API/UI contracts and synthetic coverage. |
| Running segments, trends, or training | Substantial separate feature                   | Define sport-owned segments, comparison cohorts, metrics, thresholds/zones, load interpretation, and reports/readiness. Do not broaden cycling predicates or mix runs into XC/DH reports and freshness without an explicit model decision.                                                                                                                      |
| Live Strava running sync              | Activity promotion in addition to Bike support | Requires provider fixtures, retained-summary promotion, quota/checkpoint handling and Bike boundary tests. Running recording streams remain outside the cycling workflow.                                                                                                                                                                                       |

Acceptance requires outdoor, trail, treadmill/GPS-free, and virtual run
fixtures; correct pace/laps, preserved subtype/source, idempotent promotion and
replay, and zero cycling segment, analytics, or heatmap contributions. Virtual
runs must not become physical geography in a future running map. Raw retention
alone remains a smaller feature than full running support.

### Swimming

| Scope                                    | Relative lift                                          | Required work                                                                                                                                                                                                                                                                                                        |
| ---------------------------------------- | ------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Swim summary/history from retained files | Moderate after the shared foundation                   | Swim filter/API vocabulary, summary requirements, distance/time units and swim pace, GPS-free detail states, and file fixtures. Reuse ownership, retention, promotion, and telemetry where semantics match.                                                                                                          |
| Full pool/open-water detail              | Substantial; more model/parser work than basic running | Pool/open-water subtypes; pool length/units, individual lengths, stroke/rest intervals, distance accounting, and lap/session semantics. Extend parser and versioned derived data. Open-water routes need appropriate gap/plausibility controls and GPS-free fallback. Pool swims must not produce geographic tracks. |
| Swim training, reports, or live sync     | Substantial additional scope                           | Sport-specific metrics/zones/load and comparisons; separately authorized Strava delivery work. Adding Swim to a dropdown does not enable these capabilities.                                                                                                                                                         |

The FIT adapter currently consumes record, lap, and session summaries and uses
the last session for a single activity draft. `ActivityDerivedData` exposes no
pool-length/stroke/rest model. Full swimming therefore requires parser and
normalized-model work. If multisport sessions are supported, specify activity
splitting or typed sessions rather than flattening to the final session's sport.

Full swimming acceptance requires GPS-free pool, pool distance/rest accounting,
open-water with GPS gaps, missing telemetry, retained originals, promotion/replay, and
zero cycling analytics/map contributions. Generic FIT decoding is insufficient
swimming coverage.

## Requirements when changing support

Document and test the exact sources/capabilities promised before promoting a
sport. Preserve originals and unknown metadata so retained inputs can be
replayed. Keep ownership, deduplication, promotion, retries, and deletion
consistent. Update Rust/OpenAPI and shared UI vocabulary together. Keep
SeaORM queries on owning models.

Activity deletion keeps persistence operations on the owning entities:

- Remove analytics and training analyses by activity ID, then delete the
  activity with both its ID and owner ID.
- Count remaining activities for the same import and owner. Retain shared
  imports while any activity still references them.
- Find and delete an unreferenced import with both its ID and owner ID.
  A missing import does not prevent activity deletion.
- Commit database changes before removing the returned source file. Preserve
  affected segment IDs for downstream derived-state rebuilds.

Verify these boundaries with synthetic fixtures and owning database tests:

- Supported real rides still import and contribute to eligible cycling maps.
- Indoor/virtual rides remain usable for supported cycling features while
  contributing zero heatmap geometry.
- Non-cycling and unknown inputs stay isolated from cycling processing/maps,
  including replay, generated artifacts, and duplicate/copy arrival.
- Deferred imports produce no full detail decode or ride jobs until promoted.
- Two users remain isolated across storage, jobs, queries, caches, and maps.

Future global maps also need participation and removal checks. This spec does
not promote run/swim to full support. The summary and vocabulary changes await
rollout. ACT05 and
MAPS12 own the pending deferred retention and stricter recording admission implementation.

## Code anchors

Rust paths are relative to `bike-rs`; UI paths are relative to the
repository root.

- Sport normalization, cycling predicate, and list aliases: `bike-rs/bike-core/src/activity_sport.rs`
- Shared UI choices: `bike-ui/lib/activitySports.ts`
- File summary parsing: `bike-rs/bike-core/src/activity_summary.rs`
- FIT session/sub-sport handling: `bike-rs/bike-core/src/fit_support.rs`
- Provider classification: `bike-rs/bike-core/src/activity_parser/strava.rs`
- Native provider admission: `bike-rs/bike-core/src/activity_parser/strava.rs`, `strava_activity_is_bike`
- Non-cycling graph guards: `bike-rs/bike-core/src/activity_import_pipeline.rs`
- Cycling query scope: `bike-rs/bike-core/src/entities/activities.rs`
- Derived detail model: `bike-rs/bike-core/src/activity_data.rs`
- Detail metrics: `bike-ui/components/activity-detail/ActivityMetricsSummary.tsx`
- Heatmap preparation: `bike-rs/bike-core/src/heatmaps/preparation.rs`
