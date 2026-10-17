# Local development

```sh
mise run compose:up
```

| Service                 | Address                                         |
| ----------------------- | ----------------------------------------------- |
| Bike                    | http://localhost:3001                           |
| Bike API                | http://localhost:3000/api                       |
| Temporal history        | http://localhost:8080/namespaces/bike/workflows |
| Temporal gRPC           | localhost:7233                                  |
| Native SDK metrics      | http://localhost:9092/metrics                   |
| Strava provider metrics | http://localhost:9093/metrics                   |

- The API accepts durable Strava events and converted Bike commands. One
  `temporal-worker` polls `bike-strava` and `bike-tasks`; the SQL `worker` retains
  unconverted jobs. The backlog records which conversions are ready for rollout.
- Setup containers create Temporal's databases, schema, namespace, and six
  search attributes, then exit successfully.
- Temporal uses the `bike` namespace and seven days of closed-history retention.
  Its databases share PostgreSQL's persistent Docker volume.
- API and both workers share uploads. `BIKE_UPLOADS_DIR` selects the directory.
- Rust caches live in Docker volumes. Restart `temporal-worker` after code changes.
- Set `STRAVA_CLIENT_ID`, `STRAVA_CLIENT_SECRET`,
  `STRAVA_WEBHOOK_VERIFY_TOKEN`, and `STRAVA_SUBSCRIPTION_ID` for real Strava.
  API and Temporal worker must share the token encryption key.
- `BIKE_TEMPORAL_ADDRESS` overrides the worker's server address.
- `COMPOSE_PROJECT_NAME` defaults to `bike`; preserve the existing project's
  database volume when switching checkouts.
- `NEXT_PUBLIC_TEMPORAL_UI_URL` configures admin job links; default is localhost.
- Run `mise run test:rs` for `cargo test --workspace --locked`, with automatic
  discovery of workspace tests. `mise run checks:docker test:rs` uses the same task.
  Activities and child workflows use fixture-backed mocks; no running database,
  HTTP service, or Temporal server is required.
- Run `mise exec -- docker compose logs temporal-worker` to inspect correlated logs.
- Run `mise run compose:down` to stop the stack without deleting its volumes.
- See [the spec](specs/background-work.md#strava-protocol-and-workflows) for webhook routes, processors, and job searches.
- [TODO](TODO.md) records production rollout, retention, restart/load verification,
  and fresh-server startup diagnostics. Local fixtures do not prove production delivery.

Image versions and digests are pinned in `mise.toml`.
