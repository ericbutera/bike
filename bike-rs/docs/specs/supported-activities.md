# Supported Activities Specification

Bike is a cycling platform. Recognizing a sport label, retaining an original,
processing a ride, and admitting its route to a heatmap are separate decisions.

This specification owns support tiers and the activity inventory. The
[ingestion specification](activity-ingestion.md) owns retention and processing;
the [heatmap specification](../../../docs/specs/heatmaps.md) owns geographic
contribution. Task status stays in [the Bike backlog](../../../docs/TODO.md).
The inventory reflects inspected code on 2026-10-06, not a new production
verification. Expansion work below is proposed, not implemented behavior.

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
- **Recognized only:** a label survives normalization, but dedicated UI, source
  delivery, metrics, and coverage are incomplete. Preserve available originals
  under the deferred-retention proposal rather than interpreting them as rides.
- **Unsupported/unknown:** no declared product support. Retain available inputs
  and metadata under the deferred-retention proposal; do not guess cycling.

`sport` identifies the discipline. `activity_type` remains the separate
Training/Race classification. Recording environment and recorder evidence are
independent of both. Indoor cycling is supported but contributes no real-world
heatmap geometry.

## Supported cycling

| Activity                            | Current normalized sport                                                                         | Source support and limitations                                                                                                                                                          |
| ----------------------------------- | ------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Ride / generic cycling              | `road_ride`; legacy rows may contain `ride`                                                      | FIT/TCX/GPX and gateway `Ride`. Generic `ride` is included by the Road ride filter; it does not prove road terrain or outdoor recording.                                                |
| Road ride                           | `road_ride`                                                                                      | Shared ride imports, list/detail, and cycling workflows. No separate road-training product is implied.                                                                                  |
| Mountain bike                       | `mountain_bike`                                                                                  | Gateway `MountainBikeRide`; normalized FIT cycling sub-sport tokens `mountain`, `downhill`, `ebikemountain`, and `ebikedownhill` map here. Recognized file labels also normalize here.  |
| Indoor trainer / stationary cycling | `indoor_trainer_ride`                                                                            | Recognized indoor labels, FIT indoor cycling/spin, and Strava cycling with `trainer = true`. Cycling telemetry and training can be retained without GPS.                                |
| Virtual cycling, including Zwift    | `indoor_trainer_ride` when explicit virtual sport/normalization signals exist                    | Gateway `VirtualRide`, FIT virtual cycling, and virtual recorder evidence. Recording context must still reject virtual geometry when the stored sport remains generic.                  |
| Gravel ride                         | `ride` for `GravelRide`                                                                          | Gateway delivery and shared cycling workflows; no dedicated gravel filter or analytics. Preserve raw subtype metadata.                                                                  |
| E-bike ride                         | `ride` for `EBikeRide`                                                                           | Gateway delivery and shared cycling workflows; assisted rides have no separate filter or comparison cohort.                                                                             |
| E-mountain-bike ride                | `ride` for gateway `EMountainBikeRide`; supported FIT mountain sub-sports map to `mountain_bike` | Source-dependent normalization; no separate assistance classification. Consistent e-bike subtype preservation is not implemented.                                                       |
| Handcycle                           | `ride` for recognized `handcycle`                                                                | Bike's cycling predicate and provider parser recognize it. The gateway does not deliver it; compatible retained file inputs can use shared workflows, without dedicated metrics/filter. |
| Velomobile                          | `ride` for recognized `velomobile`                                                               | Same Bike-only recognition and gateway limitation as handcycle; no dedicated metrics/filter.                                                                                            |

The UI offers Mountain bike, Indoor trainer ride, and Road ride as distinct
cycling filters. Other recognized cycling forms group into generic `ride`,
which the Road ride filter includes. That compatibility grouping does not make
gravel, assisted cycling, handcycling, and velomobiles all road rides.

The gateway delivers exactly `Ride`, `VirtualRide`, `MountainBikeRide`,
`GravelRide`, `EBikeRide`, and `EMountainBikeRide`. Bike recognizes more labels
than this contract delivers. Parser acceptance does not establish live sync.
No gateway change is authorized by this specification.

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

