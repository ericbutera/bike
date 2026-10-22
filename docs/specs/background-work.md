# Durable worker and Strava integration

- This is the single worker specification. It owns client submission, background
  processing, Strava integration, and retained activity sources.
  [The TODO](../TODO.md#worker) owns outstanding work;
  [cutover evidence](#cutover-evidence) records completed verification.
- **Design decision, 2026-10-17:** treat ingestion as repeatable, low-consequence
  work. Read the activity file, process it, and call the downstream tasks.
  A developer should be able to follow that sequence in one workflow.
- This replaces the earlier execution-accounting, shared-output coordination,
  cancellation-handshake, and exact batch-result requirements. The Bike task
  code implements this sequence. The [2026-10-17 cutover](#cutover-evidence)
  records production verification.

## Ownership and runtime

| Boundary                       | Strava            | Bike tasks                                   |
| ------------------------------ | ----------------- | -------------------------------------------- |
| Workflow/Activity registration | `strava-temporal` | `bike-temporal`                              |
| Task queue                     | `bike-strava`     | `bike-tasks`                                 |
| Workflow ID prefix             | `strava-`         | `bike-`                                      |
| Object keys                    | `strava/`         | `activity-imports/` and retained `archives/` |

- Keep both SDK task-queue workers in the existing `temporal-worker`, sharing the SDK connection,
  database pool, object store, and metrics. Preserve coordinated process shutdown.
- Run domain Activities on Tokio's blocking pool so parsing and segment matching
  cannot starve Temporal polling, timers, or health requests. The Bike queue uses
  two native Activity slots within the existing worker resource budget.
- Give Bike Activities a 30-minute Start-To-Close timeout for whole-archive and
  segment rematching work. Keep the provider RPC timeout independent.
- Temporal owns running work, retries, child execution, and status. PostgreSQL
  stores activities and other business data. Do not build a second execution
  system in database rows, audit JSON, or storage descriptors.
- Controllers accept requests and return a job reference. Workflow methods show
  the processing steps. Activities call existing services/models; models own
  queries, transactions, uniqueness, and replacement writes.
- Strava owns provider intake, OAuth, fetching, normalization, and connection
  checks. Its workflows call the same neutral domain computations as file imports.
  Share domain operations and a small typed SDK helper without a generic DAG engine.
- Temporal UI shows running/failed work and child steps. Bike retains source
  metadata, downloads, and useful activity-change history. Link recorded namespace
  and workflow IDs through `TEMPORAL_UI_URL`; never invent a job ID from an
  activity ID. Without a recorded job, link the configured namespace list.
- All Bike producers submit to Temporal. The SQL worker, queue, batch tracking,
  stage executor, recovery loops, and task administration API are retired.

## Task inventory and behavior parity

- The baseline is the 12 default processors registered in
  [`tasks/mod.rs` before SQL retirement](https://github.com/ericbutera/bike/blob/e401699a/bike-rs/worker/src/tasks/mod.rs).
  All 12 have registered workflows on `bike-tasks` in
  [the current worker](../../bike-rs/bike-temporal/src/runtime.rs).
  The separately registered `email_notification` is intentionally retired.
- Preserve each task's inputs, owned activity selection, parsing, calculations,
  and downstream effects. Reuse the existing domain operations. Temporal replaces
  execution machinery; reproducing SQL locks, audit receipts, queue rows, and
  exact batch accounting is not behavior parity.

| Former task / current workflow name      | Required domain behavior                                                                                                                                                 | Implementation                                                                                                                                                                                                                                  |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `process_activity_import`                | Read the retained source and supplemental recording, parse/enrich, deduplicate/save, then await segments, segment results, achievements, heatmap, training, and fitness. | [Import](../../bike-rs/bike-temporal/src/workflows/process_activity_import.rs), [shared source operation](../../bike-rs/bike-core/src/activity_reprocessing.rs), [derived sequence](../../bike-rs/bike-temporal/src/workflows/steps/derived.rs) |
| `reprocess_activity_import`              | Run the same sequence against the owned import/activity; preserve identity and user-edited title/Training/Race selection.                                                | [Reprocess](../../bike-rs/bike-temporal/src/workflows/reprocess_activity_import.rs)                                                                                                                                                             |
| `rebuild_fitness_freshness`              | Rebuild the user's stored fitness/fatigue/form from current activities using `rebuild_fitness_freshness_cache`.                                                          | [Fitness](../../bike-rs/bike-temporal/src/workflows/rebuild_fitness_freshness.rs)                                                                                                                                                               |
| `rebuild_segment_analytics`              | Rebuild current segment and user summaries/achievements using `rebuild_segment_analytics_cache`; client lists become one workflow per distinct valid segment.            | [Segment analytics](../../bike-rs/bike-temporal/src/workflows/rebuild_segment_analytics.rs)                                                                                                                                                     |
| `prepare_heatmap`                        | Load the owned activity's current pending projection and prepare it through the existing privacy/source rules.                                                           | [Heatmap](../../bike-rs/bike-temporal/src/workflows/prepare_heatmap.rs)                                                                                                                                                                         |
| `regenerate_activity_segments`           | Replace matches for one owned activity, refresh affected old/new segment results, then activity achievements. A broader rebuild on retry is acceptable.                  | [Activity segments](../../bike-rs/bike-temporal/src/workflows/regenerate_activity_segments.rs)                                                                                                                                                  |
| `regenerate_segment_efforts`             | Run the existing segment-wide matching operation, including segment analytics and affected activity achievements.                                                        | [Segment efforts](../../bike-rs/bike-temporal/src/workflows/regenerate_segment_efforts.rs), [domain operation](../../bike-rs/bike-core/src/segment_regeneration.rs)                                                                             |
| `backfill_user_xc_training`              | Page the user's cycling activities, rebuild each training analysis, then mark the existing preference complete.                                                          | [User batches](../../bike-rs/bike-temporal/src/workflows/activity_pages.rs)                                                                                                                                                                     |
| `reprocess_user_activity_imports`        | Page owned activities linked to retained imports, exclude native Strava sources, and await reprocessing for every item.                                                  | [User batches](../../bike-rs/bike-temporal/src/workflows/activity_pages.rs)                                                                                                                                                                     |
| `reprocess_archive_fit_activity_imports` | Use the same reprocessing loop restricted to `archive_url_import` imports with format `fit`.                                                                             | [User batches](../../bike-rs/bike-temporal/src/workflows/activity_pages.rs)                                                                                                                                                                     |
| `regenerate_user_segments`               | Page the user's cycling activities and await activity segment regeneration, including its downstream results.                                                            | [User batches](../../bike-rs/bike-temporal/src/workflows/activity_pages.rs)                                                                                                                                                                     |
| `activity_archive_import`                | Retain ZIP bytes, find supported entries, retain/deduplicate their files, await ordinary import workflows, and finish after the last page.                               | [Archive](../../bike-rs/bike-temporal/src/workflows/activity_archive_import.rs), [extraction](../../bike-rs/bike-core/src/archive_import/native.rs)                                                                                             |

- [Domain producers](../../bike-rs/bike-core/src/jobs/adapter.rs) and the
  [import submission service](../../bike-rs/bike-temporal/src/submission.rs) start
  Temporal directly. Uploads, admin/bulk requests, archives, segment changes,
  training settings, and activity edits/deletes use these clients. Per-activity
  segment regeneration also runs as an awaited child of user regeneration.
- Startup recovery, expired-job scans, SQL retry timers, and batch barriers are
  replaced by native retries, replay, and awaited children. Heatmap preparation
  runs from activity changes; automatic heatmap/Strava scans are retired.
  `reconcile_heatmaps` and Strava's scan workflow remain explicit operator tools.
- SQL task/admin progress and exact archive outcome counts are intentionally
  retired. Temporal is authoritative for running, failed, and canceled execution.
  Existing archive/training domain completion fields remain summaries, not a
  second execution state machine. A stopped workflow can leave those summaries
  pending; inspect or rerun the native job.
- Aggregate work may repeat after each item instead of once at a SQL batch
  barrier. Successful effects survive later failures. These are explicit
  orchestration changes; parsing and domain calculations remain shared.
- The existing [workflow tests](../../bike-rs/bike-temporal/src/tests/imports.rs),
  [batch tests](../../bike-rs/bike-temporal/src/tests/batches.rs),
  [composition tests](../../bike-rs/bike-temporal/src/tests/composition.rs), and
  [producer tests](../../bike-rs/bike-temporal/src/tests/client.rs) cover these paths.
  Domain suites cover retained bytes, selection, writes, and actual calculations.

### Earlier Strava processors

- Before the separate Strava cutover, the SQL worker also
  [registered ten provider processors](https://github.com/ericbutera/bike/blob/7d811e85cc1f1992a99002b0d609e7cbd9d9bdef/bike-rs/worker/src/tasks/strava/mod.rs).
  Include these in the inventory so the later 12-task baseline does not hide
  them. The seven provider operations are native; the three infrastructure
  processors below have explicit retirement decisions.

| Former processor         | Current disposition                                                                                                                                                                                      |
| ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `strava_activity_create` | `CreateActivity` accepts a create and awaits its upsert.                                                                                                                                                 |
| `strava_activity_update` | `UpdateActivity` accepts an update and awaits its upsert.                                                                                                                                                |
| `strava_activity_upsert` | `UpsertActivity` fetches, normalizes, saves, and refreshes through its source child.                                                                                                                     |
| `strava_activity_delete` | `DeleteActivity` captures affected data, deletes, and refreshes through its source child.                                                                                                                |
| `strava_sync`            | `SyncActivities` pages provider activities, awaits upserts, and advances the connection offset.                                                                                                          |
| `strava_backfill`        | `BackfillActivities` processes a fixed range without advancing the normal sync offset.                                                                                                                   |
| `strava_disconnect`      | `DisconnectStrava` deletes owned provider data, optionally revokes access, and removes the connection.                                                                                                   |
| `strava_reconcile`       | Automatic polling is retired. Explicit `ScheduledSync` operator runs can still scan connections and await sync children.                                                                                 |
| `strava_retention`       | The hourly/startup SQL cleanup processor is retired. Retained provider/source bytes remain available; disposable local scratch still cleans up. No automatic replacement sweeper is part of this worker. |
| `strava_workflow`        | The historical SQL dispatcher is retired; clients start the specific native provider workflow directly.                                                                                                  |

- [Provider workflow registration](../../bike-rs/strava-temporal/src/client.rs)
  and [the provider contract](#strava-protocol-and-workflows) own the current
  implementation and details. Retired infrastructure behavior is not claimed
  as a 1:1 conversion or silently counted as another native task.

## Processing flow

```mermaid
flowchart TD
    File[Retained activity file] --> Parse[Parse and save activity]
    Parse --> Segments[Rebuild segments and segment results]
    Segments --> Achievements[Rebuild achievements]
    Parse --> Heatmap[Rebuild heatmap]
    Parse --> Fitness[Rebuild fitness and training where needed]
```

- The Bike task workflow calls these named operations explicitly, through
  Activities or child workflows where a task has its own steps. Reuse the existing
  registered task names; an achievement operation does not require a new queue,
  registry, or scheduler.
- Save the activity before starting derived work. Achievements that depend on
  segment results run after that segment work. Independent children may overlap,
  with bounded concurrency.
- A request-owned parent normally awaits its children and returns or fails.
  While work is in flight, it is simply running. The API already returned after
  acceptance; it does not wait for computations.
- Derived data may become visible at different times. If a later step fails,
  preserve the source and successful effects, show the failed step in Temporal,
  and retry. There is no atomic all-output publication requirement.
- Reprocessing may parse the file again and rebuild already computed outputs.
  That is an accepted cost. Prefer this to recovering an exact previous execution
  result or coordinating every producer through shared output owners.

### Activity climbs and stage backfills

- After saving an activity, imports and reprocessing run `rebuild_activity_climbs`
  independently of the segment/heatmap/training branch. Reuse the existing Rust
  climb detector and the stored normalized recording.
- Save an immutable, content-addressed climb artifact with the existing object
  store. `activity_climb_outputs` holds its reference, input checksum, detector
  version and climb count; `activity_climbs` stores ordered climb bounds and
  metrics for consumers. Publish both in one transaction against the current
  owned activity. Repeating unchanged input reuses the saved output.
- An empty climb artifact is a successful result. Keep its output row even when
  there are no child climb rows, so a flat or GPS-less activity is distinguishable
  from an activity that has not been processed.
- For the initial rollout, start `backfill_activity_climbs` on `bike-tasks` in the
  `bike` namespace with JSON input `0`, using the existing Temporal UI or CLI.
  It selects activities missing the current climb output, processes at most 16
  per page through the same climb operation, and Continues-As-New after settling
  a full page. Temporal owns progress and retries. Restarting from zero skips
  outputs already saved, including empty results.
- Roll out future stages the same way: persist their actual output, including an
  empty result, then run a one-off paged workflow for missing outputs. A changed
  calculation can select an older output version. Backfill only that stage;
  existing efforts and awards do not need full import reprocessing.
- This slice generates climb data. Activity screens switch to these saved rows
  in a subsequent change.

## Identity and causal context

- Pass the owned import/file reference and activity ID when known. New imports
  resolve the activity during save; reprocessing preserves its activity/import IDs.
- Use a stable native workflow identity for a retry of the same submission.
  Include the target in that identity so different activities cannot attach to
  the same job. An intentional rerun can start a new execution.
- Keep useful owner, actor, workflow/run, and trace IDs in existing inputs/logs.
  No separate command registry, admission ledger, contributor list, or persisted
  command snapshot is required.
- A duplicate submission may return its existing native job. Do not require
  reading and decoding the first history event just to acknowledge every start.
  Authorization remains an ordinary owned-record check.

## Fast durable acceptance

- Bike retains the input file, starts the workflow, and returns HTTP 202 with
  the native job reference after acknowledgment. Parsing and derived work run
  in the worker. Worker downtime does not require synchronous execution in the API.
- A failed or uncertain start returns an error; retain the file for retry.
  Do not acknowledge work held only in API memory.
- Uploading the same retained file again starts processing when it has no saved
  activity yet. Completed imports return their existing activity as duplicates.
- Preserve Strava request history and bounded callback acceptance. The 1.5-second
  deadline covers lookup, validation, persistence, and Temporal start acknowledgment
  within the two-second callback budget. Timeouts and uncertain starts return
  non-2xx; canonical event identity handles another delivery.
- Requests start their native workflow directly. There is no SQL inbox scanner
  or background retry dispatcher. Keep registered callback aliases. Worker downtime
  is acceptable once Temporal has acknowledged the start; Temporal unavailability
  returns an error to the caller.
- The Strava connection extractors authenticate once and load the owned
  connection once. Sync, backfill, and disconnect require that connection;
  connection GET allows absence while retaining authentication failures.
- `integrations::strava` owns provider calls, OAuth, and request acceptance.
  Core owns stored models, their event contracts, retained activity-file parsing,
  and shared activity persistence; it does not depend on the integrations crate.
- User-requested Strava work accepts the extracted user and connection, validates
  them, commits the request, and starts Temporal without reloading either record. Sync and
  backfill return that connection snapshot. OAuth completion reuses its newly
  saved connection. Later deletion or a failed read cannot undo acceptance.
- Connection response formatting is a pure mapping of the available connection
  snapshot. Reading current state belongs to the connection GET operation.

## Idempotent effects and activity history

- Parse retained bytes and save through ordinary owned-model operations.
  Use existing import/provider uniqueness, upserts, replacement writes, and
  transactions so repeating work does not create duplicate activities or efforts.
- Preserve ownership, user-edited fields, deletion, and privacy rules. Keep a
  local version check or lock where an observed domain race needs it; there is
  no universal execution-generation or exactly-once protocol.
- Reprocessing a retained file preserves the saved title and Training/Race
  selection. Initial imports use the file's title; re-reading it must not undo
  a later edit in Bike.
- Rebuild projections from current stored activities/efforts. Replacement either
  commits or rolls back. Keep the existing transaction fix for overlapping segment
  achievement updates and bounded bulk writes; simplicity does not require
  reintroducing the observed duplicate-key bug.
- When rematching removes a segment membership, refresh the removed segment too.
  Use the old/new IDs during the normal operation; when a retry no longer knows
  the old set, a broader rebuild is acceptable. Do not persist a result receipt
  solely to recover the original minimal refresh set.
- Activity-change history remains useful domain history. Preserve its existing
  transactional consistency, but remove execution decisions based on an audit
  entry's `refresh_scope`, exact original outcome, or one-audit-per-attempt count.
  A workflow does not need an audit record to prove its progress.
- Existing Strava source mappings, connection generations, and deletion state
  serve provider ownership/data rules. They are not a generic task ledger.
- No new execution, receipt, result, readiness, publication, or reconciliation
  accounting tables are part of this migration.

## Cancellation, recovery, and completion

- Use native retries and cooperative cancellation. An already executing Activity
  may finish and write data after cancellation is requested. Partial processing
  is acceptable; do not promise rollback or instantaneous interruption.
- Stop starting later work when cancellation is observed at normal step or batch
  boundaries. Preserve the file and completed effects for a later rerun.
- Remove custom commit-boundary/cancellation handshakes and special finalization
  protocols. Native status describes what happened; no separate cancellation
  accounting is required.
- Worker restart resumes native execution. A repeated Activity may reparse or
  recompute; ordinary domain writes make that safe. Exhausted failures remain
  visible for retry from the retained source.
- Completion means the workflow's own steps finished. It does not certify that
  every shared projection across Bike is simultaneously current.
- Strava's provider-specific cancellation behavior is described below. Its
  native cancellation tokens do not change Bike task cancellation or add a
  database handshake, result ledger, or shared-output coordinator.

## Reprocessing implementation

- `POST /admin/activity-imports/reprocess-activity` remains the entry point.
  It checks the owned retained source, submits the native workflow, and returns
  the job. Keep the existing UI command reuse for transport retries where useful.
- Read primary/supplemental files through the shared `ObjectStore`, verify their
  integrity, and reuse the parser and enrichment code.
- Source processing reparses the retained file through
  [source processing](../../bike-rs/bike-core/src/activity_reprocessing.rs) and
  saves through [the owning model](../../bike-rs/bike-core/src/entities/activities/reprocessing.rs).
- The [workflow](../../bike-rs/bike-temporal/src/workflows/reprocess_activity_import.rs)
  and `process_activity_import` save through the same import Activity, then call one
  [shared derived sequence](../../bike-rs/bike-temporal/src/workflows/steps/derived.rs):
  source save → segments → achievements → heatmap → training → fitness.
  Both await the same `rebuild_segment_analytics`, `prepare_heatmap`, and
  `rebuild_fitness_freshness` children, with the same activity-level analytics
  and training operations. Keep the priority and saved identity throughout.
  [Named derived steps](../../bike-rs/bike-temporal/src/workflows/steps/derived.rs)
  use awaited child workflows and Activities without scheduling RPCs or signals.
- Segment matching also refreshes the user's existing segment-summary scope.
  This broader rebuild covers a removed membership after a repeated matching
  attempt without retaining the earlier execution's result.
- Domain calls write their domain outputs only. No pipeline-output ledger remains.
- Retired coordinator histories from before the completed simplification cutover
  cannot be reset/replayed with the current worker. Sharing the import sequence
  preserves its Temporal commands and ordering; it does not require inbox draining.
- Preserve canonical source keys, activity/import identity, and valid user data.
  Native Strava sources stay on provider sync/backfill. Legacy files without a
  recorded checksum remain readable; verify their stored size and parse the file.

## Bike workflow composition and bounded batches

- Keep the [task inventory](#task-inventory-and-behavior-parity).
  Import and reprocess reuse source processing and downstream domain tasks.
  Collection workflows loop over items and call the individual task.
- Use ordinary paging or an archive-entry iterator and bounded child concurrency.
  A compact extracted-file index is fine when needed to find files; preparing an
  immutable command manifest for every user/batch is not required.
- Await children for the current page. A child failure remains visible and fails
  the parent; completed children are not rolled back.
- Use native Continue-As-New for long loops, carrying the selection, retained
  file reference, and next cursor. Repeating a page/entry is acceptable.
  Do not add exact settled-count, per-entry outcome, or cross-run result ledgers.
- Counts may help display progress but are not the recovery authority. Use source
  files, domain records, and native execution history to inspect or rerun work.
- User batches process 16 activities per page, awaiting each child before advancing
  the ID cursor with Continue-As-New. Training backfills calculate each activity
  in the same page loop. Archive workflows retain the ZIP in `archives/{job_id}.zip`,
  extract at most 16 files, await their imports, and continue with the next offset.
- Upload and reprocess workflows call the ordinary heatmap child. Manual activity
  edits submit heatmap preparation after committing the edit. A request retry may
  repeat that submission; native/domain idempotency handles repeated computation.
- Do not create periodic heatmap reconciliation or all-connection Strava sync
  schedules. Existing reconciliation/scan workflows remain available for explicit
  operator repairs and retained-history replay. They are never started on a timer.
- If heatmap preparation discovers stronger recording evidence, persist it and
  return a retryable Activity failure. Native retry reloads the new projection
  generation and completes preparation within the same requested workflow.
- Unit fixtures execute the parent and child workflow code for archive processing
  and user reprocessing, including their segment, heatmap, and fitness children.
  Assert that parents remain pending until children finish, child failures stop
  later work, and full pages continue only after their work completes. Keep
  retained-file parsing and PostgreSQL write checks in the owning domain suites.

## Eventual consistency, debounce, and priority

- Accept temporarily stale fitness, segments, achievements, and heatmaps during
  processing. Read existing stored output; show native work status where useful.
- Explicit child calls are the default. Mandatory Signal-With-Start output
  coordinators, merge buffers, revision watermarks, and publication barriers are
  removed from the target design.
- Use existing native task priorities to keep archive/backfill work in the
  background where useful. Running work need not be preempted.
- Measure before introducing debounce or fairness tuning. A concrete performance
  problem may justify a small change later; a general accumulator/coordinator
  framework is not a prerequisite for processing a ride.

## Shared object storage and retention

- Reuse `ObjectStore` and compact `ObjectRef { path, sha256, bytes }` values.
  Keep existing logical keys, namespace ownership, and integrity verification.
  Source bytes belong in the retained store, not Temporal payloads.
- Uploads, supplemental artifacts, archive ZIPs, and extracted activity files
  read/write retained bytes through this wrapper. The worker injects one backend;
  archive and import operations select `archives/` and `activity-imports/` namespaces
  on that backend. Worker domain code must not recreate a filesystem backend or join
  retained keys to a worker-local upload directory.
- Preserve existing import keys/extensions and `archives/{job_id}.zip`. Archive
  continuation carries that logical key, never an absolute worker-host path.
  Seekable ZIP parsing uses streamed disposable local scratch, removed on return
  or failure. The filesystem adapter uses `.scratch/` on the writable storage
  mount, keeping the production root filesystem read-only. Bound multipart upload
  concurrency and memory; do not load a whole large ZIP into a Temporal payload
  or an in-memory byte buffer.
- Keep canonical activity/import files available for reprocessing, including
  after failed or canceled work. Deleting native execution history must not
  delete the activity's retained source.
- Keep existing safe temporary-file cleanup. A new reference/pin inventory,
  workflow-history scan, or accounting-based garbage collector is outside this
  conversion. If uncertain whether a file is still needed, retain it.
- Reclaim obsolete scratch separately, using a clear storage rule that does not
  remove canonical files. Do not make storage redesign a dependency of ingestion.

## Large archive durability

- Retain the archive before processing. Stream extraction with bounded memory,
  save each activity file, and call the ordinary import task.
- Carry a compact entry/page cursor through native continuation. A useful native
  heartbeat may report extraction progress, but restarting extraction or repeating
  the current entry after interruption is acceptable.
- Preserve bytes so an expired download URL is not required for a rerun.
  Use normal import identity/upserts to avoid duplicate activities.
- No exact success/duplicate/failure accounting or custom pause/resume protocol
  is required. Verify bounded memory, multiple pages, a failed child, and restart
  from retained input; extend tests when an observed failure warrants it.

## Strava protocol and workflows

- Target intake is Strava → `/api/strava/webhook` → bounded durable acceptance.
  Preserve registered `/webhooks/strava-gateway` and `/oauth/gateway/callback`
  as aliases to native handlers, with existing ingress/TLS; no gateway code returns.
- Activity `create` / `update` / `delete` start dedicated native workflows.
  Athlete `update` with `authorized=false` disconnects without remote revoke.
  Other combinations and unconnected athletes are acknowledged and ignored.
  [Typed intake](../../bike-rs/integrations/src/strava/events/intake.rs) and
  [client registration](../../bike-rs/strava-temporal/src/client.rs) own mapping;
  there is no generic Strava processor.
- Retain all activity summaries, including manual entries and non-cycling sports,
  with normalized sport, title, distance, duration, reported elevation gain,
  description, and gear ID/name when supplied. Missing gear name preserves its
  ID without another call. Store complete provider responses in object storage.
- Fetch streams for recorded activities of every sport so supplied GPS can feed
  [activity maps](maps.md#rider-experience). Keep the provider payload, parsed data,
  and stream bytes behind the existing object-storage wrapper.
- Manual entries skip stream fetching. A stream endpoint's 404 means unavailable
  recording data: retain the summary with empty streams. Other failures, including
  permission errors and rate limits, retain their normal failure/retry behavior.
  Invent no GPS or sensor data. Prefer reported elevation gain over samples. Apply the
  [central sport policy](supported-activities.md#central-activity-vocabulary-and-retention)
  to exclude non-cycling activities from cycling calculations while keeping them
  visible in the feed; preserve unknown provider sports.

| Workflow                        | Input                      | Source steps and result                                               |
| ------------------------------- | -------------------------- | --------------------------------------------------------------------- |
| CreateActivity / UpdateActivity | Strava event               | Await Upsert child; return Bike ID/status                             |
| UpsertActivity / DeleteActivity | Event + remote activity ID | Await source-keyed ReconcileActivity; return ID/status                |
| ReconcileActivity               | Event + remote activity ID | Fetch → normalize → persist → refresh, or capture → delete → refresh  |
| SyncActivities                  | Event + saved offset       | List pages → await upsert children → commit offset; counts/new offset |
| BackfillActivities              | Event + fixed range        | List range → await upsert children; counts without offset change      |
| DisconnectStrava                | Event + revoke flag        | Delete children → optional revoke → remove connection                 |
| ScheduledSync                   | Schedule context           | List connections → await sync children; per-connection results        |

- These are the nine workflows (Create and Update are separate). Review
  [their explicit implementations](../../bike-rs/strava-temporal/src/workflows.rs)
  and [typed Activities](../../bike-rs/strava-temporal/src/activities.rs).
  Call steps as `run(ctx, RevokeConnection, event).await?`; shared helpers adapt
  SDK failures/options and derive display names from registered markers.
- The workflow table describes the existing provider sequence. Keep its explicit
  fetch/normalize/persist/refresh shape when reusing Bike domain tasks. Per-item
  rebuilds are acceptable; shared-output coordination is not a conversion gate.
- Provider single-event parents await request-owned children and preserve the
  child's result: canceled preparation reports cancellation; a started mutation
  reports its actual completion or failure. Collections settle the current child,
  then stop before the next item or cursor update. Parent close requests child
  cancellation and waits for the child to settle.
- Native patch `strava-commit-boundary-v1` preserves retained histories. New
  provider runs check cancellation before preparation and shield a started
  mutation and its downstream steps with independent native cancellation tokens.
  Disconnect shields its initial state write and final revoke/removal separately;
  item boundaries remain cancellable. This does not promise atomic publication.
- SQL import recovery, bulk replay, and historical source repair must exclude
  `strava_sync`, including interrupted native publication. Use native sync/backfill;
  do not call legacy processors or create SQL continuation/checkpoint tasks.
- `strava_events.status` is authoritative mutation status: `pending`, `applied`,
  `unchanged`, `deleted`, `already_absent`, `superseded`. `captured` is transient
  deletion input. Use typed operations/mutation/attempt enums; count children by
  the status column rather than JSON or a status relation.
- Status and retry dependencies commit atomically. Connection/customer validation
  is split into named operations; DB failures remain server errors and missing
  customers are not found. Required remote IDs live on `StravaEvent`; source-state
  lookup/absence/content/supersession predicates live on the owning model.
- Explicit operator scans continue starting later customers after a child-start failure,
  await every started child, and carry total failures through continuation.
  Cancellation remains cancellation; a nonzero final failure count fails the scan.
- Use primary-key-aware SeaORM save for import/artifact create/update; separate
  model construction, saving, and provider-artifact linking. Schema retirement and
  status promotion use new migrations; never rewrite migration history.

### Callback security

- `/oauth/gateway/callback` completes **Connect Strava**: verify Bike's signed
  OAuth `state`, exchange the authorization code with Strava, save the connection,
  and request sync. It does not authenticate webhook POSTs.
- Strava returns the OAuth `state` that Bike supplied; this is a Bike signature,
  not a Strava signature. State must also be bound to the initiating browser
  session, consumed once, and verified with the HMAC library's constant-time check.
- The webhook GET `hub.verify_token` check verifies subscription setup only.
  [Strava's published webhook protocol](https://developers.strava.com/docs/webhooks/)
  does not document a signature header for event POSTs.
- Subscription ID, athlete ID, event time, deduplication, and connection-generation
  checks do not authenticate the sender. TLS protects transport, not sender identity.
- Accept unsigned event POSTs on the registered callback routes. The absence of
  sender authentication is an accepted Strava protocol limitation, not a release gate.
  Do not require unavailable signatures or provider confirmation before processing
  supported delete and deauthorization events.
- Keep configured subscription checks, connected-athlete lookup, ownership and
  connection-generation validation, deduplication, and event ordering. These are
  processing checks; they do not prove the sender's identity.
- Track OAuth state hardening separately from webhook delivery. Cover webhook
  validation/deduplication and OAuth state protections with isolated mocked units.
- Connect creates a random, HttpOnly `strava_oauth_session` cookie and a signed
  state for the authenticated Bike user. The cookie is host-only, `SameSite=Lax`,
  has path `/` for both callback aliases, and lasts ten minutes. HTTPS uses the
  Secure `__Host-strava_oauth_session` cookie; sibling domains cannot set it.
- Verify the state signature and browser binding with `Hmac::verify_slice` before
  consuming it from the existing `oauth_states` store under provider `strava`.
  Reject malformed, future, expired, mismatched, or already consumed state before
  exchanging any provider code. Consume valid denied callbacks too; clear the
  cookie on every callback. A failed exchange requires a new Connect request.
- Test handler policy with fixture providers, in-memory object storage, and mocked
  persistence. No unit test opens a database or contacts a running service.

### Source ordering and reconciliation

- Retain raw `owner_id`, `object_id`, `object_type`, `aspect_type`, `event_time`,
  `subscription_id`, and `updates`. Interpret IDs by object type: an athlete
  deauthorization is not an activity. Deduplicate canonical payloads within the
  connection generation; owner/object/time alone can collapse distinct updates.
- Bike assigns event IDs. Event identity spans one operation; source identity
  spans create/update/delete. Persist the remote-to-Bike mapping in the same
  transaction as create, update, unchanged recognition, or deletion. Backfill
  known live mappings through owning connection/generation without inventing history.

| Stored timestamp                                | Meaning                                                      |
| ----------------------------------------------- | ------------------------------------------------------------ |
| State `event_time`                              | Last accepted provider event, epoch seconds                  |
| State `deleted_at`                              | Provider time of current deletion; cleared by a newer upsert |
| State `updated_at` / compatibility `applied_at` | Synchronized local write version, advanced on every write    |
| History `occurred_at`                           | Source event/manual change time                              |
| History `recorded_at`                           | Bike archive time                                            |

- Never compare local sync/backfill request/receive time with a provider watermark,
  or use activity start time as event order. Derive provider time only from the
  retained webhook; zero means unknown. Sync/backfill preserves the watermark,
  and a new connection generation resets it. Repair old mixed values from
  committed provider mutations, never from receipt or sync time.
- Upsert/Delete await a request-owned `ReconcileActivity` child keyed by owner,
  connection generation, athlete, and remote activity. A busy source retries
  after a native timer; distinct events each fetch and apply rather than attaching
  to another request's result. Continue a waiting parent after 128 attempts or
  Temporal's history suggestion, preserving its original event and job identity.
- Capture the source row version before provider I/O. Under the existing
  connection lock, return any committed retry result, then check that version.
  If the source changed, refetch using a response key containing the new version.
  Atomically advance the existing timestamp token on writes; no new table or
  execution ledger is required. A generation change clears an absent Bike mapping.
- The native `strava-source-reconciliation-v1` patch preserves retained inline
  histories. Drain started provider runs before the data repair and worker
  transition; keep callback acceptance available during deployment.
- Reject clearly older events within the same owner/object/generation. Deletion
  wins an equal-time upsert; only a strictly newer upsert restores it. Ignore
  deauthorization older than the connection; delayed old generations cannot write.
- Distinct equal-second provider updates may require another fetch. Keep the
  existing canonical deduplication and owned writes; do not introduce a universal
  source-serialization workflow as a condition of converting file ingestion.
- Preserve atomic deletion and provider state updates. Repeated processing or a
  broader derived rebuild is acceptable; exact recovery of the first mutation
  outcome is not a migration requirement.

## Temporal execution policy

- Immutable retained-input integrity and JSON decoding failures use the existing
  typed domain error (`422 Unprocessable Entity`); pure file parser failures are
  also terminal client errors. Missing immutable input is terminal. Native
  Activity error adaptation classifies these without matching message text.
  Backend I/O/timeouts remain server errors and retryable. Repair or recover the
  retained source before intentionally submitting new work; retries cannot fix
  the same checksum, size, or malformed bytes. Primary and supplemental sources
  obey the same rule before any mutation.
- [Shared platform policy](../../bike-rs/bike-core/src/platform/temporal.rs) owns
  native retry-policy factories and provider timeouts. Bike's
  [Activity options](../../bike-rs/bike-temporal/src/workflows/options.rs) own its
  longer execution timeout; workflow steps do not build policies.
- Keep retry settings explicit: initial interval, backoff coefficient, maximum
  interval, and maximum attempts. Attempts include the initial execution.
- Preserve current execution budgets:

| Activity role        | Execution timeout      | Heartbeat timeout | Retry factory             | Total attempts | Initial interval | Backoff | Maximum interval |
| -------------------- | ---------------------- | ----------------- | ------------------------- | -------------- | ---------------- | ------- | ---------------- |
| Bike domain effect   | 30 minutes per attempt | None by default   | `activity_retry_policy()` | 12             | 1 second         | 2×      | 1 minute         |
| Strava domain effect | 2 minutes per attempt  | 30 seconds        | `activity_retry_policy()` | 12             | 1 second         | 2×      | 1 minute         |

- These are current finite attempt budgets, not a guarantee of eventual success
  after every outage. Adapters distinguish permanent invalid input from transient
  failure. Policy factories do not implement private retry loops.
- Workflow/child executions have no automatic execution retry policy. Activities
  own effect retries; Workflow Task retry/replay stays native. A deliberate whole
  workflow retry creates another run and still needs domain idempotency.
- Workflow code performs deterministic orchestration only. Keep database, files,
  network calls, random IDs, and wall-clock reads inside Activities or clients;
  use native workflow APIs for timers, cancellation, children, and continuation.
- Await every page's children before Continue-As-New; carry only the next cursor
  and retained input reference. Never continue with unfinished children.
- Bike Activities do not checkpoint with heartbeats. A lost worker can delay
  retry until the 30-minute attempt timeout and repeat that Activity's work;
  durable execution does not imply instant recovery or exactly-once effects.
- Provider `next_retry_delay` overrides computed backoff. Clamp expired waits to
  the shared initial interval. Heartbeat every **5 seconds**, below the shared
  **30-second** heartbeat timeout.
- Native SDK patch/version markers protect changed Strava decisions, including
  continuation inside a populated page, failed customer starts, input budgets,
  and future output handoffs. Old histories retain their previous decisions;
  verify replay rather than modifying deployed Activity sequences blindly.
- Verify emitted Activity options against the shared factories. Retry builders
  and named budgets stay out of steps.

## Payload and history budgets

- Retain raw activity responses, stream responses, normalized sources, and parsed
  activities in Bike's existing persistent upload-volume object store. Temporal
  carries their path, SHA-256, and byte count; source size does not affect input size.
- Limit each Strava value crossing a Temporal boundary to **256 KiB of serialized
  UTF-8 JSON**, including the complete event, nested values, and dependency IDs.
  This leaves room for SDK metadata below the server's warning threshold.
- Validate before durable webhook acceptance, workflow handoff, activity/child
  scheduling, continue-as-new, and returning activity results. Oversized callbacks
  return HTTP 413. Oversized worker values produce non-retryable application
  failures, with size and limit in the error and no provider body in the message.
- Temporal Server 1.32.1 defaults: payload warning **512 KiB**, payload rejection
  **2 MiB**, event-history transaction maximum **4 MiB**, history warning **10 MiB**
  or **10,240 events**, history rejection **50 MiB** or **51,200 events**. The deployed
  dynamic configuration does not override these limits. Source:
  [versioned server defaults](https://github.com/temporalio/temporal/blob/v1.32.1/common/dynamicconfig/constants.go)
  and [Temporal service limits](https://docs.temporal.io/self-hosted-guide/defaults).
- Sync and backfill list 100 activities per provider page. Continue as new after
  25 pages or the server's history suggestion. Check the suggestion after every
  completed child, preserving the remaining page IDs in continuation input.
  Carry the original fixed window, next page, counts, latest start,
  event/correlation IDs, request ID, and trace carrier. Advance the sync cursor
  only after the workflow's children finish. Backfill never advances it.
  Existing provider counts may remain; no extra result accounting is required.
- Explicit operator scans continue as new after each 20-connection page. Await every
  started customer, carry failures and correlation ID into the next run, and fail
  the final run with the cumulative count when any customer failed.

| Temporal activity                                                                                                                    | Input → output                                                               |
| ------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------- |
| `strava_fetch_activity`                                                                                                              | StravaEvent → ActivitySource (activity object reference)                     |
| `strava_fetch_streams`                                                                                                               | ActivitySource → ActivitySources (activity + streams references)             |
| `strava_normalize_activity`                                                                                                          | ActivitySources → NormalizedActivity (source + optional parsed reference)    |
| `strava_persist_activity`                                                                                                            | NormalizedActivity → ActivityChange (Bike ID, status, affected dependencies) |
| `strava_capture_deletion`                                                                                                            | StravaEvent → ActivityChange                                                 |
| `strava_delete_activity`                                                                                                             | ActivityChange → ActivityChange                                              |
| `strava_delete_source`                                                                                                               | ActivityChange → ActivityChange                                              |
| `strava_rebuild_activity`, `strava_rebuild_segments`, `strava_rebuild_fitness`, `strava_publish_heatmap`, `strava_complete_activity` | ActivityChange → ActivityChange; each method performs its named step         |
| `strava_resolve_collection_window`                                                                                                   | StravaEvent → CollectionInput (fixed date range)                             |
| `strava_list_activity_page`                                                                                                          | PageInput → ActivityPage (IDs, next page, latest start)                      |
| `strava_accept_child_event`                                                                                                          | ChildEvent → canonical StravaEvent                                           |
| `strava_complete_sync`, `strava_complete_backfill`                                                                                   | CollectionResult → CollectionResult; only sync advances the offset           |
| `strava_begin_disconnect`                                                                                                            | StravaEvent → up to 50 owned remote IDs; blocks new writes                   |
| `strava_revoke_connection`, `strava_remove_connection`                                                                               | StravaEvent → StravaEvent                                                    |
| `strava_scan_connections`                                                                                                            | ScanRequest → ScanPage (up to 20 customer sync events)                       |

- Typed domain values live in
  [the stored event contract](../../bike-rs/bike-core/src/entities/strava_events/contracts.rs).
  The table describes the existing provider Activity sequence. Changes must
  preserve retained-history compatibility; reuse domain operations without
  introducing a second execution accounting system.

## Logs, job visibility, and alerts

### Manual admin actions

- Activity and user grids expose a shared gear menu. Resolve targets from the row;
  never ask an administrator to retype the owner or activity ID. Existing import
  details and account editing remain available independently.
- `POST /admin/activities/{id}/tasks` supports retained-file reprocessing,
  rebuilding the owner's fitness/freshness, activity segments/achievements,
  heatmap preparation, and resyncing the mapped Strava activity.
- `POST /admin/users/{id}/tasks` supports fitness/freshness, user segment
  regeneration, all retained imports or archive FIT imports only, XC training
  backfill, Strava incremental sync, and Strava backfill with an explicit date
  range. The UI explains the scope before submission; user-wide work never
  appears to affect only the selected activity.
- Both routes require administrator authentication. The server loads the target
  and derives ownership. Strava resync resolves the remote ID through the current
  connection generation's live mapping; no caller-supplied athlete or remote ID.
  Missing connections/mappings and invalid ranges fail visibly.
- Accept only named actions and their typed parameters. A client-generated
  command UUID survives uncertain/failed submissions. Bind native IDs to the
  action, target and parameters; repeated clicks cannot create duplicate runs
  for the same intent. Return 202 only after Temporal acknowledges, with a job
  link. Request acceptance does not mean the workflow has completed.
- Reuse the existing workflow/domain implementations, native priorities, paging
  and child ordering. Do not create an admin SQL queue, scheduler or execution
  ledger. Strava sync/backfill/resync use the existing persisted-event intake,
  including scopes, connection checks, canonical identity and integration events.
- The new manual endpoints do not reset legacy cached execution status. In
  particular, retrying an XC command must not replace a completed backfill's
  preference state with `queued`. Its existing workflow still rebuilds training
  analysis and marks completion; the admin dialog follows the native job.
- Record manual acceptance and submission failures in existing integration
  events, with initiating administrator, affected user/activity, command/request
  IDs, chosen parameters and the acknowledged job reference. Preserve Strava's
  accepted-event and activity-change history. Native history remains authoritative
  for later execution outcomes; do not label accepted work completed.
- Segment-wide operations, archive ingestion, global analytics prewarming, and
  duplicate cleanup remain on their existing parameter-specific tools. Cleanup
  remains a synchronous domain operation that enqueues affected cache rebuilds;
  ordinary uploads already submit import workflows.
  Strava create/update/delete are provider events, not arbitrary admin commands;
  use resync for current remote content. Disconnect remains the account's explicit
  destructive operation. Retired email/SQL dispatch/schedulers stay retired.
- Unit fixtures verify every exposed action's native name/input, target lookup,
  access control, retry identity, rejection behavior and event records. UI tests
  exercise gear menus, owner scope, date/FIT parameters, job links and errors.

### Correlation and execution history

- `/admin/tasks` provides a small read-only history table for retained Bike and
  Strava workflow runs, including children: task type, native status, start/end
  times, status and user filters, cursor paging, refresh, and original input parameters.
- Apply the positive-integer `user_id` filter in Temporal visibility using
  `BikeUserId` before pagination. New owned Bike submissions and children record
  that keyword, as Strava submissions already do. Historical runs without the
  attribute and tasks with no single owner appear only in the unfiltered list.
- Use the shared bottom pagination control with known page numbers and the next
  cursor. Never claim an unknown total or last page. Reset cursors when filters
  change; use the Users page's compact right-aligned Search/Clear toolbar.
- Administrator-only `GET /admin/tasks` reads Temporal visibility in the
  configured namespace and the two worker queues. Fetch 25 executions per page,
  in [Temporal's native order](https://docs.temporal.io/list-filter): running
  first, then recently closed. Do not send unsupported `ORDER BY` clauses;
  pass Temporal's opaque cursor through the API. Visibility may
  briefly lag new submissions. Native `Running` includes work waiting for a worker.
- `GET /admin/tasks/{workflow_id}/runs/{run_id}/input` reads the first history
  event only when an administrator opens Parameters. Scope inputs to the same
  worker queues and exact run ID. Display JSON parameters and a link to that
  native run; unavailable/expired history remains an explicit error.
- Retention follows Temporal. No new SQL history table, polling service,
  lifecycle reconstruction, or execution ledger. Integration events remain the
  existing record of submission and domain changes. Detailed retries and
  cancellation controls remain in Temporal.

- Use **Strava event**, never receipt. `correlation_id` is the root Bike-assigned
  Strava event ID, unchanged through children; scheduled scans use their first
  native run UUID. Preserve `strava_event_id`, `bike_customer_id` (public UUID),
  `bike_user_id`, optional `bike_activity_id`, `strava_athlete_id`, and
  `strava_activity_id` separately.
- Add namespace/workflow/run/Activity/attempt IDs at their corresponding boundary.
  Provider exchanges add exchange ID, method/path, status, and provider request ID.
  Never log credentials or provider bodies. Typed inputs retain event/customer/
  activity context for acceptance, attempts, exchanges, and terminal results.
- Parent/child logs link both native workflow/run pairs and preserve root request/
  trace context. Lifecycle logs distinguish success, failure, cancellation, and
  continuation. Suppress replay logs without making workflow decisions depend
  on tracing state. Sort serialized carrier keys for byte-stable reconstruction.
- Use the [operation tracing contract](observability.md) for shared INFO stage
  names, selected IDs/counts, DB ancestry, and hotspot interpretation. Export
  existing Activity attempt spans under their registered step names; do not add
  redundant spans or instrument every helper/poll. Handoff success and failure
  use the same correlation span.
- Open the admin integration feed, select a user, and use **View job in Temporal**.
  Local UI: `http://localhost:8080/namespaces/bike/workflows`. Search
  `BikeCustomerId = 'customer-uuid'` or `StravaActivityId = 'remote-id'`; logs use
  the same IDs. Temporal owns execution status; accepted-event history records
  requests. Account sync status reflects connection state. Do not reconstruct
  retired SQL stages or fabricate completion events.
- Submission failures return to the initiating request and are logged with its
  correlation context. There is no periodic inbox/backlog sample. Monitor provider
  requests and native Temporal execution metrics. Retire inbox backlog alerts with
  the dispatcher; do not alert on intentionally absent retired metrics.

| Alert                       | Condition / response                                                                                                                                                                                                                                |
| --------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `BikeWorkerFailures`        | Terminal `bike-tasks` workflow failure in the last hour for one minute, including first series/reset. Inspect native history and failed children before retrying; Activity retries alone do not page.                                               |
| `BikeStravaWorkflowFailed`  | Terminal workflow failure in last six hours for one minute, including first series/reset. Warn through Alertmanager/ntfy; Activity retries do not page. Root/child can both alert. Inspect the failed step and partial effects before native retry. |
| `BikeStravaWebhookRejected` | Rejected callback in the last hour. Inspect the request logs, correct acceptance or provider configuration, then redeliver or explicitly sync the missed activity. SQL event persistence alone does not mean Temporal accepted the workflow.        |

- Grafana's **Bike Worker** dashboard owns native execution visibility for both
  `bike-tasks` and `bike-strava`; Overview and System Health summarize it.
  Show terminal workflow outcomes separately from failed Activity attempts,
  native pollers/slots and Activity execution/schedule-to-start latency.
  Slots do not represent queue backlog. Both provider and SDK scrape endpoints
  must be reachable for worker health.
- Strava panels use the combined `temporal-worker` provider metrics. Logs and
  traces follow the combined process identity and link to native jobs; retire
  SQL queue, anomaly, processor, stage, and handoff-sampler panels and alerts.
- Selected-range counts are scrape-based estimates and count parent/child runs
  separately, not unique imported activities. Missing series remain unknown/N/A;
  Temporal history owns exact job state. Shared Temporal remains a server-health
  dashboard. Dashboard definitions belong to the deployment repository.

## Verification and acceptance scenarios

- Use existing shared fixtures, PostgreSQL SeaORM mocks, object-store fakes, and
  the native SDK harness. Unit tests cover happy paths and concrete regressions.
  No SQLite substitute, separate E2E framework, or real provider calls in units.
- Test the actual readable workflow: parse/save before derived children, segments
  before dependent achievements, and a failed child visible as failure.
- Reprocess the same retained file twice: keep activity/import identity and
  correct outputs. Test a transient Activity failure, corrupt input, deletion/
  privacy checks, and rerun after partial completion. Repeated computation passes.
- Exercise a multi-page batch, a failed child, and continuation/restart from
  retained input. Do not require exact once-only counts or receipt reconstruction.
- Keep Strava's protocol, OAuth, ownership, payload-limit, and paging regressions.
  Their detailed fixtures remain in the existing suites; the superseded U/P
  accounting matrix does not govern the simplified migration.
- Use the owning Docker/mise unit tasks with fixture-backed workflow children,
  object storage, and PostgreSQL mocks. Local checks must establish the converted
  paths work without waiting for CI. Warnings, lint, and required hooks remain blocking.
- Before deploying changed workflow sequences, verify retained native histories
  still replay or drain the old runs. Use existing replay tools. For the actual
  converted path, verify one real file's downstream results and a worker restart.
  Mock success alone is not deployed evidence.

## Deployment and compatibility

- The SQL worker and its queue/batch/stage/recovery/UI/build code are retired.
  Preserve historical migrations and the append-only retirement migration;
  activities, imports, source files, and activity history remain domain data.
- Keep the combined `temporal-worker`. Changes to deployed workflow commands must
  replay existing histories, use native patching, or explicitly drain the old
  executions before rollout. Do not restore retired coordinators to replay them.
- Record completed checks and user-visible verification below; keep remaining
  acceptance work in TODO. Keep local, CI, and deployed evidence distinct.
- For direct-submission rollout, update the API before removing the old worker
  relay. Stable workflow IDs tolerate overlap. A worker without a relay must not
  serve an old API that only records SQL events. Confirm the API can reach the
  configured Temporal namespace/queue. A failed start needs request retry or
  explicit resubmission; retained event rows are not an automatic retry queue.
- Source-repair writes, projection upgrades, or feature re-enablement must request
  heatmap preparation or an explicit `reconcile_heatmaps` repair. Pending database
  rows alone do not dispatch work.

## Cutover evidence

- This section consolidates the completed portions of the former worker rollout
  and polling audit. Git retains their detailed chronology. Superseded accounting,
  coordinator, SQL queue, and scheduler requirements are closed; unfinished live
  verification is tracked once in [TODO](../TODO.md#worker).
- **2026-10-17 SQL retirement:** intake and both old workers were paused after
  running work drained. Coordinated database/source backup
  `20261017T140351Z` validated on both disks. The append-only retirement migration
  removed all 13 execution tables, plus SQL worker deployment/service/scrapes and
  unused SMTP configuration. Domain records and retained files remain. The four
  application deployments and public health passed the owning release check.
- **Real task parity:** retained activity/import `1653` reprocessed through
  save, segments, achievements, heatmap, training, and fitness while preserving
  its title. Verified 13 efforts across seven segments, achievement/training
  caches, 32 user segment-summary rows, a ready version-5 heatmap, and cleared
  fitness dirty state. Batch imports, activity/user-segment regeneration, and
  training backfill completed on bounded tail pages; full segment-effort
  regeneration completed in 114.67 seconds.
- **Recovery and continuation:** a worker replacement completed the same real
  reprocess run across a scheduled-Activity boundary. A 17-entry archive of one
  retained GPX continued as new and awaited every file child, producing one
  duplicate import without adding activities. These prove a small canary, not
  interruption during every effect or multi-gigabyte capacity. Domain work uses
  the blocking pool, two Bike Activity slots, and 30-minute attempt timeouts.
- **Polling removal:** on 2026-10-17 both automatic schedules were paused and
  then deleted. The 828 completed `reconcile_heatmaps` executions were deleted
  with zero failures; no activity/source data was removed. The former 15-second
  heatmap dispatch, one-second SQL inbox relay, 30-second inbox sampler, and
  six-hour Strava scan are removed from source. Native long polls, active-request
  heartbeats/timers, and explicit paged repair workflows remain.
- **Evidence boundary:** the polling audit verified schedule/history deletion,
  not deployment of the new direct request path. The user waived inbox draining.
  Live webhook/upload/edit/redelivery/idle acceptance and dashboard deployment
  remain open in TODO until separately observed.
- **Local validation:** the cutover passed 589 Rust and 39 focused UI tests plus
  formatting/Clippy, shell/workflow, and infrastructure fixtures. The subsequent
  direct-submission/shared-source correction passed the full Rust workspace:
  638 tests, 17 explicitly ignored, zero failures/filtered tests, with formatting
  and Clippy warnings denied. Its extra UI coverage run emitted a pnpm update
  notice and is not a clean cross-project pass. CI and production observations
  do not substitute for that distinction.
- **Earlier provider evidence:** real sync/upsert and natural create/delete
  webhooks completed; a repeated sync retained one unchanged activity and shared
  provider trace context. Real terminal Create/Upsert failures reached
  Alertmanager and ntfy on 2026-10-14. Live backfill/disconnect, interruption
  during executing provider effects, fresh authenticated UI access, and capacity
  qualification remain provider operational follow-ups in TODO.

## References

- [Temporal Activity idempotency](https://docs.temporal.io/activity-definition#idempotency)
- [Deterministic workflow definitions](https://docs.temporal.io/workflow-definition)
- [Child workflows and continuation](https://docs.temporal.io/child-workflows)
- [Workflow/run identity](https://docs.temporal.io/workflow-execution/workflowid-runid)
- [Rust SDK history patching](https://github.com/temporalio/sdk-rust/blob/main/crates/sdk/README.md#patching)
- [Retry policies](https://docs.temporal.io/encyclopedia/retry-policies)
- [Cancellation](https://docs.temporal.io/encyclopedia/workflow/cancellation-and-termination)
- [Activity heartbeats](https://docs.temporal.io/encyclopedia/detecting-activity-failures#activity-heartbeat)
- [Strava webhook contract](https://developers.strava.com/docs/webhooks/)
