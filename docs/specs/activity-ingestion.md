# Activity ingestion

Bike retains activity inputs, normalizes supported activities, and rebuilds
derived results through one processing graph. Riders can follow an import,
inspect a summary of each stage, and replay from a selected stage without
uploading again. This document defines the contract; the
[recovery runbook](../runbooks/activity-source-recovery.md) contains operator
commands. [Supported activities](supported-activities.md) owns sport support.

## Rider workflow

- Open Imports from the activity upload page. History includes manual files,
  archive entries, and Strava deliveries, including inputs with no activity.
- History is owner-scoped, paginated, and filterable by source and outcome.
  Each row links directly to its import and to its activity when one exists.
- An import shows its selected retained source, graph, recent attempts,
  stage summaries, current work, error, and available recovery action.
- Progress refreshes while work is queued or running. Loading, empty, denied,
  unavailable-source, and request-failure states are explicit.
- Stage details show compact facts and a short error, not a full log viewer.
  Selecting a node shows its summary and the Replay from here action.
- Owners can inspect and replay their imports. Administrators can inspect
  other owners' imports through existing admin controls. Replay never changes
  ownership or grants access to another owner's input.

## Supported Sources

- Manual upload accepts nonempty FIT, TCX, and GPX within the configured size
  limit. Prefer FIT in guidance. Retain exact bytes and return an accepted
  import after durable worker queueing. Invalid multipart, extension, or size
  receives a field error. Repeated identical bytes return the owned import;
  a failed import still offers replay.
- Archive import accepts a shareable HTTPS ZIP URL. Extract FIT/TCX/GPX,
  gzip-wrapped entries, and nested Garmin ZIPs within configured expansion
  limits. Try FIT before TCX before GPX. Link entries to the parent archive
  job. Persist counters during processing and preserve partial progress.
- Strava credentials, discovery, quota, fetching, and delivery belong to the
  gateway. Bike retains versioned summary/streams JSON and parses it directly.
  Initial and incremental connection sync cover the last 30 days; older
  history requires an archive. Replay makes no provider requests.

## Normalization

Every full activity-processing path uses the same graph and stage behavior:

| Stage                      | Result                                        | Summary facts                                         |
| -------------------------- | --------------------------------------------- | ----------------------------------------------------- |
| `raw_stored`               | Verify retained input                         | Filename, format, byte size, checksum, source quality |
| `activity_parsed`          | Normalize source data and recording evidence  | Route records, chart samples, laps, sport             |
| `activity_saved`           | Save summary/detail while preserving identity | Saved activity ID                                     |
| `segments_built`           | Replace segment efforts                       | Affected segments                                     |
| `segment_analytics_built`  | Rebuild affected segment analytics            | Segments rebuilt                                      |
| `activity_analytics_built` | Rebuild activity analytics                    | Activities rebuilt                                    |
| `training_analysis_built`  | Rebuild applicable training analysis          | Analyses rebuilt or explicit skip reason              |

The graph endpoint and executor derive from the same definition. Dependencies
determine order; stage status is recorded execution evidence, never inferred
from a node's position. [Segment processing](segment-processing.md) owns
matching and effort rules.

### Attempts and stage summaries

- Every execution has a stable attempt ID, requested/actual start stage,
  selected source identity/checksum, timestamps, and terminal outcome.
- Stage states are `pending`, `running`, `completed`, `failed`, `skipped`, or
  `reused`. Record start/end times, compact facts, and an error or skip
  reason. Missing historical evidence remains unknown; it is not success.
- Summaries count actual outputs. Zero is a measured result; missing is
  unknown. Parsing records means normalized route records, not every FIT
  message. Do not expose source bytes, credentials, or large GPS arrays.
- Duplicate detection links the existing activity. Stages not executed are
  skipped with a reason; a duplicate outcome does not complete every node.
- Persist failed stage and error before finishing a failed execution. Keep
  earlier attempts and their summaries; never mix timestamps across attempts.
- Persisted execution state remains authoritative if integration event
  delivery is unavailable. A missing trace must be reported explicitly.

### Replay

- Replay accepts any graph stage and reruns that stage and all descendants
  to a terminal outcome. Retry defaults to the failed stage, or `raw_stored`
  when the failed stage is unknown.
