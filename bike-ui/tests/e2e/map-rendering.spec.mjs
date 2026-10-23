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

function pixelsWithColor(buffer, color, tolerance = 0) {
  const png = PNG.sync.read(buffer);
  let count = 0;
  for (let i = 0; i < png.data.length; i += 4) {
    if (
      color.every(
        (channel, offset) =>
          Math.abs(png.data[i + offset] - channel) <= tolerance,
      )
    )
      count++;
  }
  return count;
}

function directionArrowPixels(buffer, routeColor) {
  const png = PNG.sync.read(buffer);
  const matches = (index, color) =>
    color.every(
      (channel, offset) => Math.abs(png.data[index + offset] - channel) <= 16,
    );
  let count = 0;
  for (let y = 8; y < png.height - 8; y++) {
    for (let x = 8; x < png.width - 8; x++) {
      const index = (y * png.width + x) * 4;
      if (!matches(index, [255, 255, 255])) continue;
      // Arrow interiors have route color on opposite sides; the outer white
      // casing has the basemap on one side of the straight fixture route.
      if (
        [3, 4, 5, 6].some((distance) =>
          [4, png.width * 4, (png.width + 1) * 4, (png.width - 1) * 4].some(
            (stride) =>
              matches(index - distance * stride, routeColor) &&
              matches(index + distance * stride, routeColor),
          ),
        )
      )
        count++;
    }
  }
  return count;
}

function pointWithColor(buffer, color) {
  const png = PNG.sync.read(buffer);
  const pixels = [];
  const matches = (index) =>
    color.every((channel, offset) => png.data[index + offset] === channel);
  for (let i = 0; i < png.data.length; i += 4) {
    if (
      [-png.width * 4, -4, 0, 4, png.width * 4].every((offset) =>
        matches(i + offset),
      )
    )
      pixels.push(i / 4);
  }
  expect(pixels.length, `Rendered segment color ${color}`).toBeGreaterThan(0);
  const pixel = pixels[Math.floor(pixels.length / 2)];
  return { x: pixel % png.width, y: Math.floor(pixel / png.width) };
}

