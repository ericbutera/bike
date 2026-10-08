import { test, expect } from "@playwright/test";
import { PNG } from "pngjs";
import { targets } from "./helpers/targets.mjs";
import { openRoute, artifactRoot, stabilize } from "./helpers/ui.mjs";
import { openFrontendRoute } from "./helpers/frontend.mjs";
import fs from "node:fs/promises";
import path from "node:path";

const selected = process.env.PLAYWRIGHT_TARGET
  ? targets.filter((target) => target.name === process.env.PLAYWRIGHT_TARGET)
  : targets;
const blank = PNG.sync.write({ width: 1, height: 1, data: Buffer.alloc(4) });

async function fakeBasemapProvider(
  page,
  real = process.env.HEATMAP_REAL_BASEMAP === "1",
) {
  if (real) return;
  await page.route(/^https:\/\/tiles\.openfreemap\.org\//, async (route) => {
    const url = route.request().url();
    let body;
    let contentType;
    if (url.endsWith("/planet")) {
      contentType = "application/json";
      body = JSON.stringify({
        tilejson: "3.0.0",
        tiles: ["https://tiles.openfreemap.org/fixture/{z}/{x}/{y}.pbf"],
        minzoom: 0,
        maxzoom: 14,
        vector_layers: [],
      });
    } else if (url.endsWith(".json")) {
      contentType = "application/json";
      body = "{}";
    } else if (url.endsWith(".pbf")) {
      contentType = "application/x-protobuf";
      body = Buffer.alloc(0);
    } else {
      contentType = "image/png";
      body = blank;
    }
    await route.fulfill({
      contentType,
      body,
      headers: { "access-control-allow-origin": "*" },
    });
  });
}

function bluePixels(buffer) {
  const png = PNG.sync.read(buffer);
  let count = 0;
  for (let i = 0; i < png.data.length; i += 4) {
    const [red, green, blue] = png.data.subarray(i, i + 3);
    if (blue > red + 40 && blue > green + 20) count++;
  }
  return count;
}

for (const target of selected) {
  test(`${target.name}: zoom retains heat while detailed tiles are delayed`, async ({
    page,
  }) => {
    test.skip(
      !process.env.HEATMAP_PREVIEW_FILTERS,
      "Select a bounded known-route filter for zoom verification.",
    );
    test.setTimeout(60_000);
    // Keep basemap colors out of the pixel assertion; overlay tiles use the real API.
    await fakeBasemapProvider(page, false);
    let hold = false;
    let resume;
    let notifyRequest;
    const gate = new Promise((resolve) => {
      resume = resolve;
    });
    const requested = new Promise((resolve) => {
      notifyRequest = resolve;
    });
    await page.route(/\/heatmap-tiles\//, async (route) => {
      if (hold) {
        notifyRequest();
        await gate;
      }
      await route.continue();
    });
    try {
      const flagResponse = page.waitForResponse((response) =>
        new URL(response.url()).pathname.endsWith("/feature-flags"),
      );
      await openFrontendRoute(
        page,
        target,
        {
          name: "heatmap-zoom",
          path: `/maps?${process.env.HEATMAP_PREVIEW_FILTERS}`,
        },
        { retryAuthRedirect: true },
      );
      const flags = await (await flagResponse).json();
      const enabled = flags.data.some(
        (flag) => flag.feature_key === "heatmaps" && flag.enabled,
      );
      if (process.env.HEATMAP_REQUIRE_ENABLED === "1")
        expect(enabled).toBe(true);
      test.skip(!enabled, "Heatmaps are disabled on this site.");
      const map = page.getByRole("region", {
        name: "Personal activity heatmap",
      });
      const canvas = map.locator("canvas");
      await expect(canvas).toBeVisible();
      await page.getByLabel("Heatmap help and legend").click();
      await expect(
        page.getByText(/^[1-9][\d,]* activities with routes/),
      ).toBeVisible();
      await page.getByLabel("Heatmap help and legend").click();
      await expect
        .poll(async () => bluePixels(await canvas.screenshot()))
        .toBeGreaterThan(5);
      hold = true;
      await map.getByRole("button", { name: "Zoom in", exact: true }).click();
      await requested;
      expect(bluePixels(await canvas.screenshot())).toBeGreaterThan(5);
      const loaded = page.waitForResponse(
        (response) =>
          response.url().includes("/heatmap-tiles/") &&
          response.status() === 200,
      );
      hold = false;
      resume();
      await loaded;
      await expect
        .poll(async () => bluePixels(await canvas.screenshot()))
        .toBeGreaterThan(5);
      await map.getByRole("button", { name: "Zoom out", exact: true }).click();
      await expect
        .poll(async () => bluePixels(await canvas.screenshot()))
        .toBeGreaterThan(5);
    } finally {
      hold = false;
      resume();
    }
  });
}

