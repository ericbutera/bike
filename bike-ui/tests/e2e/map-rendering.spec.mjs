import { expect, test } from "./helpers/test.mjs";
import { PNG } from "pngjs";
import { fakeProductApi, ride } from "./helpers/product-fixtures.mjs";
import { targets } from "./helpers/targets.mjs";

// Use Chromium's normal GPU renderer rather than the software headless shell.
test.use({ channel: "chromium" });

// A GeoJSON basemap exercises the real MapLibre worker without a tile provider.
const basemap = {
  version: 8,
  sources: {
    land: {
      type: "geojson",
      data: {
        type: "Feature",
        properties: {},
        geometry: {
          type: "Polygon",
          coordinates: [
            [
              [-90, 40],
              [-80, 40],
              [-80, 50],
              [-90, 50],
              [-90, 40],
            ],
          ],
        },
      },
    },
  },
  layers: [
    {
      id: "background",
      type: "background",
      paint: { "background-color": "#eeeeee" },
    },
    {
      id: "land",
      type: "fill",
      source: "land",
      paint: { "fill-color": "#36a86d" },
    },
  ],
};

function pixelsWithColor(buffer, color) {
  const png = PNG.sync.read(buffer);
  let count = 0;
  for (let i = 0; i < png.data.length; i += 4) {
    const [red, green, blue] = png.data.subarray(i, i + 3);
    if (red === color[0] && green === color[1] && blue === color[2]) count++;
  }
  return count;
}

// A long enough synthetic route keeps blue visible between endpoint markers
// at the activity map's maximum initial zoom of 14.
const routePoints = [
  {
    elapsed_seconds: 0,
    latitude: 44.76,
    longitude: -85.62,
    distance_meters: 0,
  },
  {
    elapsed_seconds: 30,
    latitude: 44.765,
    longitude: -85.615,
    distance_meters: 500,
  },
  {
    elapsed_seconds: 60,
    latitude: 44.77,
    longitude: -85.61,
    distance_meters: 1000,
  },
];
const segment = {
  id: 16,
  title: "Fixture segment",
  source: "manual",
  mode: "dh",
  effort_count: 3,
  distance_meters: 1000,
  created_at: ride.started_at,
  route_points: routePoints,
  efforts: [7286, 7249, 6945].map((id, index) => ({
    id,
    activity_id: ride.id,
    activity_title: `Fixture ride ${index + 1}`,
    rider_user_id: 1,
    rider_name: `Fixture rider ${index + 1}`,
    activity_started_at: ride.started_at,
    effort_index: index + 1,
    duration_seconds: 90 + index * 5,
    start_elapsed_seconds: 0,
    end_elapsed_seconds: 90 + index * 5,
    distance_meters: 1000,
    route_points: routePoints,
  })),
};

async function fixture(page, target, theme) {
  await page.addInitScript(
    (theme) => localStorage.setItem("bike-theme", theme),
    theme,
  );
  const api = await fakeProductApi(page, target);
  await page.route(`**/api/activities/${ride.id}`, (request) =>
    request.fulfill({ json: { ...ride, route_points: routePoints } }),
  );
  await page.route("**/api/segments/16**", (request) =>
    request.fulfill({
      json: new URL(request.request().url()).pathname.endsWith("/comparison")
        ? {
            segment_id: segment.id,
            route_points: routePoints,
            efforts: segment.efforts,
          }
        : segment,
    }),
  );
  await page.route("**/api/feature-flags", (request) =>
    request.fulfill({
      json: {
        data: [
          { feature_key: "heatmaps", enabled: true },
          { feature_key: "enhanced_maps", enabled: true },
        ],
      },
    }),
  );
  await page.route("**/api/maps/heatmap**", (request) =>
    request.fulfill({
      json: {
        revision: "fixture",
        style_version: "v1",
        filters: {},
        ready: 0,
        failed: 0,
        pending: 0,
        skipped: 0,
        preparing: false,
        bounds: null,
        min_zoom: 0,
        max_zoom: 18,
        tile_size: 512,
        zones: [],
      },
    }),
  );
  const styleRequests = [];
  await page.route("**/map-styles/*.json", (request) => {
    const path = new URL(request.request().url()).pathname;
    styleRequests.push(path);
    return request.fulfill({
      json: {
        ...basemap,
        layers: [
          basemap.layers[0],
          {
            ...basemap.layers[1],
            paint: {
              "fill-color": path.includes("fiord") ? "#167e49" : "#36a86d",
            },
          },
        ],
      },
    });
  });
  return { api, styleRequests };
}

for (const target of targets) {
  for (const theme of ["light", "dark"]) {
    for (const route of [
      {
        path: "/maps?lng=-85.62&lat=44.76&zoom=13",
        label: "Personal activity heatmap",
      },
      { path: `/activities/${ride.id}`, label: "Activity route map" },
      { path: "/segments/16", label: "Segment comparison map" },
      {
        path: "/segments/16/race?efforts=7286%2C7249%2C6945",
        label: "Segment race viewer map",
      },
    ]) {
      test(`${target.name}: ${route.label} uses the shared ${theme} background and blue route`, async ({
        page,
      }, testInfo) => {
        const { api, styleRequests } = await fixture(page, target, theme);
        const backgroundColor =
          theme === "dark" ? [22, 126, 73] : [54, 168, 109];
        await page.goto(new URL(route.path, target.url).toString());
        const canvas = page
          .locator(`[aria-label="${route.label}"]`)
          .locator("canvas");
        await expect(canvas).toBeVisible();
        await expect
          .poll(async () =>
            pixelsWithColor(await canvas.screenshot(), backgroundColor),
          )
          .toBeGreaterThan(1000);
        expect(styleRequests.at(-1)).toBe(
          `/map-styles/${theme === "dark" ? "fiord" : "route-light"}-v1.json`,
        );
        if (route.label !== "Personal activity heatmap") {
          await expect
            .poll(async () =>
              pixelsWithColor(await canvas.screenshot(), [0, 96, 223]),
            )
            .toBeGreaterThan(100);
        }
        await testInfo.attach("map-appearance", {
          body: await canvas.screenshot(),
          contentType: "image/png",
        });
        expect(api.unexpected).toEqual([]);
      });
    }
  }
}