function routeScreenshot(canvas) {
  return canvas.screenshot({
    style: ".maplibregl-control-container { visibility: hidden; }",
  });
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
  await page.route("**/api/segments/16/personal-achievements", (request) =>
    request.fulfill({
      json: segment.efforts.map((effort, index) => ({
        position: index + 1,
        effort_id: effort.id,
        duration_seconds: effort.duration_seconds,
      })),
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

async function expectRouteEndpoints(map, checkFraming = true) {
  const start = map.getByLabel("Route start", { exact: true });
  const finish = map.getByLabel("Route finish", { exact: true });
  await expect(start.locator('svg[data-icon="play"]')).toBeVisible();
  await expect(finish.locator('svg[data-icon="flag-checkered"]')).toBeVisible();
  const bounds = await map.boundingBox();
  for (const marker of [start, finish]) {
    await expect(marker).toHaveCSS("pointer-events", "none");
    await expect(marker).toHaveCSS("width", "16px");
    await expect(marker).toHaveCSS("height", "16px");
    await expect(marker).toHaveText("");
    const centerOffset = await marker.evaluate((element) => {
      const marker = element.getBoundingClientRect();
      const svg = element.querySelector("svg").getBoundingClientRect();
      return {
        x: svg.x + svg.width / 2 - marker.x - marker.width / 2,
        y: svg.y + svg.height / 2 - marker.y - marker.height / 2,
      };
    });
    expect(Math.abs(centerOffset.x)).toBeLessThanOrEqual(1);
    expect(Math.abs(centerOffset.y)).toBeLessThanOrEqual(1);
    if (!checkFraming) continue;
    const label = await marker.boundingBox();
    expect(label.x).toBeGreaterThanOrEqual(bounds.x);
    expect(label.y).toBeGreaterThanOrEqual(bounds.y);
    expect(label.x + label.width).toBeLessThanOrEqual(bounds.x + bounds.width);
    expect(label.y + label.height).toBeLessThanOrEqual(
      bounds.y + bounds.height,
    );
  }
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
        if (route.label === "Segment comparison map") {
          const achievements = page.getByRole("region", {
            name: "Personal achievements",
          });
          await expect(achievements.getByRole("listitem")).toHaveCount(3);
          await expect(
            achievements.getByRole("link", { name: "Your PR 1m 30s" }),
          ).toHaveAttribute("href", "/segments/16?efforts=7286");
        }
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
          await expectRouteEndpoints(
            page.getByLabel(route.label, { exact: true }),
            route.label !== "Segment race viewer map",
          );
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

function matchedActivity(segmentCount, pointsPerSegment = 2) {
  return {
    ...ride,
    route_points: Array.from(
      { length: segmentCount * pointsPerSegment },
      (_, index) => ({
        elapsed_seconds: index * 60,
        latitude: 44.76 + Math.floor(index / pointsPerSegment) * 0.0015,
        longitude:
          -85.62 +
          (Math.floor(index / pointsPerSegment) % 2 === 0
            ? (index % pointsPerSegment) / (pointsPerSegment - 1)
            : 1 - (index % pointsPerSegment) / (pointsPerSegment - 1)) *
            0.012,
        distance_meters: index * 1000,
      }),
    ),
    segment_efforts: Array.from({ length: segmentCount }, (_, index) => ({
      segment_id: index + 1,
      segment_title: `Matched segment ${String(index + 1).padStart(2, "0")}`,
      effort_index: 1,
      duration_seconds: 60,
      start_route_point_index: index * pointsPerSegment,
      end_route_point_index: (index + 1) * pointsPerSegment - 1,
    })),
  };
}

const climbingRoute = routePoints.map((point, index) => ({
  ...point,
  elevation_meters: 100 + index * 40,
}));

async function exerciseRouteLayers(page, overview, theme, testInfo) {
  const button = page.getByRole("button", { name: "Map layers", exact: true });
  const zoomOut = await page
    .getByRole("button", { name: "Zoom out", exact: true })
    .boundingBox();
  expect((await button.boundingBox()).y).toBeGreaterThanOrEqual(
    zoomOut.y + zoomOut.height - 1,
  );
  await button.click();
  const segments = page.getByRole("checkbox", {
    name: "Segments",
    exact: true,
  });
  const climbs = page.getByRole("checkbox", { name: "Climbs", exact: true });
  await expect(segments).toBeChecked();
  await expect(climbs).not.toBeChecked();
  await segments.uncheck();
  await page.keyboard.press("Escape");
  await expect(button).toBeFocused();
  await expect(page.getByRole("group", { name: "Map segments" })).toHaveCount(
    0,
  );
  await expect
    .poll(async () =>
      pixelsWithColor(await routeScreenshot(overview), [244, 63, 94]),
    )
    .toBe(0);
  const plainRoute = await routeScreenshot(overview);

  await button.click();
  await climbs.check();
  await page.keyboard.press("Escape");
  await expect(
    page.getByText("Climb start → finish", { exact: true }),
  ).toBeVisible();
  await expect
    .poll(async () =>
      pixelsWithColor(await routeScreenshot(overview), [168, 85, 247], 16),
    )
    .toBeGreaterThan(10);
  const climbOnly = await routeScreenshot(overview);
  expect(climbOnly).not.toEqual(plainRoute);
  await testInfo.attach(`climb-layer-${theme}`, {
    body: climbOnly,
    contentType: "image/png",
  });

  await button.click();
  await segments.check();
  await page.keyboard.press("Escape");
  await expect
    .poll(async () =>
      pixelsWithColor(await routeScreenshot(overview), [244, 63, 94]),
    )
    .toBeGreaterThan(30);
  await expect
    .poll(async () =>
      pixelsWithColor(await routeScreenshot(overview), [168, 85, 247], 16),
    )
    .toBeGreaterThan(10);
  await testInfo.attach(`both-layers-${theme}`, {
    body: await routeScreenshot(overview),
    contentType: "image/png",
  });

  await page.evaluate(
    (nextTheme) =>
      document.documentElement.setAttribute("data-theme", nextTheme),
    theme === "light" ? "dark" : "light",
  );
  await button.click();
  await expect(climbs).toBeChecked();
  await expect(segments).toBeChecked();
  await page.keyboard.press("Escape");
  await expect
    .poll(async () =>
      pixelsWithColor(await routeScreenshot(overview), [168, 85, 247], 16),
    )
    .toBeGreaterThan(10);
  await page.evaluate(
    (nextTheme) =>
      document.documentElement.setAttribute("data-theme", nextTheme),
    theme,
  );

  await button.click();
  await climbs.uncheck();
  await page.keyboard.press("Escape");
  await expect(
    page.getByText("Climb start → finish", { exact: true }),
  ).toHaveCount(0);
  await expect
    .poll(async () =>
      pixelsWithColor(await routeScreenshot(overview), [168, 85, 247], 16),
    )
    .toBe(0);
}
async function expectSegmentStatuses(page, segments, colors) {
  for (const [index, segment] of segments.entries()) {
    const card = page.locator(`#activity-segment-${segment.segment_id}`);
    const status = card.locator(".status");
    const title = card.getByRole("link", {
      name: segment.segment_title,
      exact: true,
    });
    await expect(status).toHaveCSS(
      "background-color",
      `rgb(${colors[index].join(", ")})`,
    );
    const dot = await status.boundingBox();
    const label = await title.boundingBox();
    expect(dot.width).toBe(12);
    expect(dot.height).toBe(12);
    await expect(status).toHaveCSS("background-image", "none");
    expect(dot.x + dot.width).toBeLessThanOrEqual(label.x);
    expect(
      Math.abs(dot.y + dot.height / 2 - (label.y + label.height / 2)),
    ).toBeLessThanOrEqual(1);
  }
}

async function exerciseLegendHoverSweep(page, canvas, legend, colors) {
  const bounds = await canvas.boundingBox();
  await page.mouse.move(
    bounds.x + bounds.width / 2,
    bounds.y + bounds.height / 2,
  );
  const buttons = legend.getByRole("button");
  const positions = await buttons.evaluateAll((entries) =>
    entries.slice(0, 5).map((entry) => {
      const bounds = entry.getBoundingClientRect();
      return {
        x: bounds.x + bounds.width / 2,
        y: bounds.y + bounds.height / 2,
      };
    }),
  );
  for (const index of [0, 4, 0, 4, 0, 4, 0, 4]) {
    await page.mouse.move(positions[index].x, positions[index].y, {
      steps: 12,
    });
  }
  await expect
    .poll(async () => {
      const screenshot = await canvas.screenshot();
      return colors
        .slice(0, 10)
        .flatMap((color, index) =>
          pixelsWithColor(screenshot, color) > 30 ? [index] : [],
        );
    })
    .toEqual([4]);
  for (const index of [0, 1, 2, 3, 4, 3, 2, 1, 0]) {
    const button = await buttons.nth(index).boundingBox();
    await page.mouse.move(
      button.x + button.width / 2,
      button.y + button.height / 2,
      {
        steps: 8,
      },
    );
    await expect
      .poll(async () => {
        const screenshot = await canvas.screenshot();
        return colors
          .slice(0, 10)
          .flatMap((color, colorIndex) =>
            pixelsWithColor(screenshot, color) > 30 ? [colorIndex] : [],
          );
      })
      .toEqual([index]);
  }
  await page.mouse.move(0, 0);
}

async function exerciseMapSegmentSelection(page, canvas, legend, colors) {
  const initial = await routeScreenshot(canvas);
  const first = pointWithColor(initial, colors[1]);
  const second = pointWithColor(initial, colors[3]);
  const selected = legend.locator('[aria-pressed="true"]');
  const scrollBefore = await page.evaluate(() => scrollY);
  await canvas.hover({ position: first });
  await expect
    .poll(async () => pixelsWithColor(await canvas.screenshot(), colors[0]))
    .toBe(0);
  await canvas.click({ position: first });
  await page.mouse.move(0, 0);
  await expect(selected).toHaveCount(1);
  await expect
    .poll(async () => pixelsWithColor(await canvas.screenshot(), colors[1]))
    .toBeGreaterThan(30);
  await canvas.click({ position: second });
  await page.mouse.move(0, 0);
  await expect(selected).toHaveCount(2);
  await expect
    .poll(async () => pixelsWithColor(await canvas.screenshot(), colors[1]))
    .toBeGreaterThan(30);
  const multiple = await canvas.screenshot();
  expect(pixelsWithColor(multiple, colors[1])).toBeGreaterThan(30);
  expect(pixelsWithColor(multiple, colors[3])).toBeGreaterThan(30);
  expect(pixelsWithColor(multiple, colors[0])).toBe(0);
  expect(await page.evaluate(() => scrollY)).toBe(scrollBefore);
  await canvas.click({ position: first });
  await page.mouse.move(0, 0);
  await expect(selected).toHaveCount(1);
  await page.getByRole("button", { name: "Clear selection (1)" }).click();
  await expect(selected).toHaveCount(0);
  await expect
    .poll(async () => pixelsWithColor(await canvas.screenshot(), colors[0]))
    .toBeGreaterThan(30);
}

for (const target of targets) {
  for (const theme of ["light", "dark"]) {
    test(`${target.name}: closed routes center both endpoint icons on the same point in ${theme} theme`, async ({
      page,
    }, testInfo) => {
      const { api } = await fixture(page, target, theme);
      await page.route(`**/api/activities/${ride.id}`, (request) =>
        request.fulfill({
          json: {
            ...ride,
            route_points: [
              routePoints[0],
              { ...routePoints[1], longitude: -85.61 },
              routePoints[2],
              { ...routePoints[0], elapsed_seconds: 90, distance_meters: 1500 },
            ],
            segment_efforts: [],
          },
        }),
      );
      await page.goto(new URL(`/activities/${ride.id}`, target.url).toString());
      const map = page.getByLabel("Activity route map", { exact: true });
      await expectRouteEndpoints(map);
      const start = await map
        .getByLabel("Route start", { exact: true })
        .boundingBox();
      const finish = await map
        .getByLabel("Route finish", { exact: true })
        .boundingBox();
      expect(
        Math.abs(start.x + start.width / 2 - finish.x - finish.width / 2),
      ).toBeLessThanOrEqual(1);
      expect(
        Math.abs(start.y + start.height / 2 - finish.y - finish.height / 2),
      ).toBeLessThanOrEqual(1);
      await testInfo.attach(`route-endpoints-${theme}`, {
        body: await map.screenshot(),
        contentType: "image/png",
      });
      expect(api.unexpected).toEqual([]);
    });

    test(`${target.name}: climb and segment maps open independently in ${theme} theme`, async ({
      page,
    }, testInfo) => {
      const { api } = await fixture(page, target, theme);
      await page.route(`**/api/activities/${ride.id}`, (request) =>
        request.fulfill({
          json: {
            ...ride,
            route_points: climbingRoute,
            climb_output: {
              detector_version: 1,
              recording_sha256: "fixture-climbing-route",
              climbs: [
                {
                  climb_number: 1,
                  start_route_point_index: 0,
                  end_route_point_index: climbingRoute.length - 1,
                  start_elapsed_seconds: climbingRoute[0].elapsed_seconds,
                  end_elapsed_seconds: climbingRoute.at(-1).elapsed_seconds,
                  duration_seconds:
                    climbingRoute.at(-1).elapsed_seconds -
                    climbingRoute[0].elapsed_seconds,
                  distance_meters:
                    climbingRoute.at(-1).distance_meters -
                    climbingRoute[0].distance_meters,
                  elevation_gain_meters:
                    climbingRoute.at(-1).elevation_meters -
                    climbingRoute[0].elevation_meters,
                  average_grade_percent: 4,
                  route_points: climbingRoute,
                },
              ],
            },
            segment_efforts: [
              {
                segment_id: 1,
                segment_title: "Fixture climb segment",
                effort_index: 1,
                duration_seconds: 30,
                start_route_point_index: 0,
                end_route_point_index: 1,
              },
            ],
          },
        }),
      );
      await page.goto(new URL(`/activities/${ride.id}`, target.url).toString());
      const overview = page
        .getByRole("img", { name: "Activity route map" })
        .locator("canvas");
      await expect
        .poll(async () =>
          pixelsWithColor(await routeScreenshot(overview), [0, 96, 223]),
        )
        .toBeGreaterThan(50);
      await expect
        .poll(async () =>
          pixelsWithColor(await routeScreenshot(overview), [244, 63, 94]),
        )
        .toBeGreaterThan(30);
      const before = await routeScreenshot(overview);
      await testInfo.attach(`overview-before-${theme}`, {
        body: before,
        contentType: "image/png",
      });
      expect(pixelsWithColor(before, [245, 158, 11])).toBe(0);
      await exerciseRouteLayers(page, overview, theme, testInfo);
      await expect
        .poll(async () =>
          PNG.sync
            .read(await routeScreenshot(overview))
            .data.equals(PNG.sync.read(before).data),
        )
        .toBe(true);
      await page.getByRole("tab", { name: "Climbs (1)", exact: true }).click();
      const climb = page.getByRole("button", { name: "Show climb 1 details" });
      await climb.click();
      await expect(climb).toHaveAttribute("aria-pressed", "true");
      await expect(
        page.getByRole("button", { name: "Zoom out map" }),
      ).toHaveCount(0);
      expect(
        PNG.sync
          .read(await routeScreenshot(overview))
          .data.equals(PNG.sync.read(before).data),
      ).toBe(true);
      await page.getByRole("button", { name: "View Climb 1 on map" }).click();
      const climbDialog = page.getByRole("dialog", {
        name: "Climb 1",
        exact: true,
      });
      const climbCanvas = climbDialog
        .getByRole("img", { name: "Climb 1 map" })
        .locator("canvas");
      await expect
        .poll(async () =>
          pixelsWithColor(await climbCanvas.screenshot(), [0, 96, 223]),
        )
        .toBeGreaterThan(50);
      await expectRouteEndpoints(
        climbDialog.getByLabel("Climb 1 map", { exact: true }),
      );
      await expect
        .poll(async () =>
          directionArrowPixels(await climbCanvas.screenshot(), [0, 96, 223]),
        )
        .toBeGreaterThan(4);
      await testInfo.attach(`climb-map-${theme}`, {
        body: await climbDialog.screenshot(),
        contentType: "image/png",
      });
      await page.keyboard.press("Escape");
      await expect(climbDialog).toHaveCount(0);
      await expect(climb).toHaveAttribute("aria-pressed", "true");

      await page.getByRole("tab", { name: /^Segments/ }).click();
      await page
        .getByRole("button", {
          name: "View Fixture climb segment on map",
          exact: true,
        })
        .click();
      const segmentDialog = page.getByRole("dialog", {
        name: "Fixture climb segment",
        exact: true,
      });
      const segmentCanvas = segmentDialog
        .getByRole("img", { name: "Fixture climb segment map" })
        .locator("canvas");
      await expect
        .poll(async () =>
          pixelsWithColor(await segmentCanvas.screenshot(), [244, 63, 94]),
        )
        .toBeGreaterThan(30);
      await expectRouteEndpoints(
        segmentDialog.getByLabel("Fixture climb segment map", { exact: true }),
      );
      await expect
        .poll(async () =>
          directionArrowPixels(await segmentCanvas.screenshot(), [244, 63, 94]),
        )
        .toBeGreaterThan(4);
      await testInfo.attach(`segment-map-${theme}`, {
        body: await segmentDialog.screenshot(),
        contentType: "image/png",
      });
      await segmentDialog
        .getByRole("button", { name: "Close", exact: true })
        .click();
      await expect(segmentDialog).toHaveCount(0);
      await expect(
        page.getByRole("button", { name: "Highlight Fixture climb segment" }),
      ).toHaveAttribute("aria-pressed", "false");
      expect(
        PNG.sync
          .read(await routeScreenshot(overview))
          .data.equals(PNG.sync.read(before).data),
      ).toBe(true);
      expect(api.unexpected).toEqual([]);
    });

    test(`${target.name}: activity segment palette and multiple selection in ${theme} theme`, async ({
      page,
    }, testInfo) => {
      await page.setViewportSize({ width: 1440, height: 1600 });
      const { api } = await fixture(page, target, theme);
      const activity = matchedActivity(22, 512);
      const segments = [...activity.segment_efforts];
      Object.assign(activity.segment_efforts.at(-1), {
        start_route_point_index: 512,
        end_route_point_index: 1023,
      });
      activity.segment_efforts.push({
        ...activity.segment_efforts[1],
        effort_index: 2,
        duration_seconds: 55,
      });
      await page.route(`**/api/activities/${ride.id}`, (request) =>
        request.fulfill({ json: activity }),
      );
      await page.route("**/map-styles/*.json", (request) =>
        request.fulfill({
          json: {
            ...basemap,
            layers: [
              basemap.layers[0],
              {
                ...basemap.layers[1],
                paint: {
                  "fill-color": theme === "dark" ? "#273e31" : "#b7dca9",
                },
              },
            ],
          },
        }),
      );
      await page.goto(new URL(`/activities/${ride.id}`, target.url).toString());
      const canvas = page
        .getByRole("img", { name: "Activity route map" })
        .locator("canvas");
      const legend = page.getByRole("group", { name: "Map segments" });
      const buttons = legend.getByRole("button");
      await expect(buttons).toHaveCount(22);
      await legend.scrollIntoViewIfNeeded();
      const colors = await legend
        .locator(".status")
        .evaluateAll((dots) =>
          dots.map((dot) =>
            getComputedStyle(dot).backgroundColor.match(/\d+/g).map(Number),
          ),
        );
      expect(new Set(colors.map(String)).size).toBe(10);
      await expect(
        page.getByText(
          "Hover to isolate a segment. Select segments to keep them highlighted.",
        ),
      ).toHaveCount(0);
      await expect(
        page.getByRole("button", { name: /Clear selection/ }),
      ).toHaveCount(0);
      await expect(legend.locator(".status").first()).toHaveCSS(
        "width",
        "12px",
      );
      await expect(legend.locator(".status").first()).toHaveCSS(
        "background-image",
        "none",
      );
      for (const color of colors.slice(0, 10)) {
        await expect
          .poll(async () => pixelsWithColor(await canvas.screenshot(), color))
          .toBeGreaterThan(30);
      }
      await testInfo.attach(`segment-palette-${theme}`, {
        body: await canvas.screenshot(),
        contentType: "image/png",
      });
      await exerciseMapSegmentSelection(page, canvas, legend, colors);
      await exerciseLegendHoverSweep(page, canvas, legend, colors);
      await expectSegmentStatuses(page, segments, colors);

      await buttons.nth(1).hover();
      await expect
        .poll(async () => pixelsWithColor(await canvas.screenshot(), colors[1]))
        .toBeGreaterThan(30);
      await expect
        .poll(async () => pixelsWithColor(await canvas.screenshot(), colors[0]))
        .toBe(0);
      const scrollBefore = await page.evaluate(() => scrollY);
      await buttons.nth(1).click();
      await buttons.nth(3).click();
      await page
        .getByRole("heading", { name: "Route map", exact: true })
        .hover();
      await expect(buttons.nth(1)).toHaveAttribute("aria-pressed", "true");
      await expect(buttons.nth(3)).toHaveAttribute("aria-pressed", "true");
      const clearSelection = legend.getByRole("button", {
        name: "Clear selection (2)",
      });
      await expect(
        clearSelection.locator('svg[data-icon="rectangle-xmark"]'),
      ).toBeVisible();
      await expect(clearSelection).toHaveText("Clear selection");
      const clear = await clearSelection.boundingBox();
      const lastSegment = await buttons.nth(21).boundingBox();
      expect(clear.x).toBeGreaterThanOrEqual(lastSegment.x + lastSegment.width);
      expect(Math.abs(clear.y - lastSegment.y)).toBeLessThanOrEqual(1);
      await testInfo.attach(`segment-legend-${theme}`, {
        body: await legend.screenshot(),
        contentType: "image/png",
      });
      const focused = await canvas.screenshot();
      expect(pixelsWithColor(focused, colors[1])).toBeGreaterThan(30);
      expect(pixelsWithColor(focused, colors[3])).toBeGreaterThan(30);
      expect(pixelsWithColor(focused, colors[0])).toBe(0);
      expect(await page.evaluate(() => scrollY)).toBe(scrollBefore);
      await testInfo.attach(`selected-segments-${theme}`, {
        body: focused,
        contentType: "image/png",
      });
      await page.getByRole("button", { name: "Clear selection (2)" }).click();
      await expect
        .poll(async () => pixelsWithColor(await canvas.screenshot(), colors[0]))
        .toBeGreaterThan(30);
      await expect(
        page.getByRole("button", { name: /Clear selection/ }),
      ).toHaveCount(0);
      expect(api.unexpected).toEqual([]);
    });
  }
}