for (const target of selected) {
  test(`${target.name}: personal heatmap tiles and filters`, async ({
    page,
  }) => {
    test.setTimeout(90_000);
    await fakeBasemapProvider(page);
    const tileResponses = [];
    page.on("response", (response) => {
      if (response.url().includes("/heatmap-tiles/"))
        tileResponses.push(response);
    });
    const flagResponse = page.waitForResponse((response) =>
      new URL(response.url()).pathname.endsWith("/feature-flags"),
    );
    await openRoute(page, target, "/maps");
    const flags = await (await flagResponse).json();
    const enabled = flags.data.some(
      (flag) => flag.feature_key === "heatmaps" && flag.enabled,
    );
    if (process.env.HEATMAP_REQUIRE_ENABLED === "1") expect(enabled).toBe(true);
    const disabled = page.getByText("Heatmaps are not enabled on this site.");
    const toolbar = page.getByRole("toolbar", { name: "Heatmap controls" });
    if (!enabled) {
      await expect(disabled).toBeVisible();
      await expect(
        page.getByRole("link", { name: "Maps", exact: true }),
      ).toHaveCount(0);
      expect(tileResponses).toHaveLength(0);
      return;
    }
    await expect(toolbar).toBeVisible();
    await expect(
      page.getByRole("link", { name: "Maps", exact: true }),
    ).toBeVisible();
    const map = page.getByRole("region", { name: "Personal activity heatmap" });
    await expect(map.locator("canvas")).toBeVisible();
    await page.getByLabel("Heatmap help and legend").click();
    await expect(page.getByText(/\d+ activities with routes/)).toBeVisible();
    await page.getByLabel("Heatmap help and legend").click();
    await expect
      .poll(() => tileResponses.filter((r) => r.status() === 200).length)
      .toBeGreaterThan(0);
    const successful = tileResponses.find((r) => r.status() === 200);
    const png = PNG.sync.read(await successful.body());
    expect(png.width).toBe(512);
    expect(png.height).toBe(512);
    const apiUrl = await page.evaluate(() => window.__APP_CONFIG__.API_URL);
    const metadataRequest = await page.request.get(`${apiUrl}/maps/heatmap`);
    expect(metadataRequest.status()).toBe(200);
    const metadata = await metadataRequest.json();
    expect(metadata.ready).toBeGreaterThan(0);
    const world = await page.request.get(
      new URL(
        `/heatmap-tiles/0/0/0.png?revision=${metadata.revision}`,
        target.url,
      ).toString(),
    );
    expect(world.status()).toBe(200);
    const worldPng = PNG.sync.read(await world.body());
    const colors = new Set();
    for (let i = 0; i < worldPng.data.length; i += 4) {
      if (worldPng.data[i + 3])
        colors.add([...worldPng.data.subarray(i, i + 3)].join(","));
    }
    expect(colors).toEqual(new Set(["0,96,223"]));
    expect(
      [...worldPng.data]
        .filter((_, i) => i % 4 === 3)
        .some((alpha) => alpha > 0),
    ).toBe(true);
    expect(world.headers()["cache-control"]).toBe("private, no-cache");
    const conditional = await page.request.get(
      new URL(
        `/heatmap-tiles/0/0/0.png?revision=${metadata.revision}`,
        target.url,
      ).toString(),
      { headers: { "if-none-match": world.headers().etag } },
    );
    expect(conditional.status()).toBe(304);
    await map.locator("canvas").evaluate((canvas) => {
      canvas.dataset.heatmapIdentity = "same-map";
    });
    await page.getByText("Filters", { exact: true }).click();
    await page.getByLabel("Activity type").selectOption("road_ride");
    await expect(page).toHaveURL(/sport=road_ride/);
    await expect(map.locator("canvas")).toHaveAttribute(
      "data-heatmap-identity",
      "same-map",
    );
    await page.getByRole("button", { name: "This year", exact: true }).click();
    await expect(page).toHaveURL(/start=\d{4}-01-01/);
    await expect(map.locator("canvas")).toHaveAttribute(
      "data-heatmap-identity",
      "same-map",
    );
    await page.getByRole("button", { name: "All time", exact: true }).click();
    await expect(page).not.toHaveURL(/start=/);
    await page.getByLabel("Activity type").selectOption("");
    await expect(page).not.toHaveURL(/sport=/);
    await page.getByText("Filters", { exact: true }).click();
    await page.getByLabel("Heatmap help and legend").click();
    await expect(
      page.getByText(
        `${metadata.ready.toLocaleString()} activities with routes`,
      ),
    ).toBeVisible();
    await expect(page.getByLabel("Activities per path legend")).toBeVisible();
    await page.getByLabel("Heatmap help and legend").click();
    await page.getByRole("button", { name: "Account", exact: true }).click();
    await page.getByRole("checkbox", { name: "Switch to dark mode" }).check();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
    await expect(map.locator("canvas")).toHaveAttribute(
      "data-heatmap-identity",
      "same-map",
    );
    await page.keyboard.press("Escape");
    await page.getByRole("button", { name: "Account", exact: true }).click();
    await page
      .getByRole("checkbox", { name: "Switch to light mode" })
      .uncheck();
    await page.keyboard.press("Escape");
    if (process.env.HEATMAP_PREVIEW_FILTERS) {
      const previewTile = page.waitForResponse(
        (response) =>
          response.url().includes("/heatmap-tiles/") &&
          response.status() === 200,
      );
      await openRoute(
        page,
        target,
        `/maps?${process.env.HEATMAP_PREVIEW_FILTERS}`,
      );
      await page.getByLabel("Heatmap help and legend").click();
      await expect(page.getByText(/\d+ activities with routes/)).toBeVisible();
      await page.getByLabel("Heatmap help and legend").click();
      await previewTile;
    }
    await page.getByText("Zoom preset", { exact: true }).click();
    await page.getByRole("button", { name: "Full", exact: true }).click();
    await page.mouse.move(10, 850);
    await stabilize(page);
    await fs.mkdir(path.join(artifactRoot, "screenshots"), { recursive: true });
    await page.screenshot({
      path: path.join(
        artifactRoot,
        "screenshots",
        `${target.name}-heatmap.png`,
      ),
      fullPage: true,
    });
  });
}
