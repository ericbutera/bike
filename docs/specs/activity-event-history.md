# Activity change history

- `strava_activity_states` stores the latest accepted state of one Strava
  `(owner_id, object_id)` pair and its Bike activity PK. It supports idempotency
  and stale-event checks; it is not an execution queue or a historical archive.
- `activity_event_history` is the shared, append-only archive of activity
  modifications. Strava changes and manual edits use the same model. Native
  import and reprocess adapters use its typed import-event constructor.
- Model comments explain both roles and timestamp meanings at their definitions.

## Atomic changes and correlation

- Append history in the same database transaction as the activity mutation and
  provider state update. An archive failure rolls back that transaction and is
  retried by the owning Temporal Activity where applicable.
- Archive before/after activity summaries, retained import/source identity, and
  source metadata. Keep recording bytes in shared object storage; do not copy
  route/chart data or heart-rate-zone payloads into every historical entry.
- Preserve the Bike activity PK after deletion. History has no cascading activity
  foreign key, so deleting the current activity does not erase its archive.
- Strava entries retain the event, remote owner/object, connection generation,
  applied state, event status, root workflow ID, request ID, and trace carrier.
- Native import entries retain their command, actor, affected user, request and
  correlation IDs, trace carrier, and native workflow/run reference.
- Manual edits retain the authenticated owner and current trace carrier. Reload
  the current activity under a row lock before building its before snapshot.
- Strava mutation paths take the same activity row lock before capturing their
  previous summary, so concurrent manual edits do not produce stale before snapshots.
- Use a source-qualified event/command key for idempotent appends. A repeated
  command preserves its original historical entry; a legitimate later edit has
  its own identity. Never use activity ID alone as the event key.
- An unchanged Strava payload can still advance accepted provider state; archive
  that transition with equal activity summaries. Superseded events retain their
  outcome in the existing Strava event record and do not mutate current state.

## Timestamps and stale work

| Field                 | Meaning                                                                         |
| --------------------- | ------------------------------------------------------------------------------- |
| State `event_time`    | Last accepted source event time, in epoch seconds.                              |
| State `deleted_at`    | Source timestamp of the current deletion; cleared by a newer upsert.            |
| State `updated_at`    | Time Bike last wrote the state record.                                          |
| State `applied_at`    | Compatibility timestamp for the deployed worker; new writes match `updated_at`. |
| History `occurred_at` | When the source event or manual change occurred.                                |
| History `recorded_at` | When Bike archived the change.                                                  |

- Compare an incoming Strava `event_time` with the stored source timestamp for
  that same owner/object and connection generation. Never compare it with Bike's
  `updated_at` or an activity's start date.
- Webhooks supply Strava's event timestamp. Existing sync/backfill commands use
  their request event time because the activity API provides no equivalent event
  sequence. That remains a best-effort ordering rule; it is not a global clock or
  a strict version number.
- Reject older events. Deletion wins over an upsert at the same timestamp;
  a strictly newer upsert can restore the activity. Reconnection establishes a
  new generation, so an old connection cannot supersede the current one.
- Equal-second updates do not have a provider sequence number. Do not discard
  all events that share a timestamp: Strava documents that one save can emit
  multiple webhooks. Fetch current provider data for updates. Per-activity
  serialization/coalescing and its race checks remain part of the later Strava
  coordination conversion. See [Strava's webhook contract](https://developers.strava.com/docs/webhooks/).

## Rollout and coverage

- The additive migrations backfill known live remote-to-Bike mappings and
  existing deletion/update timestamps. They do not invent historical changes.
- Strava persistence, unchanged-content recognition, deletion, and manual
  detail edits append shared history. Native import adapters must append their
  event with their conditional domain writes before producer cutover.
- Unit tests use DTO fixtures, mocked repository outcomes, and native Temporal
  test environments. Database-engine and deployed crash drills remain separate
  conversion checks.
- This change does not activate the native Bike task worker or replace current
  SQL import producers. The conversion inventory remains the rollout authority.
