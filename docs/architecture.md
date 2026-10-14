# Bike architecture

Bike has one Rust application backend and one Next.js frontend. PostgreSQL holds
normalized activity records, derived analytics, accounts, and durable jobs.
Original activity files remain available for download and replay.

## Service boundaries

```mermaid
flowchart LR
    Browser[Browser] --> UI[Next.js UI]
    Browser --> API[Rust API]
    UI --> API
    API --> DB[(Bike PostgreSQL)]
    Worker[Rust worker] --> DB
    API --> Files[Activity files]
    Worker --> Files
    API --> Maps[Go Chromium snapshot worker]
    API --> MapCache[Map PNG cache]
    Strava[Strava] -->|OAuth and webhooks| API
    Worker -->|Generated SDK| Strava
    Worker --> ProviderObjects[Retained raw and transformed objects]
```

| Component    | Owns                                                                                                    |
| ------------ | ------------------------------------------------------------------------------------------------------- |
| Rust API     | HTTP adaptation, application services, activity access, authentication, admin operations, map PNG cache |
| Rust core    | Domain models, database queries, import and segment behavior, analytics, platform modules               |
| Rust worker  | Activity/archive processing, derived-data rebuilds, queued maintenance                                  |
| Next.js UI   | Rider/admin presentation, React Query state, generated API client, private map proxy                    |
| Map renderer | Go gRPC snapshots, Chromium lifecycle, serialized MapLibre rendering, loopback browser assets           |

Controllers adapt HTTP requests and responses. Services compose workflows and
policy. Queries live on their owning entity/model modules. Axum and SeaORM
provide routing and persistence; Bike owns its platform modules inside
`bike-rs/bike-core`.

Use SeaORM entities and ActiveModels for ordinary reads, writes, joins,
aggregates, pagination, locks, and conflicts. Use SeaQuery when an expression
needs the database clock or a database function. Keep SQL escape hatches small,
bind dynamic values, and explain the unsupported primitive or atomic operation
beside each one. The retained application SQL covers heatmap writable CTEs and
the PostgreSQL box-overlap expression used by the GiST index. Migration and
fixture DDL remain schema records. The
[shared engineering quality skill](../.agents/skills/bike-engineering-quality/SKILL.md)
defines the review and verification requirements.

## Activity data flow

File uploads and archive imports retain their source data and enter the durable
processing workflow. Parsing normalizes FIT, TCX, or GPX into an activity record.
Processing then matches segments and rebuilds activity and training analytics.
Derived data can be regenerated from retained sources when parsers or rules
change. See the [ingestion specification](specs/activity-ingestion.md).

Strava callbacks are acknowledged after the Rust API commits the original source,
workflow and existing Bike worker job. Named action DAGs run on the Rust workers;
the generated vendor SDK, quota ledger, saved stage outputs and activity correlation
are defined by [the Strava specification](specs/strava-integration.md).
Raw responses and each transform remain immutable on the existing uploads volume.
Retries, quota pauses and failures appear in worker administration and integration
history. [The cutover plan](plans/strava-consolidation.md) switches webhook ownership
before provider ownership changes.

## Maps and privacy

Interactive activity and segment views use MapLibre. Activity-card PNGs pass
through the UI's private image route to the Rust API, which owns access and PNG
caching. A miss calls the Go Chromium worker over gRPC. The
[maps specification](specs/maps.md) defines the HTTP and gRPC contracts,
privacy, cache lifecycle, and required OpenTelemetry visibility.

Personal heatmaps use Rust-owned activity projections and private raster tile
endpoints. Worker preparation and feature flags are described in the
[heatmap specification](specs/heatmaps.md).

## Contracts and ownership

- Rust generates the canonical HTTP contract directly in `contracts/openapi/`.
  The frontend and test harnesses consume its YAML and JSON representations.
- `bike-ui/lib/openapi/react-query/api.ts` is generated from that contract,
  including runtime enum values for frontend controls.
- Versioned protobuf definitions under `proto/` describe the internal renderer interface.
- `mise run assets:sync` copies canonical assets into required build contexts;
  `mise run contracts:check` compares those copies with their owners.
- `mise run rust:check` verifies the HTTP contract against fresh Rust output.
- Schema migrations are append-only records. Preserve their order and history.

## Deployment

Root Docker Compose supplies the local application environment. Production definitions and release
promotion are maintained separately.

Woodpecker jobs run checks for the owning component, publish images with
immutable commit tags, and call the Pulumi release helper. Backend promotion
runs migrations before API/worker rollout; the UI and renderer have
their own image boundaries. Existing namespace and image names are deployment
identities and do not depend on the repository's display name.

OpenTelemetry supports tracing; internal metrics, dashboards, alerts, and
protected failure captures support diagnosis. See the
[Strava workflow specification](specs/strava-integration.md) and
[admin recovery procedures](specs/admin-operations.md#failed-import-recovery) for recovery procedures.
