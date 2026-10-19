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

- The API submits Strava and Bike commands directly to Temporal and acknowledges
  work only after native acceptance. One `temporal-worker` polls `bike-strava`
  and `bike-tasks`. See the [worker spec](specs/background-work.md).
- Setup containers create Temporal's databases, schema, namespace, and six
  search attributes, then exit successfully.
- Temporal uses the `bike` namespace and seven days of closed-history retention.
  Its databases share PostgreSQL's persistent Docker volume.
- API and Temporal worker share uploads. `BIKE_UPLOADS_DIR` selects the directory.
- Compose services keep Rust caches in Docker volumes; host checks use native
  caches. Restart `temporal-worker` after code changes.
- Set `STRAVA_CLIENT_ID`, `STRAVA_CLIENT_SECRET`,
  `STRAVA_WEBHOOK_VERIFY_TOKEN`, and `STRAVA_SUBSCRIPTION_ID` for real Strava.
  API and Temporal worker must share the token encryption key.
- `BIKE_TEMPORAL_ADDRESS` overrides the worker's server address.
- `COMPOSE_PROJECT_NAME` defaults to `bike`; preserve the existing project's
  database volume when switching checkouts.
- `NEXT_PUBLIC_TEMPORAL_UI_URL` configures admin job links; default is localhost.
- Run lint, formatting, type checks, builds, and unit tests on the host through
  mise. Run Playwright and Chromium browser checks in Docker.
- Run `mise run lint:ui`, `mise run lint:maps`, and `mise run lint:rs` for the
  owning JavaScript/TypeScript and Rust checks.
- Run `mise --cd bike-ui run test` for all UI unit tests and coverage, and
  `mise run test:rs` for `cargo test --workspace --locked`, with automatic
  discovery of workspace tests. `mise run coverage:unit` collects native service
  unit coverage. `mise run checks:docker <task>` is an explicit Linux fallback.
  Activities and child workflows use fixture-backed mocks; no running database,
  HTTP service, or Temporal server is required.
- Browser tests, visual previews, screenshots, and one-off UI checks use the
  pinned browser container. Prefer `mise run e2e [spec]` for the disposable
  connected suite with PostgreSQL, Temporal, application services, and scenario
  resets; see the
  [browser test guide](../bike-ui/tests/e2e/README.md).
- Run `mise exec -- docker compose logs temporal-worker` to inspect correlated logs.
- Run `mise run compose:down` to stop the stack without deleting its volumes.
- See [the spec](specs/background-work.md#strava-protocol-and-workflows) for webhook routes, processors, and job searches.
- [TODO](TODO.md) records production rollout, retention, restart/load verification,
  and fresh-server startup diagnostics. Local fixtures do not prove production delivery.

Image versions and digests are pinned in `mise.toml`.
