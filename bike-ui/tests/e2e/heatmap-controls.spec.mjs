import { expect, test } from "./helpers/test.mjs";
import { PNG } from "pngjs";
import { fakeProductApi } from "./helpers/product-fixtures.mjs";
import { targets } from "./helpers/targets.mjs";

test.use({ channel: "chromium" });
const png = PNG.sync.write({
  width: 512,
  height: 512,
  data: Buffer.alloc(512 * 512 * 4),
});
const metadata = {
  revision: "fixture",
  filters: {},
  ready: 3,
  pending: 0,
  failed: 0,
  skipped: 0,
  bounds: [-122.4, 44.7, -85.5, 47.7],
  preparing: false,
  min_zoom: 0,
  max_zoom: 18,
  tile_size: 512,
  style_version: "heatmap-v2",
};
const zones = [
  { longitude: -85.62, latitude: 44.76 },
  { longitude: -85.63, latitude: 44.77 },
  { longitude: -122.33, latitude: 47.61 },
];

async function fixture(page, target, zonesReady = Promise.resolve()) {
  const diagnostics = [];
  page.on("console", (message) => {
    if (["warning", "error"].includes(message.type()))
      diagnostics.push(message.text());
  });
  page.on("pageerror", (error) => diagnostics.push(error.message));
  const api = await fakeProductApi(page, target);
  await page.route("https://fonts.googleapis.com/**", (route) =>
    route.fulfill({ contentType: "text/css", body: "" }),
  );
  await page.route("**/api/feature-flags", (route) =>
    route.fulfill({
      json: { data: [{ feature_key: "heatmaps", enabled: true }] },
    }),
  );
  await page.route("**/api/maps/heatmap**", async (route) => {
    const isZones = route.request().url().includes("/zones");
    if (isZones) await zonesReady;
    await route.fulfill({
      json: isZones ? { revision: "fixture", zones } : metadata,
    });
  });
  await page.route("**/heatmap-tiles/**", (route) =>
    route.fulfill({ contentType: "image/png", body: png }),
  );
  await page.route("**/map-styles/*.json", (route) =>
    route.fulfill({
      json: {
        version: 8,
        sources: {},
        layers: [
          {
            id: "background",
            type: "background",
            paint: { "background-color": "#e8ece8" },
          },
        ],
      },
    }),
  );
  return { diagnostics, api };
}

function camera(page) {
  const params = new URL(page.url()).searchParams;
  return {
    longitude: Number(params.get("lng")),
    latitude: Number(params.get("lat")),
    zoom: Number(params.get("zoom")),
  };
}

