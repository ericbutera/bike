# Data loading and performance status

This is the LOAD findings register for [PR 100](https://github.com/ericbutera/bike/pull/100).
The [DAG spec](specs/performance.md) owns behavior and saved outputs;
[implementation slices](TODO.md#performance) own assignments and dependencies.
Use the pre-PR-100 application as the baseline. Do not turn this register into
a separate audit, coordination system or prerequisite for unrelated work.

Reconciled against main `471485736` and the P6 #136, P2 #137 and P5
#138 review sequence on 2026-10-24. P2 #137 has merged into the P6 branch;
P6/P2 and P5 are not yet integrated into main.
These are source findings and published implementation statuses, not measured
production latency or proof of completed production backfills. LOAD06 is a
lock-order risk, not a reproduced production incident.

## Use this checklist during related work

A slice addresses the LOAD items assigned to its producer/read path and updates
their status with its implementing PR. Keep partial work partial: batching a
helper does not finish its singleton outer callers. Other findings remain in
their own slices. AGENTS.md owns the agent workflow.

## Completed

| Item                                    | Merged result                                                                                                                                                                                                              |
| --------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **LOAD01 Bulk effort-rank writes**      | [125](https://github.com/ericbutera/bike/pull/125): bounded 500-row writes preserve ranking/tie semantics and the transaction.                                                                                             |
| **LOAD02 Bulk segment/rider summaries** | [125](https://github.com/ericbutera/bike/pull/125): bounded inserts preserve atomic replacement and empty clearing.                                                                                                        |
| **LOAD03 Shared current-results input** | [129](https://github.com/ericbutera/bike/pull/129): one effort input produces analytics, personal and overall results published atomically. The changed-source retry concern below is a follow-up, not an unmerged LOAD03. |
| **LOAD09 Saved activity-detail ranks**  | [126](https://github.com/ericbutera/bike/pull/126): detail reads saved ranks and personal bests; missing/stale rows do not trigger history ranking or repair.                                                              |
| **LOAD14 Compact activity cards**       | [130](https://github.com/ericbutera/bike/pull/130): saved card/preview rows replace feed recording hydration and route-derived lookup. Map images remain LOAD15.                                                           |
| **LOAD22 Saved award-chip order**       | [123](https://github.com/ericbutera/bike/pull/123): typed immutable awards and explicit no-award output; saved display positions flow through API/UI. Conversion-only legacy storage remains C1.                           |

## Segment analytics and refresh scope

| Item/status        | Remaining change                                                                                                                                                                                                                                                                                                                       | Slice and source                                                                                                                         |
| ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| **LOAD04 Partial** | [136](https://github.com/ericbutera/bike/pull/136) batches the domain/job adapter, refreshes each affected activity summary once including removed matches, and removes duplicate regeneration refresh. P13 must pass batches from the outer upload workflow; P14 must use batches per historical page. Saved awards remain unchanged. | P6 implemented; P13/P14 caller work remains. [derived workflow](../bike-rs/bike-temporal/src/workflows/steps/derived.rs).                |
| **LOAD05 Partial** | [131](https://github.com/ericbutera/bike/pull/131) preserves unchanged activity effort IDs and avoids unchanged effort writes. Still narrow broad rider-summary invalidation, preserve interrupted-refresh recovery, and replace historical global replacement with pair reconciliation.                                               | P4; [regeneration](../bike-rs/bike-core/src/segment_regeneration.rs), [effort persistence](../bike-rs/bike-core/src/segment_support.rs). |
| **LOAD06 Open**    | Align matching/results/deletion lock order. Matching can write efforts before segment state while analytics locks segment before efforts. Pair writes must reject deleted/changed inputs and preserve concurrent new imports.                                                                                                          | P4; same persistence paths plus analytics. Verify with focused concurrency coverage; do not weaken locking.                              |

[136](https://github.com/ericbutera/bike/pull/136) makes the #129 changed-input
guard fail for retry before downstream award recording. Deletion remains a
separate successful empty outcome. This correction is implemented in P6 and
awaits merge; the outer upload batching remains P13.

[127](https://github.com/ericbutera/bike/pull/127) completed fixed geometry and
creator-only rename without matching. It does not finish LOAD05/06/19.
[124](https://github.com/ericbutera/bike/pull/124) was closed unmerged; a full-history
recording scan and segment-wide delete/reinsert are not an implementation target.

## Saved reads and screens

| Item/status     | Remaining change                                                                                                                                                                                                                                                                                    | Producer → read/UI slices                                                                                                                                                         |
| --------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **LOAD07 Open** | Fitness GET rebuilds on cache miss and calculates an unsaved decay tail. Save required daily values upstream, then serve them without repair, writes or replacement calculation.                                                                                                                    | P11a → B3 → U3. [fitness controller](../bike-rs/api/src/controllers/fitness.rs).                                                                                                  |
| **LOAD08 Open** | Remove segment-list stale-summary enqueueing. Mutation/background operations own refresh; browsing selects saved data.                                                                                                                                                                              | P6/P14 → B1. [segment controller](../bike-rs/api/src/controllers/segments.rs).                                                                                                    |
| **LOAD10 Open** | P9 #133 now saves climbing-report facts and typed references; the report read cutover remains. Reports still redetect climbs, expand recordings and recalculate training facts; report GET also writes a cooldown. Produce missing facts and apply owner/date filters before loading saved reports. | P9/P10 → B3 → U3. [reports](../bike-rs/bike-core/src/services/reports.rs), [report controller](../bike-rs/api/src/controllers/reports.rs).                                        |
| **LOAD11 Open** | Segment UI reconstructs PR/rank/attempt results; matched-activity panels slice telemetry for metrics. Persist those facts and remove browser reconstruction while preserving saved order.                                                                                                           | P7/P8 → B1/B2 → U1/U2. [segment state](../bike-ui/components/segment-detail/useSegmentDetailState.ts), [matched facts](../bike-ui/components/activity-detail/matchedSegments.ts). |
| **LOAD12 Open** | Comparison/analysis expand whole recordings per effort and apply some limits after calculations. Query authorized selected saved samples and page effort facts before loading.                                                                                                                      | P8 → B1 → U1. [segment controller](../bike-rs/api/src/controllers/segments.rs), [effort table](../bike-ui/components/segment-detail/SegmentDetailEffortsSection.tsx).             |
| **LOAD13 Open** | Progress chooses yearly bests on GET; training goals rebuild sessions/progress/recommendations from history. Save period results and compact goal facts.                                                                                                                                            | P7/P11b → B1/B3 → U1/U3. [segment controller](../bike-rs/api/src/controllers/segments.rs), [training goals](../bike-rs/bike-core/src/services/training_goals.rs).                 |
| **LOAD15 Open** | Warm map-image requests still hydrate/hash recordings, and legacy previews load full details. Save geometry/image identity and use compact ownership reads.                                                                                                                                         | B4 → U3. [map service](../bike-rs/bike-core/src/activity_maps/service.rs), [legacy preview](../bike-ui/app/activity-previews/route.ts).                                           |

LOAD09, LOAD14 and LOAD22 are complete; do not reopen their implemented reads
as new groundwork. Preserve their missing-value, ownership and saved-order behavior.

## Imports and backfills

| Item/status                   | Remaining change                                                                                                                                                                                                                                                                                                                                                                 | Slice and source                                                                                                                                                                                 |
| ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **LOAD16 Open**               | Unchanged Strava input still runs derived operations. Skip already-completed unnecessary effects while recovering interrupted publication; unchanged source alone does not establish saved output.                                                                                                                                                                               | P12. [upsert workflow](../bike-rs/strava-temporal/src/workflows/activity_upsert.rs), [persistence](../bike-rs/integrations/src/strava/native/persistence.rs).                                    |
| **LOAD17 Open**               | Refresh affected rider/date fitness scope once per bounded import/reprocess operation instead of rebuilding overlapping history for each activity.                                                                                                                                                                                                                               | P11a producer, P13 caller. [activity pages](../bike-rs/bike-temporal/src/workflows/activity_pages.rs).                                                                                           |
| **LOAD18 Open**               | Training-analysis rebuilding still selects/writes per activity after preloading. Load the requested set and write bounded batches through its model.                                                                                                                                                                                                                             | P10. [training analysis](../bike-rs/bike-core/src/activity_training_analysis.rs).                                                                                                                |
| **LOAD19 Partial**            | [138](https://github.com/ericbutera/bike/pull/138) saves indexed activity/segment route bounds, includes missing bounds, preserves supported drift/wrapping, and provides bounded candidate-ID queries before geometry/recording hydration. New-activity matching uses the filter. The old historical scan/global replacement still needs P4 pair writes and P14 Temporal pages. | P5 implemented; P4/P14 remain. [bounds implementation](https://github.com/ericbutera/bike/pull/138/files), [segment candidates](../bike-rs/bike-core/src/entities/segments.rs).                  |
| **LOAD20 Separate follow-up** | Archive pages repeatedly materialize the complete ZIP and scan its directory. Reuse retained index/materialization with bounded temporary storage and retry recovery. This does not block the ordinary activity DAG cutover.                                                                                                                                                     | [archive import](../bike-rs/bike-core/src/archive_import/native.rs), [object files](../bike-rs/bike-core/src/object_store/files.rs).                                                             |
| **LOAD21 Separate follow-up** | Batch per-connection user lookups and admin backfill change marks, preserving ownership and dirty-state merge semantics. Keep connection scanning an explicit operation.                                                                                                                                                                                                         | [Strava scan](../bike-rs/integrations/src/strava/native/scan.rs), [analytics](../bike-rs/bike-core/src/analytics.rs), [user states](../bike-rs/bike-core/src/entities/analytics_user_states.rs). |

## Evidence for completion

Use existing unit fixtures for the changed behavior and relevant local checks.
A write slice covers unchanged retry, changed/empty output and affected ownership;
a batching slice also checks multiple overlapping inputs at the outer caller.
A read slice proves saved data is enough and missing data causes no writes,
jobs, recording reconstruction or history ranking.

P4 and P8 also require the [activity 1671 comparison](specs/performance.md#activity-1671-regression-acceptance)
against the frozen, read-only reference. Capture is complete and reference
checksums were reverified on 2026-10-24; saved-value and wired replay comparisons
remain pending. The [checkpoint after PRs 138 and 140](specs/performance.md#checkpoint-after-prs-138-and-140)
extends exact-activity validation to every applicable DAG stage and its consumers.
Direct-helper parity alone does not establish full ingestion-DAG equivalence.

Keep actual timing/SQL evidence when claiming runtime latency improvements.
Fewer source-level queries and merged code alone do not establish production
speed or completed backfills.
