import fs from "node:fs";

export const ridePayload = JSON.parse(
  fs.readFileSync(
    new URL("../fixtures/strava-real-ride.json", import.meta.url),
    "utf8",
  ),
);

const { activity: summary, streams } = ridePayload;
export const ride = {
  id: 887654322,
  title: summary.name,
  sport: "ride",
  source: "strava_sync",
  activity_type: "training",
  started_at: summary.start_date,
  distance_meters: summary.distance,
  moving_time_seconds: summary.moving_time,
  total_time_seconds: summary.elapsed_time,
  average_speed_mps: summary.distance / summary.moving_time,
  max_speed_mps: summary.max_speed,
  segment_efforts: [],
  laps: [],
  heart_rate_zones: [],
  achievement_highlights: [],
  route_points: streams.latlng.data.map(([latitude, longitude], index) => ({
    latitude,
    longitude,
    elapsed_seconds: streams.time.data[index],
    distance_meters: streams.distance.data[index],
    elevation_meters: streams.altitude.data[index],
    speed_mps: streams.velocity_smooth.data[index],
  })),
};

const pixel = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScL/nwAAAABJRU5ErkJggg==",
  "base64",
);
const pagination = { page: 1, per_page: 10, total: 1, total_pages: 1 };

export async function fakeProductApi(page, target, { signedIn = true } = {}) {
  const state = { signedIn, requests: [], unexpected: [] };
  await page.route("**/api/**", async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname.replace(/^.*\/api/, "");
    state.requests.push(path);
    let json;
    switch (path) {
      case "/auth/current":
        await route.fulfill({
          status: state.signedIn ? 200 : 401,
          json: state.signedIn
            ? {
                pid: "fixture-user",
                email: "fixture@example.test",
                name: "Fixture Rider",
                verified: true,
                is_admin: false,
              }
            : { message: "Unauthorized" },
        });
        return;
      case "/oauth/providers":
        json = {
          providers: [{ id: "sso", label: "Continue with fixture SSO" }],
        };
        break;
      case "/oauth/sso":
        state.signedIn = true;
        await route.fulfill({
          status: 303,
          headers: {
            location: new URL("/auth/callback", target.url).toString(),
          },
        });
        return;
      case "/auth/logout":
        state.signedIn = false;
        await route.fulfill({ status: 204 });
        return;
      case "/preferences":
        json = { unit_system: "metric" };
        break;
      case "/feature-flags":
        json = { data: [], metadata: { ...pagination, total: 0 } };
        break;
      case "/segments":
        json = [];
        break;
      case "/activities":
        json = { data: [ride], metadata: pagination };
        break;
      case `/activities/${ride.id}`:
        json = ride;
        break;
      default:
        if (
          path.startsWith(`/activities/${ride.id}/`) &&
          path.includes("map")
        ) {
          await route.fulfill({ contentType: "image/png", body: pixel });
          return;
        }
        state.unexpected.push(`${request.method()} ${path}`);
        await route.abort();
        return;
    }
    await route.fulfill({ json });
  });
  await page.route(
    /^https:\/\/(?:tiles\.openfreemap\.org|tile\.waymarkedtrails\.org|server\.arcgisonline\.com)\//,
    async (route) => {
      if (route.request().url().includes("/styles/")) {
        await route.fulfill({ json: { version: 8, sources: {}, layers: [] } });
      } else {
        await route.fulfill({ contentType: "image/png", body: pixel });
      }
    },
  );
  return state;
}
