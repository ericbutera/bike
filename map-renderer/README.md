# Bike snapshot worker

Go and chromedp run the Chromium snapshot worker for Bike. The
[maps specification](../docs/specs/maps.md) is the source of truth for rendering,
authorization, cache lifecycle, gRPC, and tracing. This README owns runtime
commands, style synchronization, and implementation measurements.

Start the local stack with `mise run compose:up`. The worker is internal and
has no public ingress. Its runtime contains the Go executable, Chromium headless
shell, fonts, and browser assets. Node/npm install locked browser assets during
the build; the serving process is Go.

## Runtime configuration

The API uses `MAP_RENDERER_GRPC_ADDRESS=http://bike-maps:50051`,
`MAP_SERVICE_TOKEN`, `MAP_IMAGE_CACHE_DIR`, and `MAP_IMAGE_CACHE_TTL_SECONDS`.
The worker uses the same token, gRPC 50051, metrics 9090, and
`MAP_ASSETS_ADDRESS=127.0.0.1:3100` for Chromium assets.
See the [snapshot contract](../docs/specs/maps.md#internal-grpc-snapshot-contract)
for validation, limits, health, cancellation, and shutdown behavior.

## Styles

Style snapshots live in `styles/`. Run `mise run assets:sync` after editing
styles. When pixels or attribution change, update Rust's `RENDER_REVISION`
in [types.rs](../bike-rs/bike-core/src/activity_maps/types.rs) and the UI's
`ACTIVITY_MAP_STYLE_REVISION` together.

## Tracing

See the [network visibility contract](../docs/specs/maps.md#network-visibility)
for required propagation, attributes, and visibility limits, and
[local Jaeger setup](../docs/development.md#inspect-traces) for collector commands.

## Verification

Run from the repository root inside the
[checks container](../docs/development.md#checks-container):

```sh
mise run ci:renderer
mise run rust:check
mise run ci:ui:unit
mise run e2e
mise run compose:config
```

Browser tests exercise the current UI, API, and snapshot worker with frozen
basemap fixtures. Check current providers separately during release verification.
Native Rust HTTP tests cover ownership, cache
renewal/expiry, request coalescing, and ETag/304 behavior with a fake snapshot
gRPC worker. Transport tests verify client span metadata and remote parenting,
including an actual OTLP HTTP export. The
[acceptance criteria](../docs/specs/maps.md#acceptance-and-evidence) define the
required outcomes; [PR #17](https://github.com/ericbutera/bike/pull/17) records
review evidence. Deployment remains subject to coordinated release verification.
