import http from "k6/http";
import { check, fail } from "k6";

export const options = {
  scenarios: {
    smoke: {
      executor: "shared-iterations",
      vus: 1,
      iterations: 1,
      maxDuration: "90s",
    },
  },
  thresholds: {
    checks: ["rate==1"],
    "http_req_failed{kind:journey}": ["rate==0"],
  },
};

const api = (__ENV.BIKE_API_URL || "").replace(/\/$/, "");
const ui = (__ENV.BIKE_UI_URL || "").replace(/\/$/, "");
const publicUrl = (__ENV.BIKE_PUBLIC_URL || "").replace(/\/$/, "");
const key = __ENV.BIKE_SYNTHETIC_KEY;
const headers = { "X-Bike-Synthetic-Key": key };
const publicDenial = http.expectedStatuses(401, 403);
const internalDenial = http.expectedStatuses(403);
const journey = {
  headers,
  redirects: 0,
  tags: { kind: "journey" },
  timeout: "30s",
};

function read(path) {
  const response = http.get(`${api}${path}`, journey);
  if (!check(response, { [`GET ${path} succeeds`]: (r) => r.status === 200 })) {
    fail(`Production read failed: ${path}`);
  }
  try {
    return response.json();
  } catch {
    check(false, { [`GET ${path} returns JSON`]: (valid) => valid });
    fail(`Production read returned invalid JSON: ${path}`);
  }
}

function geometry(points) {
  return (
    Array.isArray(points) &&
    points.length >= 2 &&
    points.every(
      (point) =>
        Number.isFinite(point.latitude) &&
        Math.abs(point.latitude) <= 90 &&
        Number.isFinite(point.longitude) &&
        Math.abs(point.longitude) <= 180,
    )
  );
}

export function setup() {
  if (!api || !ui || !publicUrl || !key)
    fail(
      "Configure internal API/UI origins, public origin, and the synthetic credential",
    );
  const manifest = read("/synthetics/scenario");
  const positiveId = (id) => Number.isSafeInteger(id) && id > 0;
  if (
    !check(manifest, {
      "scenario is platform-smoke/v1": (m) => m.name === "platform-smoke/v1",
      "activity and segment IDs discovered": (m) =>
        positiveId(m.activity_id) && positiveId(m.segment_id),
      "two distinct effort IDs discovered": (m) =>
        Array.isArray(m.race_effort_ids) &&
        m.race_effort_ids.length === 2 &&
        m.race_effort_ids.every(positiveId) &&
        new Set(m.race_effort_ids).size === 2,
    })
  )
    fail("Invalid production scenario manifest");
  return manifest;
}

export default function (scenario) {
  const health = read("/health");
  check(health, { "API is healthy": (h) => h.status === "healthy" });
  const user = read("/auth/current");
  check(user, {
    "synthetic identity has no admin or interactive login": (u) =>
      u.is_admin === false && u.disabled === true,
  });
  const activities = read("/activities?page=1&per_page=10");
  check(activities, {
    "activity list contains fixture ride": (a) =>
      Array.isArray(a.data) &&
      a.data.some((ride) => ride.id === scenario.activity_id),
  });
  const activity = read(`/activities/${scenario.activity_id}`);
  check(activity, {
    "activity has route and positive metrics": (a) =>
      a.id === scenario.activity_id &&
      a.distance_meters > 0 &&
      a.moving_time_seconds > 0 &&
      geometry(a.route_points),
  });
  const segments = read("/segments");
  check(segments, {
    "segment list contains fixture segment": (s) =>
      Array.isArray(s) &&
      s.some((segment) => segment.id === scenario.segment_id),
  });
  const segment = read(`/segments/${scenario.segment_id}`);
  check(segment, {
    "segment has two efforts and positive metrics": (s) =>
      s.id === scenario.segment_id &&
      s.effort_count === 2 &&
      s.distance_meters > 0,
  });
  const comparison = read(`/segments/${scenario.segment_id}/comparison`);
  check(comparison, {
    "race has geometry and both fixture efforts": (c) =>
      c.segment_id === scenario.segment_id &&
      geometry(c.route_points) &&
      Array.isArray(c.efforts) &&
      scenario.race_effort_ids.every((id) =>
        c.efforts.some(
          (effort) =>
            effort.id === id &&
            effort.duration_seconds > 0 &&
            geometry(effort.route_points),
        ),
      ),
  });

  const imagePath = `/activity-map-images/thumbnail/1?activityId=${scenario.activity_id}&theme=light&dpr=1`;
  const image = http.get(`${ui}${imagePath}`, {
    ...journey,
    responseType: "binary",
  });
  check(image, {
    "UI proxy renders a real PNG": (r) =>
      r.status === 200 &&
      r.headers["Content-Type"].includes("image/png") &&
      new Uint8Array(r.body).length > 24 &&
      [137, 80, 78, 71, 13, 10, 26, 10].every(
        (byte, i) => new Uint8Array(r.body)[i] === byte,
      ),
  });

  for (const path of [
    "/api/auth/current",
    "/api/synthetics/scenario",
    imagePath,
    `/activity-previews?activityId=${scenario.activity_id}`,
  ]) {
    const response = http.get(`${publicUrl}${path}`, {
      headers,
      redirects: 0,
      tags: { kind: "public-boundary" },
      responseCallback: publicDenial,
    });
    check(response, {
      [`synthetic credential denied publicly: ${path.split("?")[0]}`]: (r) =>
        r.status === 401 || r.status === 403,
    });
  }
  for (const [method, path] of [
    ["PUT", "/preferences"],
    ["GET", "/auth/logout"],
    ["GET", "/admin/users"],
  ]) {
    const response = http.request(method, `${api}${path}`, null, {
      headers,
      redirects: 0,
      tags: { kind: "authorization-boundary" },
      responseCallback: internalDenial,
    });
    check(response, {
      [`synthetic credential denied for ${method} ${path}`]: (r) =>
        r.status === 403,
    });
  }
  const publicHealth = http.get(`${publicUrl}/api/health`, {
    tags: { kind: "journey" },
    redirects: 0,
  });
  const publicUi = http.get(publicUrl, {
    tags: { kind: "journey" },
    redirects: 0,
  });
  check(publicHealth, {
    "public API is healthy": (r) =>
      r.status === 200 && r.json().status === "healthy",
  });
  check(publicUi, {
    "public UI responds": (r) => r.status === 200 && r.body.includes("<html"),
  });
}
