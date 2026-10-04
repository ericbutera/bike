# Bike maps

Interactive activity, segment, and race views use MapLibre with normalized
route geometry. Activity cards use private PNG previews from the separate
[map renderer](../map-renderer/README.md).

The UI verifies activity access before forwarding route coordinates to the
renderer. The renderer stores no activity or account data and stays off public
ingress. Geometry, dimensions, theme, and render revision form the cache key;
identical output can share a file without including credentials or activity IDs.

Reads renew the file's last-use time. Startup and hourly cleanup remove images
idle beyond `MAP_IMAGE_CACHE_TTL_SECONDS`, which defaults to seven days.
Expired entries miss on read, even if the sweep is delayed. Observe bytes,
file count, render latency, and hit rate before changing cache or image formats.

Personal heatmaps use Rust activity projections and private tile endpoints,
independently of the route PNG renderer. See the
[heatmap specification](specs/heatmaps.md) for preparation, filters, and the
feature flag, and [architecture.md](architecture.md) for service boundaries.
