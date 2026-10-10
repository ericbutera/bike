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

Run from the repository root:

```sh
mise run checks:docker ci:renderer
mise run checks:docker ci:rust
mise run checks:docker ci:ui:unit
mise run renderer:parity
mise run renderer:smoke
mise run compose:config
```

The parity task compares decoded pixels against the pre-rewrite Node renderer
using frozen basemaps across both themes, sizes, and DPRs. The smoke task uses
current basemap providers. Native Rust HTTP tests cover ownership, cache
renewal/expiry, request coalescing, and ETag/304 behavior with a fake snapshot
gRPC worker. Transport tests verify client span metadata and remote parenting,
including an actual OTLP HTTP export. The
[acceptance criteria](../docs/specs/maps.md#acceptance-and-evidence) define the
required outcomes; [PR #17](https://github.com/ericbutera/bike/pull/17) records
review evidence. Deployment remains subject to coordinated release verification.

## Local rewrite measurements

The Linux/arm64 image measured approximately 786 MB versus 2.76 GB for the former
Node/Playwright image; the build downloads Chromium's headless shell only.
An initial Go compile took 10.8 seconds and an incremental compile 0.8 seconds.
A comparable headless-shell checkpoint measured peak cgroup memory of 404.3 MB
for Go and 398.5 MB for Node. These are historical local observations, not
cross-language benchmarks or a claim of reduced Chromium memory use. The Go
services reuse repository tooling and BuildKit module/build caches.
