# Account Strava sync history

- Show dates, plain-language sync results, and activities actually updated or
  removed. Keep queued, running, completed, failed, and cancelled syncs visible
  even when no activity changed, including retained legacy sync events/counts.
- Present the latest retained lifecycle state once per native workflow. Keep
  confirmed activity changes; ignore processing steps, unchanged activities,
  exclusions, stale work, and already absent deletions.
- Filter meaningful history by authenticated account owner and Strava provider
  before applying the 25-event limit. Internal events must not crowd syncs out.
- Refresh history after Sync now and when the connection's sync status or
  completion time changes, including the transition out of active polling.
- Link available Bike activities to their details and use their names when
  available. Otherwise retain their identifiers without inventing names.
  Removed activities have no link to a deleted Bike page.
- Hide raw messages, payloads, severity, stage names, athlete IDs, workflow IDs,
  import IDs, user IDs, and connection IDs. Admin events keep their diagnostics.
- Show an empty state only when no recent meaningful history is available;
  a bounded feed cannot establish that an entire sync changed nothing.
- Validate the account controller's queued/previous/empty history, newest 25
  meaningful events, account/provider isolation, and response mapping with
  SQLite fixtures. Validate native/legacy states, final status refresh, and
  confirmed activity changes with UI unit tests. Do not call the live provider.
