# Local development

```sh
mise run compose:up
```

| Service                     | Address                                         |
| --------------------------- | ----------------------------------------------- |
| Bike                        | http://localhost:3001                           |
| Bike API                    | http://localhost:3000/api                       |
| Temporal UI                 | http://localhost:8080/namespaces/bike/workflows |
| Temporal gRPC from the host | localhost:7233                                  |
| Temporal gRPC from Compose  | temporal:7233                                   |

- Temporal uses the `bike` namespace with seven days of closed-workflow retention.
- Its databases share the existing PostgreSQL Docker volume. Restarting or
  stopping Compose preserves Bike data and Temporal history.
- The setup containers create databases, apply schemas, and create the namespace.
  They exit after completing successfully.
- Override `BIKE_TEMPORAL_PORT` or `BIKE_TEMPORAL_UI_PORT` for occupied host ports.
- `COMPOSE_PROJECT_NAME` selects the Compose project; its default is `bike`.
- `temporal-worker` polls `bike-strava` for the upsert/delete pilot. Public
  webhooks and account actions still use the current PostgreSQL job queue.
- `BIKE_TEMPORAL_ADDRESS` configures the worker's server address.
- `BIKE_UPLOADS_DIR` configures shared object storage; it defaults to
  `./bike-rs/uploads`. API and both workers must use the same files.
- Rust dependency and build caches live in dedicated Docker volumes.
- Restart `temporal-worker` after Rust changes; it builds the current source at startup.
- Production workers will connect to the shared service using a configured
  address and the same `bike` namespace.

```sh
mise run temporal:health
mise run test:temporal:docker
mise exec -- docker compose restart temporal-worker
mise exec -- docker compose logs temporal temporal-ui
mise run compose:down
```

Image versions and digests are pinned in `mise.toml`.

Local setup verification on 2026-10-13:

- Service health, namespace creation, repeated setup, UI API access, and workflow
  history after server recreation passed. The workflow was a synthetic
  infrastructure check, then terminated.
- Fresh-database startup reached healthy state, but logged `Frontend is not
healthy yet` and `shard status unknown` during initialization. The clean-start
  check remains open. Temporal tracks the first diagnostic in
  [CLI issue 633](https://github.com/temporalio/cli/issues/633); its
  [shard implementation](https://github.com/temporalio/temporal/blob/v1.32.1/service/history/shard/context_impl.go)
  documents the second as possible during initialization or database disconnection.
- UI verification used HTTP and namespace API checks. Interactive browser
  verification was unavailable in this session.
- Idle Docker memory: Temporal 87 MiB; UI 27 MiB. Temporal databases used about
  19 MiB combined. These measurements exclude shared PostgreSQL memory and WAL;
  workload sizing remains on the checklist.
