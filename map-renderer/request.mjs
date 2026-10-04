import { createHash } from "node:crypto";

export const dimensions = { thumbnail: [288, 192], full: [1000, 300] };
export const renderRevision = 3;

export function parseRenderRequest(payload) {
  if (
    !payload ||
    (payload.theme !== "light" && payload.theme !== "dark") ||
    !Object.hasOwn(dimensions, payload.variant) ||
    (payload.dpr !== 1 && payload.dpr !== 2) ||
    !Array.isArray(payload.points) ||
    payload.points.length < 2 ||
    payload.points.length > 100000 ||
    !payload.points.every(
      (point) =>
        point &&
        Number.isFinite(point.latitude) &&
        Number.isFinite(point.longitude) &&
        Math.abs(point.latitude) <= 90 &&
        Math.abs(point.longitude) <= 180,
    )
  ) {
    return null;
  }

  // Site credentials and activity IDs never enter the cache key. The caller
  // already checked access and sends only the geometry needed to draw a map.
  return {
    theme: payload.theme,
    variant: payload.variant,
    dpr: payload.dpr,
    points: payload.points.map(({ latitude, longitude }) => ({
      latitude,
      longitude,
    })),
  };
}

export function imageKey(request) {
  return createHash("sha256")
    .update(JSON.stringify({ revision: renderRevision, ...request }))
    .digest("hex");
}
