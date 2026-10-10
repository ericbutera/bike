# Bike UI

The Next.js, React, TypeScript, and React Query frontend for
[Bike](../README.md). Rider and admin workflows use the same application and the
Rust backend's generated HTTP client.

## Run locally

Start the application from the root with `mise run compose:up`, then open
[localhost:3001](http://localhost:3001). Compose provides the backend, database,
worker, and renderer and enables frontend hot reload.

To run the frontend directly, start the backend stack first, configure `API_URL`
and `INTERNAL_API_URL` for the running API, then run from this directory:

```sh
mise install
mise run deps
mise run dev
```

`API_URL` is the browser-facing API address; `INTERNAL_API_URL` is used by
server-side requests. The [development guide](../docs/development.md) describes
local defaults and port overrides.

## Checks and generated client

```sh
mise run typecheck
mise run test
mise run build
mise run format:check
mise run openapi:check
```

Rust's Utoipa definitions generate the canonical contract directly in
`../contracts/openapi/`. After updating the contract,
run `mise run generate:typescript` here to regenerate
`lib/openapi/react-query/api.d.ts`.

## Maps

Activity cards proxy private PNG requests to Rust, which owns activity access,
geometry, and caching. The UI does not call the snapshot worker or hold its
credential. Detail pages use interactive MapLibre maps. See the canonical
[maps specification](../docs/specs/maps.md) and
[heatmap specification](../docs/specs/heatmaps.md) for behavior.

The renderer owns light and Fiord snapshots in `../map-renderer/styles/`;
`public/map-styles/` contains build-context copies. Run `mise run assets:sync`
from the root after changing the owning styles. When output changes, update
`ACTIVITY_MAP_STYLE_REVISION` in the UI and `RENDER_REVISION` in
[`types.rs`](../bike-rs/bike-core/src/activity_maps/types.rs) together.

## Deployment

Woodpecker publishes an immutable `bike-ui` image for owned changes. Pulumi pins
that image independently of backend images. See the
[architecture guide](../docs/architecture.md) for release boundaries.
