# Retired Strava gateway

- Bike Rust now owns the integration defined by
  [the Strava specification](strava-integration.md).
- The API receives vendor webhooks and existing workers execute the DAGs.
- The separate gateway, provider worker, signed delivery API, gRPC client and
  delivery outbox consumer are removed.
- Existing historical database migrations and completed records remain
  append-only history; they do not run integration work.
- Previously registered callback URLs remain aliases to native Rust handlers
  while the vendor registration is preserved.
- [The cutover plan](../plans/strava-consolidation.md) switches webhook ownership
  and retires gateway runtime without copying or replaying its work.
