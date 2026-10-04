import { test, expect } from "@playwright/test";
import { PNG } from "pngjs";
import fs from "node:fs/promises";
import {
  activityId,
  segmentId,
  raceEffortIds,
  targets,
} from "./helpers/targets.mjs";
import { openRoute } from "./helpers/ui.mjs";

const selectedTargets = process.env.PLAYWRIGHT_TARGET
  ? targets.filter((target) => target.name === process.env.PLAYWRIGHT_TARGET)
  : targets;
if (selectedTargets.length === 0) throw new Error("Unknown PLAYWRIGHT_TARGET");

async function testAuthentication(page, target) {
  const file = process.env.BIKE_TEST_AUTH_FILE;
  if (!file) return {};
  const { token } = JSON.parse(await fs.readFile(file, "utf8"));
  if (typeof token !== "string" || !token)
    throw new Error("Missing test token");
  const headers = { authorization: `Bearer ${token}` };
  const origins = new Set([
    new URL(target.url).origin,
    new URL(process.env.BIKE_API_URL ?? target.url).origin,
  ]);
  // Forward credentials only to the explicitly selected application origins.
  await page.route(
    (url) => origins.has(url.origin),
    (route) =>
      route.continue({ headers: { ...route.request().headers(), ...headers } }),
  );
  return headers;
}

async function fakeExternalBasemaps(page) {
  await page.route(
    /^https:\/\/(?:tiles\.openfreemap\.org|tile\.waymarkedtrails\.org|server\.arcgisonline\.com)\//,
    async (route) => {
      const url = route.request().url();
      if (url.endsWith("/planet")) {
        await route.fulfill({
          contentType: "application/json",
          headers: { "access-control-allow-origin": "*" },
          body: JSON.stringify({
            tilejson: "3.0.0",
            tiles: ["https://tiles.openfreemap.org/fixture/{z}/{x}/{y}.pbf"],
            minzoom: 0,
            maxzoom: 14,
            vector_layers: [],
          }),
        });
      } else if (url.endsWith(".pbf")) {
        await route.fulfill({
          contentType: "application/x-protobuf",
          headers: { "access-control-allow-origin": "*" },
          body: Buffer.alloc(0),
        });
      } else if (url.includes("/styles/")) {
        await route.fulfill({
          contentType: "application/json",
          headers: { "access-control-allow-origin": "*" },
          body: JSON.stringify({ version: 8, sources: {}, layers: [] }),
        });
      } else if (url.endsWith(".json")) {
        await route.fulfill({
          contentType: "application/json",
          headers: { "access-control-allow-origin": "*" },
          body: "{}",
        });
      } else {
        await route.fulfill({
          contentType: "image/png",
          headers: { "access-control-allow-origin": "*" },
          body: PNG.sync.write({ width: 1, height: 1, data: Buffer.alloc(4) }),
        });
      }
    },
  );
}

for (const target of selectedTargets) {
  test(`${target.name}: activity, segment, and race hot paths`, async ({
    page,
  }) => {
    test.setTimeout(90_000);
    const authHeaders = await testAuthentication(page, target);
    // External tiles are faked. Product APIs, SQL and the owned map renderer are real.
    await fakeExternalBasemaps(page);
    await openRoute(page, target);
    await expect(
      page.getByRole("heading", { name: "Recent activities" }),
    ).toBeVisible();
    const rideLink = page.locator(
      `article h3 a[href="/activities/${activityId}"]`,
    );
    await expect(rideLink).toBeVisible();
    const rideTitle = await rideLink.innerText();
    const card = page.locator("article").filter({
      has: page.locator(`h3 a[href="/activities/${activityId}"]`),
    });
    const image = card.locator("img").first();
    await expect(image).toBeVisible();
    await expect
      .poll(() =>
        image.evaluate(
          (element) => element.complete && element.naturalWidth > 1,
        ),
      )
      .toBe(true);

    // Exercise the server-side API fetch and real rendering even when list-map flags are off.
    const renderedMap = await page.request.get(
      new URL(
        `/activity-map-images/thumbnail/1?activityId=${activityId}&theme=light&dpr=1`,
        target.url,
      ).toString(),
      { headers: authHeaders },
    );
    expect(renderedMap.status()).toBe(200);
    expect(renderedMap.headers()["content-type"]).toContain("image/png");
    const png = PNG.sync.read(await renderedMap.body());
    expect(png.width).toBeGreaterThan(1);
    expect(png.height).toBeGreaterThan(1);

    await rideLink.click();
    await expect(page).toHaveURL(new RegExp(`/activities/${activityId}$`));
    await expect(
      page.getByRole("heading", { name: rideTitle, exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole("heading", { name: "Matched segments" }),
    ).toBeVisible();
    await expect(
      page.locator('[aria-label="Activity route map"]'),
    ).toBeVisible();

    await page.getByRole("button", { name: "Training", exact: true }).click();
    await page.locator('a[href="/segments"]').first().click();
    await expect(
      page.getByRole("heading", { name: "Segments", exact: true }),
    ).toBeVisible();
    const segmentLink = page.locator(`tbody a[href="/segments/${segmentId}"]`);
    await expect(segmentLink).toBeVisible();
    await segmentLink.click();
    await expect(page.getByLabel("Segment efforts table")).toBeVisible();
    await expect(
      page.locator('[aria-label="Segment comparison map"]'),
    ).toBeVisible();
    const raceLink = page.getByRole("link", { name: "Open race viewer" });
    await expect(raceLink).toHaveAttribute(
      "href",
      new RegExp(`/segments/${segmentId}/race\\?efforts=`),
    );
    const selectedEfforts = new URL(
      await raceLink.getAttribute("href"),
      target.url,
    ).searchParams.get("efforts");
    expect(selectedEfforts.split(",").sort()).toEqual(
      raceEffortIds.split(",").sort(),
    );

    await raceLink.click();
    const raceMap = page.locator('[aria-label="Segment race viewer map"]');
    await expect(raceMap).toBeVisible();
    await expect(
      page.getByRole("button", { name: /^Remove .* from race viewer$/ }),
    ).toHaveCount(raceEffortIds.split(",").length);
    const timeline = page.getByLabel("Race playback timeline");
    await page.getByRole("button", { name: "Play race playback" }).click();
    await expect
      .poll(async () => Number(await timeline.inputValue()))
      .toBeGreaterThan(0);
    await page.getByRole("button", { name: "Pause race playback" }).click();
    const canvas = raceMap.locator("canvas");
    await expect(canvas).toBeVisible();
    const beforeSeek = (await canvas.screenshot()).toString("base64");
    await timeline.fill("50");
    await expect(timeline).toHaveValue("50");
    await expect
      .poll(async () => (await canvas.screenshot()).toString("base64"))
      .not.toBe(beforeSeek);

    const back = page.getByRole("link", { name: "Back", exact: true });
    expect(
      new URL(await back.getAttribute("href"), target.url).searchParams.get(
        "efforts",
      ),
    ).toBe(selectedEfforts);
    await back.click();
    await expect(page).toHaveURL(
      new RegExp(`/segments/${segmentId}\\?efforts=`),
    );
    await expect(page.getByLabel("Segment efforts table")).toBeVisible();
  });
}
