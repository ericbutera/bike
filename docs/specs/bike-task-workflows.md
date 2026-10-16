# Bike task workflow proof of concept

- Keep the task names in [the conversion plan](../worker-conversion-plan.md#task-inventory-and-conversion-order).
  Renaming any task requires the user's approval and an explanation. Rust type
  names may follow Rust casing; registered Temporal names match the plan exactly.
- This proof of concept establishes typed workflow composition and handoffs.
  Production domain adapters, write fences, producer cutover, worker deployment,
  and retained-history recovery remain conversion gates.

## Workflows and Activities

| Task | Temporal Workflow name                   | Responsibility                                                                         |
| ---- | ---------------------------------------- | -------------------------------------------------------------------------------------- |
| 1    | `reprocess_activity_import`              | Replay retained input into an existing activity, then hand off derived requests.       |
| 2    | `rebuild_fitness_freshness`              | Debounce and refresh one user's fitness output.                                        |
| 3    | `rebuild_segment_analytics`              | Debounce and refresh one segment's analytics output.                                   |
| 4    | `prepare_heatmap`                        | Debounce and prepare the current generation of one activity.                           |
| 5    | `regenerate_activity_segments`           | Rematch an activity and hand off affected segment analytics.                           |
| 6    | `regenerate_segment_efforts`             | Page affected activities and call #5 for one segment generation.                       |
| 7    | `backfill_user_xc_training`              | Page activity training analysis, hand off fitness changes, then finalize the backfill. |
| 8    | `process_activity_import`                | Import retained input as a new activity, then hand off derived requests.               |
| 9    | `reprocess_user_activity_imports`        | Page retained user imports and call #1.                                                |
| 10   | `reprocess_archive_fit_activity_imports` | Select only archive FIT imports and reuse #1 through the same page mechanism.          |
| 11   | `regenerate_user_segments`               | Page a user's cycling activities and reuse #5.                                         |
| 12   | `activity_archive_import`                | Retain archive and manifest references, then page entries through #8.                  |

- Task 0, `email_notification`, remains a retirement item with no replacement
  workflow. All production conversion statuses remain separate from these
  proof-of-concept definitions.
- `activity_import` is an explicit reusable **Activity**, not another task name.
  Both import workflows invoke it. Its typed target distinguishes new activity
  creation from reprocessing an existing activity at an expected revision.
- The Activity owns parsing and conditional domain persistence. It receives a
  retry-stable command ID, owner/import identity, and a shared-storage reference.
  It does not schedule derived work or submit SQL tasks.
- A successful Activity returns the committed activity/import identity and typed
  fitness, affected-segment, and optional heatmap requests. Each output carries
  its own committed revision/generation; an activity revision must not be reused
  as a user's fitness or a segment's revision.
- A retry after a committed write and lost completion returns that command's
  original outcome. Concrete adapters must enforce this and reject stale writes.
- Each workflow declares its steps explicitly. Any later workflow may call the
  same Activity or request these derived workflows in its own order.

## Review from the workflows

- Start at [the workflow index](../../bike-rs/bike-temporal/src/workflows.rs).
  Each task has its own file and registered name. Review its `run` method first.
- Named [application steps](../../bike-rs/bike-temporal/src/workflows/steps.rs)
  compose typed effects and derived-output handoffs. Both imports use one
  `refresh_import_outputs` sequence, whose fitness/segment/heatmap steps remain
  individually reusable by other workflows.
- [Batch paging](../../bike-rs/bike-temporal/src/workflows/batches.rs) and
  [typed child calls](../../bike-rs/bike-temporal/src/workflows/children.rs)
  own shared SDK options, stable child IDs, cancellation, result awaiting, counts,
  and continuation. Workflow call sites name the page Activity and child Workflow;
  they do not repeat SDK builders or implement private task queues.
- [Activity adapters](../../bike-rs/bike-temporal/src/activities.rs) are separated
  into imports, derived data, segments, training, and archives. Each adapter calls
  a typed domain operation. Concrete implementations must enforce idempotency and
  write fences; they must not call legacy queue/lock/barrier orchestration.

```mermaid
flowchart LR
    Import[process_activity_import] --> Shared[Activity: activity_import]
    Reprocess[reprocess_activity_import] --> Shared
    Shared --> Committed[Committed result and output revisions]
    Committed --> Fitness[rebuild_fitness_freshness]
    Committed --> Segments[rebuild_segment_analytics]
    Committed --> Heatmap[prepare_heatmap]
    Other[Other task workflows] --> Fitness
    Other --> Segments
    Other --> Heatmap
    ReprocessUser[reprocess_user_activity_imports] --> Reprocess
    ReprocessFits[reprocess_archive_fit_activity_imports] --> Reprocess
    Archive[activity_archive_import] --> Import
    UserSegments[regenerate_user_segments] --> Match[regenerate_activity_segments]
    SegmentEfforts[regenerate_segment_efforts] --> Match
    Match --> Segments
    Training[backfill_user_xc_training] --> Fitness
```

## Shared bounded batches

- Prepare a retained immutable manifest once per command. It fixes eligible
  inputs, stable entry command IDs, resource revisions, selection, and ordering.
  Page readers return references and small typed commands, never archive bytes
  or route payloads in workflow history.
- Bind each manifest to the parent command and selection. A continued archive-FIT
  workflow must reject a general user-import manifest. User, training, segment,
  and archive selections cannot substitute for one another.
- Read at most 16 entries per page. The reader must return exactly the expected
  number of entries through the manifest boundary; truncated/oversized pages
  fail instead of silently skipping inputs or reporting success.
- Validate the whole page before starting work: entry command IDs belong to the
  parent, IDs within the page are unique, and entries match the manifest's scope.
  Preparation must enforce global entry uniqueness across the retained manifest.
- Reprocessing and archive parents await existing import child workflows.
  User and global-segment regeneration parents await the same per-activity
  segment workflow, with `All` or one specific segment generation respectively.
- A global segment manifest may include activities from multiple users, including
  previous effort holders that no longer match. A user segment manifest includes
  only that user's selected cycling activities. The per-activity domain operation
  commits matching and per-activity analytics together, then hands off global
  segment analytics through its existing debounce workflow.
- Training backfill uses the same page loop with per-activity training Activities
  and optional fitness handoffs. Only after every page settles may it record
  successful goal/backfill completion. Errors and cancellation prevent that
  completion effect; native workflow status remains the execution authority.
- Await one child/Activity at a time in this proof of concept. Advance the cursor
  and counts only after the page settles. Child-start and execution failures
  fail the parent; starting a duplicate child is not proof of successful work.
- Shared execution helpers check cancellation before dispatch and after a
  dependency settles. A late successful Activity or child must not turn a canceled
  parent into success or cause another derived handoff. Use native cancellation
  with completion acknowledgement; preserve that outcome at the workflow boundary.
- Continue-As-New only after a settled page, after 16 pages (at most 256 entries
  per run), or when the SDK suggests continuation. Carry the retained manifest
  and settled count; no live children cross the boundary.
- Archive manifest preparation has a one-hour Activity execution timeout and a
  30-second heartbeat timeout. Its domain implementation must stream through
  shared storage and resume safely from older heartbeat details. References
  remain retained across worker loss and continuation; do not delete source bytes
  at parent completion. The shared storage lifetime/reference policy owns cleanup.
- Batch counts describe settled source/matching/training operations and accepted
  handoffs. Final derived-readiness observation/barriers, persisted attempt fences,
  and real archive crash drills remain production conversion gates.

## Handoff and completion

- Import workflows finish after derived requests are durably accepted. They do
  not await derived publication. Source completion and derived readiness are
  separate; a successful import must not claim that all user data is refreshed.
- Use one stable Workflow ID per user for fitness, per segment for analytics,
  and per activity for heatmap. Shared segments must converge even when changes
  come from different users or parent workflows.
- Producers use native Signal-With-Start with `UseExisting`. Repeated parent
  calls reach the same named output workflow, including after the parent closes.
- Signal-With-Start is a client RPC in the pinned Rust SDK. The small typed
  scheduling Activities perform that atomic RPC and return after acceptance.
  They do not execute the derived operation, wait for its publication, use a SQL
  bridge, or create a private queue.
- Retrying a partially completed segment fanout resubmits the same output
  revisions. The receiving workflows deduplicate those requests.

## Debounce and priority

- Priority is Temporal's native Workflow/Activity priority option. New activity
  refreshes have high priority (1); deletion/background refreshes have lower
  priority (5) and may wait longer. Immediate deletion/privacy restrictions stay
  separate from expensive refreshing.
- New archive/import entries request high-priority refreshes even if their
  ingestion runs at background priority. Repair/reprocessing retains its selected
  refresh priority; stored output revisions do not encode urgency.
- Debounce is Temporal's [signal and resettable timer pattern](https://docs.temporal.io/design-patterns/updatable-timer),
  not a server debounce setting. Each named derived workflow owns its pending
  typed request, durable timer, and published revision. There is no generic
  task-type coordinator or independent running-task state machine in Bike core.
- Initial configurable windows remain 2s quiet / 10s cap for new activity and
  60s quiet / 5m cap for deletion/background. Tune these after the working proof
  of concept; backlog aging and performance policy are follow-up work.
- Merge revisions and urgency; fitness also keeps the earliest dirty date.
  Requests for the same revision do not extend the deadline. High-priority
  arrivals advance a pending low-priority flush; low-priority requests do not
  postpone a high-priority flush.
- Five quick requests for one output produce one rebuild. Requests arriving
  during a rebuild merge into one follow-up. Published revisions do no more work.
- Temporal records signals, timers, Activities, retries, and cancellation.
  Continue-As-New carries pending requests, deadlines, and publication state
  only after active work settles. A superseded result waits for newer input.
- Give each processing Activity its current batch priority/fairness and disable
  eager dispatch so native matching applies. Priority does not preempt running
  Activities. Broader starvation/aging policy and deployed measurements remain
  explicit follow-ups; do not implement a parallel priority scheduler.

## Validation and remaining work

- Unit proof uses the pinned SDK workflow runner, shared fixtures, and fake
  domain/RPC adapters. Assert actual Activity and timer commands, the exact task
  names, handoff ordering, duplicate bursts, priority changes, replay, continuation,
  cancellation, and requests arriving during execution.
- Cover each named workflow's happy path and shared-batch recovery: more than one
  page, restart during a child, continuation with the same manifest, failed child
  start/execution, empty snapshots, invalid pages, selection substitution, and
  global-segment work across users. Large-manifest fixtures are synthetic;
  they do not establish real archive streaming, disk usage, or crash durability.
- Real database import effects, persisted idempotent command outcomes, generation
  fences, derived-readiness observation/wait contracts, worker registration and
  deployment, producer migration, and live crash drills remain required before
  converting a production task. Unit proof does not establish those guarantees.
- Shared storage/TTL behavior remains in the shared storage service. Complete
  the [Strava follow-up](../TODO.md#worker-conversion) after the Bike conversions.
