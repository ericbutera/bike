# Import history and inspection

- Admin import history and import details show domain metadata: retained source,
  filename, size, receipt time, status/error, and linked Bike activity.
- Load on demand. Do not poll or refetch on window focus or network reconnect.
  Provide Refresh; disable it during a fetch or when a required owner filter is
  invalid. An unselected import must not issue a request.
- Details use `GET /admin/activity-imports/{id}` with administrator authorization.
  Return no storage path, parser checkpoints, attempt payloads, or execution graph.
- Temporal UI owns workflow execution, children, retries, and cancellation, as
  specified in [background work](background-work.md#ownership-and-runtime).
  Retire the custom import trace, processing graph, and stage replay endpoints.
- Existing retained import/attempt data and historical migrations remain intact.
  Keep SQL execution behavior for unconverted producers while displaying only
  domain metadata. Native single-activity reprocessing remains an admin task.
- Verify details and missing imports with PostgreSQL mocks/shared fixtures.
  Verify on-demand loading, manual refresh, failure details, and the activity link
  with mocked React Query responses; do not add SQLite compatibility machinery.