for (const target of targets) {
  for (const initialQuery of [
    "sport=road_ride",
    "sport=road_ride&lng=0&lat=0&zoom=2",
  ]) {
    test(`${target.name}: delayed routes choose local detail from ${initialQuery}`, async ({
      page,
      context,
    }) => {
      const pending = Promise.withResolvers();
      const { diagnostics, api } = await fixture(page, target, pending.promise);
      await context.grantPermissions([], { origin: target.url });
      const zonesRequested = page.waitForRequest("**/api/maps/heatmap/zones**");
      try {
        await page.goto(
          new URL(`/maps?${initialQuery}`, target.url).toString(),
        );
        const canvas = page
          .getByRole("region", { name: "Personal activity heatmap" })
          .locator("canvas");
        await expect(canvas).toBeVisible();
        expect(
          await page.evaluate(
            async () =>
              (await navigator.permissions.query({ name: "geolocation" }))
                .state,
          ),
        ).toBe("denied");
        await zonesRequested;
        await page.setViewportSize({ width: 1200, height: 800 });
        await page.evaluate(
          () =>
            new Promise((resolve) =>
              requestAnimationFrame(() => requestAnimationFrame(resolve)),
            ),
        );
        expect(new URL(page.url()).searchParams.toString()).toBe(initialQuery);
        pending.resolve();
        await expect.poll(() => camera(page).zoom).toBe(13);
        expect(camera(page).longitude).toBeCloseTo(-85.625);
        expect(camera(page).latitude).toBeCloseTo(44.765);
        expect(new URL(page.url()).searchParams.get("sport")).toBe("road_ride");
        await page.reload();
        await expect(canvas).toBeVisible();
        expect(camera(page).longitude).toBeCloseTo(-85.625);
        expect(camera(page).zoom).toBe(13);
        expect(api.unexpected).toEqual([]);
        expect(diagnostics).toEqual([]);
      } finally {
        pending.resolve();
      }
    });
  }
  test(`${target.name}: automatic location takes precedence over delayed route centers`, async ({
    page,
    context,
  }) => {
    const pending = Promise.withResolvers();
    const { diagnostics, api } = await fixture(page, target, pending.promise);
    await context.grantPermissions(["geolocation"], { origin: target.url });
    await context.setGeolocation({ longitude: -122.33, latitude: 47.61 });
    const zonesLoaded = page.waitForResponse("**/api/maps/heatmap/zones**");
    try {
      await page.goto(new URL("/maps?sport=road_ride", target.url).toString());
      await expect(
        page
          .getByRole("region", { name: "Personal activity heatmap" })
          .locator("canvas"),
      ).toBeVisible();
      await expect.poll(() => camera(page).zoom).toBe(13);
      expect(camera(page).longitude).toBeCloseTo(-122.33, 5);
      expect(camera(page).latitude).toBeCloseTo(47.61, 5);
      pending.resolve();
      await (await zonesLoaded).finished();
      await page.evaluate(
        () =>
          new Promise((resolve) =>
            requestAnimationFrame(() => requestAnimationFrame(resolve)),
          ),
      );
      expect(camera(page).longitude).toBeCloseTo(-122.33, 5);
      expect(new URL(page.url()).searchParams.get("sport")).toBe("road_ride");
      expect(api.unexpected).toEqual([]);
      expect(diagnostics).toEqual([]);
    } finally {
      pending.resolve();
    }
  });
  test(`${target.name}: local view, zoom presets, GPS and help work together`, async ({
    page,
    context,
  }) => {
    const { diagnostics, api } = await fixture(page, target);
    await context.grantPermissions(["geolocation"], { origin: target.url });
    await context.setGeolocation({ longitude: -85.62, latitude: 44.76 });
    await page.goto(new URL("/maps?sport=road_ride", target.url).toString());
    const map = page.getByRole("region", { name: "Personal activity heatmap" });
    await expect(map.locator("canvas")).toBeVisible();
    await expect.poll(() => camera(page).zoom).toBe(13);
    expect(camera(page).longitude).toBeCloseTo(-85.62, 5);
    expect(camera(page).latitude).toBeCloseTo(44.76, 5);
    await expect(
      page.getByRole("heading", { name: "Your heatmap" }),
    ).toHaveCount(0);
    const toolbar = page.getByRole("toolbar", { name: "Heatmap controls" });
    await expect(toolbar.locator("summary")).toHaveCount(4);
    const toolbarPosition = await toolbar.boundingBox();
    const mapPosition = await map.boundingBox();
    expect(toolbarPosition.x).toBeGreaterThan(mapPosition.width / 2);
    const buttons = map.locator(".maplibregl-ctrl-bottom-right button");
    await expect(buttons).toHaveCount(3);
    expect(
      await buttons.evaluateAll((items) =>
        items.map((item) => item.getAttribute("aria-label")),
      ),
    ).toEqual(["Zoom in", "Zoom out", "Use my location"]);
    await page.getByLabel("Heatmap help and legend").click();
    await expect(page.getByText(/Paths become more opaque/)).toBeVisible();
    await expect(page.getByText("3 activities with routes")).toBeVisible();
    await page.getByLabel("Heatmap help and legend").click();
    await page.getByText("Zoom preset", { exact: true }).click();
    await page.getByRole("button", { name: "Region", exact: true }).click();
    await expect.poll(() => camera(page).zoom).toBeLessThan(8);
    expect(camera(page).latitude).toBeGreaterThan(44);
    expect(camera(page).latitude).toBeLessThan(47);
    await page.getByText("Zoom preset", { exact: true }).click();
    await page.getByRole("button", { name: "Full", exact: true }).click();
    await expect.poll(() => camera(page).longitude).toBeLessThan(-100);
    await page.getByRole("button", { name: "Use my location" }).click();
    await expect.poll(() => camera(page).zoom).toBe(13);
    expect(camera(page).longitude).toBeCloseTo(-85.62);
    expect(new URL(page.url()).searchParams.get("sport")).toBe("road_ride");
    await page.screenshot({
      path: ".artifacts/playwright/heatmap-controls-desktop.png",
    });
    expect(api.unexpected).toEqual([]);
    expect(diagnostics).toEqual([]);
  });

  test(`${target.name}: saved view and mobile controls remain usable`, async ({
    page,
    context,
  }) => {
    const { diagnostics } = await fixture(page, target);
    await context.grantPermissions(["geolocation"], { origin: target.url });
    await context.setGeolocation({ longitude: -85.62, latitude: 44.76 });
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto(
      new URL("/maps?lng=-122&lat=47&zoom=9", target.url).toString(),
    );
    await expect(
      page
        .getByRole("region", { name: "Personal activity heatmap" })
        .locator("canvas"),
    ).toBeVisible();
    expect(camera(page)).toEqual({ longitude: -122, latitude: 47, zoom: 9 });
    await expect(
      page.getByRole("button", { name: "Use my location" }),
    ).toBeVisible();
    const help = page.getByLabel("Heatmap help and legend");
    await help.focus();
    await page.keyboard.press("Enter");
    await expect(page.getByLabel("Activities per path legend")).toBeVisible();
    await page.screenshot({
      path: ".artifacts/playwright/heatmap-controls-mobile.png",
    });
    const legend = await page
      .getByLabel("Activities per path legend")
      .boundingBox();
    expect(legend.x).toBeGreaterThanOrEqual(0);
    expect(legend.x + legend.width).toBeLessThanOrEqual(390);
    expect(diagnostics).toEqual([]);
  });
}
