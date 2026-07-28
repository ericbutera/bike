# Bike

Bike is a multi-user activity and training app for cycling data. It imports rides from files, archive exports, and Strava; stores normalized activity data; matches user-owned route segments; and surfaces XC/DH training progress.

Natural-language product specifications live in [docs/specifications](docs/specifications). Treat those docs as the source of truth for intended behavior, and update the affected spec alongside code changes that alter user-visible functionality, processing semantics, or training recommendations.

## Project Shape

- `api`: Rust Axum API, auth wiring, import endpoints, activity/segment/training/admin APIs.
- `worker`: background processors for imports, sync, analytics, segment regeneration, and backfills.
- `migration`: SeaORM migrations for Bike-owned tables.
- `ui-next`: Next.js frontend.
- `garmin-iq`: Garmin Connect IQ companion app and watch sync docs.
- `docs/specifications`: product behavior contracts.

## Local Development

```sh
docker compose up
```

The default compose setup uses Postgres on `localhost:5432`, API on `localhost:3000`, and UI on `localhost:3001`.

## Kaleido Dependency

Kaleido updates are explicit in this repo:

```sh
task kaleido:version
VERSION=0.7.0 task kaleido:upgrade
```

The upgrade task updates `ui-next/package.json` and `ui-next/pnpm-lock.yaml`, then runs `pnpm typecheck`.
