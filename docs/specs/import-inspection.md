# Import history and trace inspection

- Admin import history and import traces load on demand. Do not poll, or refetch
  on window focus or network reconnect. Keep the loaded view stable.
- Apply the same query policy to import-ID traces, activity-linked admin traces,
  and import history lists, including activity pages that reuse these queries.
- Provide Refresh in list/detail views. Disable it while fetching and when a
  required owner filter is invalid. Unselected traces do not make a request.
- Explicit replay/recovery actions may invalidate the affected queries.
- Verify no additional requests during elapsed time, focus, or reconnect, and
  verify an explicit refresh fetches fresh data, using fake timers and responses
  at the React Query boundary. Cover the visible Refresh actions with unit tests.
