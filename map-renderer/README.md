# Bike map renderer

The route preview service for [Bike](../README.md). It renders coordinates into
PNGs through MapLibre and caches results on disk. The UI owns activity
authorization and forwards only permitted coordinates.

The renderer starts with `mise run compose:up` from the root. It is an internal
service; the UI's private image route serves images to the browser.

## Render contract

`POST /render` accepts:

| Field     | Values                                        |
| --------- | --------------------------------------------- |
| `points`  | Two to 100,000 valid latitude/longitude pairs |
| `variant` | `thumbnail` or `full`                         |
| `theme`   | `light` or `dark`                             |
| `dpr`     | `1` or `2`                                    |

The response is `image/png` with `X-Map-Cache: hit` or `miss`.
`GET /healthz` checks readiness. HTTP uses port `3100`.

`bike.maps.v1.MapService/Render` on gRPC port `50051` accepts the same inputs
and returns PNG bytes and a cache-hit flag. Both transports use the same
renderer and cache. `MAP_SERVICE_TOKEN` authenticates HTTP Bearer requests and
gRPC metadata when configured.

## Cache and styles

Cache keys include geometry, dimensions, theme, and render revision. They omit
credentials and activity IDs. The renderer stores no account or activity data.
Reads renew last-use time; startup and hourly cleanup remove images idle beyond
`MAP_IMAGE_CACHE_TTL_SECONDS`, which defaults to seven days.

Style snapshots live in `styles/`. Run `mise run assets:sync` from the root after
editing styles, then bump `renderRevision` in `request.mjs` and the UI's
`ACTIVITY_MAP_STYLE_REVISION` when pixels or attribution change.

## Verification

From the repository root:

```sh
mise run test
mise run format:check
mise run compose:config
```