- Preserve earlier successful checkpoints as `reused`, with their originating
  attempt. Persist the parsed checkpoint needed to start at `activity_saved`;
  later stages use the linked normalized activity and affected segment set.
- Reuse requires the same retained source checksum and valid prerequisite
  results. Missing/stale prerequisites move the actual start to the earliest
  required stage. Show the requested/actual start and reason before queueing.
- A missing or checksum-invalid source blocks replay with a recovery reason;
  generated exports cannot become replay inputs. `raw_stored` replay verifies
  retained bytes rather than fetching or fabricating replacements.
- One active execution may mutate an import/activity at a time. Reject
  conflicting replay with a conflict response and link to the active work.
  Queueing and import state must agree; a queue failure is not accepted work.
- Replay replaces selected downstream outputs without duplicating activities
  or efforts. Preserve provider/Bike IDs, rider classifications, ownership,
  original bytes, and rejecting recording evidence. Source changes require
  parsing again. Repeated replay remains idempotent.
- Success means the selected graph stages finished and required follow-up
  work was durably queued. Fitness rebuilding and heatmap publication have
  separate readiness/outcome; graph success does not claim a visible map.

## Processing State

Imports expose queued/running and terminal processed, duplicate, failed, or
canceled outcomes. A failure before activity creation remains replayable by
import ID. Manual tasks and admin reruns cannot report success by skipping a
failed import. Bulk reprocessing persists each import's result.