| Activity                                    | Current behavior                                                                                                                                                                         | Missing for full support                                                                                                                                                   |
| ------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Hike                                        | `hike`/`hiking` normalize to `hike`; Hike filter exists. Compatible file/archive inputs can retain summaries and GPS for generic detail display.                                         | No live gateway delivery, hiking segments/training/reports, or documented end-to-end hiking coverage. Deferred retention is proposed, not implemented.                     |
| Walk                                        | `walk`/`walking` normalize to `walk`; Walk filter exists. Same generic file/detail compatibility as hiking.                                                                              | No live gateway delivery or walking-specific metrics/analytics.                                                                                                            |
| Run, including trail and virtual run labels | Recognized labels normalize to `run`; Run filter exists. A retained running FIT fixture verifies basic summary parsing. Compatible files can expose generic telemetry, laps, and routes. | No live gateway delivery, running pace/cadence presentation, running segments, training, or reports. Trail/virtual distinctions are not separate list sports.              |
| Swim                                        | `swim`/`swimming` normalize to `swim`; generic FIT session/lap fields may survive parsing. **Recognized only**, not a complete swim experience.                                          | No Swim filter or gateway delivery; no dedicated pool lengths, stroke/rest model, swim pace, or analytics. Generic parsing does not prove correct pool/open-water support. |

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

The unchanged gateway skips non-cycling streams and delivers removal without
an activity payload. Bike cannot retain Strava hike/run/swim summaries or GPS
that it never receives. Archives/uploads provide the available original-file
path while that contract remains unchanged.

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

| Scope                                 | Relative lift                                      | Required work                                                                                                                                                                                                                                                                                                                                                   |
| ------------------------------------- | -------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| File-based running history/detail     | Moderate after the shared foundation               | Promote retained runs; preserve road/trail/treadmill/virtual claims where available; add pace with proper units and zero/missing-value handling; clarify moving/elapsed time; define running cadence semantics instead of relabeling cycling RPM. Reuse maps, charts, and laps with appropriate labels/actions. Update API/UI contracts and synthetic coverage. |
| Running segments, trends, or training | Substantial separate feature                       | Define sport-owned segments, comparison cohorts, metrics, thresholds/zones, load interpretation, and reports/readiness. Do not broaden cycling predicates or mix runs into XC/DH reports and freshness without an explicit model decision.                                                                                                                      |
| Live Strava running sync              | Delivery-contract work in addition to Bike support | Requires separately authorized gateway delivery changes, provider fixtures, quota/checkpoint handling, and Bike boundary tests. Bike-only changes cannot recover data missing from current deliveries.                                                                                                                                                          |

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

Verify these boundaries with synthetic fixtures and owning database tests:

- Supported real rides still import and contribute to eligible cycling maps.
- Indoor/virtual rides remain usable for supported cycling features while
  contributing zero heatmap geometry.
- Non-cycling and unknown inputs stay isolated from cycling processing/maps,
  including replay, generated artifacts, and duplicate/copy arrival.
- Deferred imports produce no full detail decode or ride jobs until promoted.
- Two users remain isolated across storage, jobs, queries, caches, and maps.

Future global maps also need participation and removal checks. This spec does
not promote run/swim to full support or change production behavior. ACT05 and
MAPS12 own the pending deferred retention and stricter recording admission implementation.

## Code anchors

Rust paths are relative to `bike-rs`; UI and gateway paths are relative to the
repository root.

- Sport normalization, cycling predicate, and list aliases: `bike-core/src/activity_sport.rs`
- Shared UI choices: `bike-ui/lib/activitySports.ts`
- File summary parsing: `bike-core/src/activity_summary.rs`
- FIT session/sub-sport handling: `bike-core/src/fit_support.rs`
- Provider classification: `bike-core/src/strava_provider_payload.rs`
- Gateway allowlist: `strava-gateway/internal/provider/client.go`, `Activity.Cycling`
- Non-cycling graph guards: `bike-core/src/activity_import_pipeline.rs`
- Cycling query scope: `bike-core/src/entities/activities.rs`
- Derived detail model: `bike-core/src/activity_data.rs`
- Detail metrics: `bike-ui/components/activity-detail/ActivityMetricsSummary.tsx`
- Heatmap preparation: `bike-core/src/heatmaps/preparation.rs`
