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

function greenPixels(buffer) {
  const png = PNG.sync.read(buffer);
  let count = 0;
  for (let i = 0; i < png.data.length; i += 4) {
    const [red, green, blue] = png.data.subarray(i, i + 3);
    if (green > red + 40 && green > blue + 40) count++;
  }
  return count;
}

for (const target of targets) {
  for (const route of [
    { path: "/maps", label: "Personal activity heatmap" },
    { path: `/activities/${ride.id}`, label: "Activity route map" },
  ]) {
    test(`${target.name}: ${route.label} renders its basemap with the bundled worker`, async ({
      page,
    }) => {
      const diagnostics = [];
      page.on("console", (message) => {
        if (["warning", "error"].includes(message.type())) {
          diagnostics.push(message.text());
        }
      });
      page.on("pageerror", (error) => diagnostics.push(error.message));
      const api = await fakeProductApi(page, target);
      await page.route("**/api/feature-flags", (request) =>
        request.fulfill({
          json: {
            data: [
              { feature_key: "heatmaps", enabled: true },
              { feature_key: "enhanced_maps", enabled: true },
            ],
            metadata: { page: 1, per_page: 10, total: 2, total_pages: 1 },
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
      await page.route("**/map-styles/*.json", (request) =>
        request.fulfill({ json: basemap }),
      );
      await page.goto(new URL(route.path, target.url).toString());
      const canvas = page
        .locator(`[aria-label="${route.label}"]`)
        .locator("canvas");
      await expect(canvas).toBeVisible();
      await expect
        .poll(async () => greenPixels(await canvas.screenshot()))
        .toBeGreaterThan(1000);
      expect(api.unexpected).toEqual([]);
      expect(diagnostics).toEqual([]);
    });
  }
}