Worker startup and owner inspection recover executions after five minutes
without progress. A live task or parent archive/bulk task heartbeat prevents
recovery from stealing active work. Preserve interrupted attempts and queue a
new execution; queued replay keeps its selected start and attempt ID.
Background task attempt numbers remain monotonic through recovery. Recovery
compares the task's status, attempt, and heartbeat timestamp before requeueing;
an intervening heartbeat prevents that stale snapshot from reclaiming the task.
An exhausted task receives at least one eligible recovery attempt without
erasing prior execution history. The separate import execution IDs and replay
stage contracts remain unchanged. See
[task processing history](admin-operations.md#task-processing-history).

Archive jobs expose queued/running, succeeded, partial, or failed. Succeeded
means every supported entry was imported or duplicate; partial means both
successful and failed entries; failed means no supported entry succeeded or
the job could not finish. Unsupported entries have their own count. Counters
and bounded error samples survive failure; retry preserves completed work.

## Deduplication

Deduplicate within an owner using provider IDs, exact file checksums, then
activity fingerprints and route agreement. Missing/ambiguous evidence must
not merge unrelated activities. Cross-source representations of the same
activity retain their source/provenance and link to one visible activity.

Choose the richest authentic owned source: FIT, TCX, native Strava archive
JSON, GPX, then provider JSON. Selection currently covers artifacts on one
import. DATA20 tracks the remaining implementation gap: a richer source
arriving as a separate duplicate import must promote the replay source and
rebuild affected outputs while preserving identity. Rejecting recording
evidence must win regardless of arrival order.

## Current FIT-First Implementation

### Generated Strava TCX retirement and source backfill

Bike generates no replacement TCX/GPX. Genuine rider TCX remains supported.
Reject generated artifact labels and the retired Bike export header, including
mislabeled copies. Recover authentic originals before retained provider JSON;
recent gaps may use an explicitly requested incremental gateway refresh.
Preserve summaries/GPS and sole retained bytes for unresolved gaps. Withhold
unavailable sources from heatmaps and invalidate existing contributions.
Promotion is not completed recovery until replay succeeds. Cleanup removes
obsolete generated bytes only after a verified replacement and successful
replay. See the [operator runbook](../runbooks/activity-source-recovery.md).

## Native Strava Provider Parsing

Retain delivered typed and unknown summary fields plus supported streams and
modeled stream metadata. Stream JSON is provider-derived data, not an original
FIT file. Authentic Strava archive JSON is retained unchanged as an original.
File downloads return authentic originals; provider payloads stay distinct.

## Activity and GPS storage

Owned imports/artifacts record kind, format, source quality, filename, size,
checksum, and storage path. Production uploads storage is persistent. Archive
working copies are temporary; an archive URL is not retained source data.
Import versions describe replay compatibility: historical version 1 remains
readable; artifact-aware version 2 supports multiple sources/native JSON.

Activities currently store scalar summaries and normalized GPS/chart/lap and
recording context in `derived_data_json`. Version 2 objects and historical
version 1 arrays remain readable. Coordinates are degrees and point times
are elapsed seconds from activity start. Missing GPS stays missing.

Recording evidence is distinct from transport and sport. The generated scalar
`recording_environment` reflects retained context atomically. Current heatmap
policy v5 rejects non-cycling, unavailable-source, and known indoor/virtual
recordings; unknown provenance is still admitted. Projections/chunks are
rebuildable publication data, with current policy/generation guards and owner
revisions. Preserve the exact 120-second gap and valid continuous-road controls.

### Proposed separation of activity detail from summaries

DATA19 remains a proposal: owned one-to-one normalized details, parser/schema
version, artifact/checksum and activity generation; lightweight admission
fields remain with summaries. Backfill in bounded batches, compare values,
verify transactional replay/deletion/isolation, and retain rollback until
production verification. Remove the old large payload only in a later
migration. Measure storage, transfer, latency, and memory before chunking.

## Non-cycling retention proposal

ACT05 remains unimplemented: retain exact archive/upload originals and minimal
metadata as `deferred` imports without placeholder rides, GPS/detail decoding,
ride jobs, analytics, or projections. Unknown sport remains deferred. Count
deferral separately from failure, duplicate, and unsupported. Ordinary replay
preserves deferral; explicit owned promotion or a supported-sport policy change
can run the graph idempotently. Sport support does not authorize heatmaps.
Metadata includes available provider identity, sport, time/duration/distance,
recording claims, checksum, retention reason, and metadata-parser version.
Record raw-present, summary-only, or unavailable recoverability. Retain one blob
per identical owned checksum; measure bytes, memory, metadata time, and decodes.

### Unchanged gateway constraint

Non-cycling gateway events currently deliver deletion without summary or GPS.
Bike honors removal and keeps receipts without inventing an activity catalog.
ACT05 does not change the gateway or authorize Bike-owned provider fetching.

## Heatmap admission proposal

MAPS12 remains unimplemented. Proposed admission fails closed for unsupported,
indoor/virtual, unavailable, conflicting, and unknown outdoor provenance.
Document accepted evidence rules and trust limits before implementation.
Persist a versioned decision/reason/evidence separately from claimed recording
environment. Rejecting evidence survives duplicates, replay, and generic
exports. Personal assertions do not independently qualify global contribution.
Use one admission policy across ingestion, recovery, publication, and every
read; changes invalidate generations/contributions. Preserve owner isolation
and explicit global participation/removal. See [heatmaps](heatmaps.md).

### Required verification

Use synthetic fixtures for ingestion, failure/retry before activity creation,
stage summaries, every replay start, duplicate/source order, missing/stale
checkpoints, queue failure, persisted bulk results, partial archives, and two
owners. Verify both skipped and positive outputs, not just graph colors.
Admission/retention proposals require their positive/exclusion/promotion
regressions before release. Server-specific migration/locking checks are
separate from unit tests. Production claims require deployed image/policy,
migrations/backfill, terminal jobs, readiness, and user-visible results.

## Follow-up tracking

Admission regressions use synthetic data at retention, classification, every
graph stage, replay, duplicate merging, preparation, publication, and reads.
Known virtual rides, stripped metadata, and unknown recordings must not gain
contributions when the proposed stricter admission policy is implemented.
Eligible outdoor controls must produce chunks, visible pixels, bounds, zones,
and mapped counts. Preserve the exact 120-second GPS-gap regression and valid
continuous-road controls. Rejecting evidence wins regardless of arrival order,
provider correlation, title edits, or replay; verify stale workers, policy
changes, and removal after reclassification. Deferred non-cycling inputs retain
originals and summaries without full GPS decoding or downstream ride jobs.
Verify two-owner isolation and explicit global participation and removal.

The default [TEST11 browser gate](../E2E-TODO.md) owns a disposable PostgreSQL
environment and excludes worker execution. Unit checks use mocked boundaries
or in-memory databases; server-specific checks remain opt-in. A successful
local check does not establish production recovery or heatmap publication.

Status lives in [Bike TODO](../TODO.md). This contract owns DATA04, DATA12–14,
DATA17–20, ACT04–05, and ingestion trace/replay work. ACT05, DATA19, MAPS12, and
global maps remain proposals; this change does not implement them.
