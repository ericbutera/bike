import { test, expect } from "@playwright/test";
import {
  activityId,
  expectedClimbCount,
  expectZoneDistribution,
  targets,
} from "./helpers/targets.mjs";
import { openActivityDetail } from "./helpers/ui.mjs";

async function stubMapTiles(page) {
  await page.route(
    /^https:\/\/(?:tiles\.openfreemap\.org|tile\.waymarkedtrails\.org|server\.arcgisonline\.com)\//,
    async (route) => {
      if (route.request().url().includes("/styles/")) {
        await route.fulfill({
          contentType: "application/json",
          headers: { "access-control-allow-origin": "*" },
          body: JSON.stringify({ version: 8, sources: {}, layers: [] }),
        });
        return;
      }

      await route.fulfill({
        contentType: "image/png",
        headers: { "access-control-allow-origin": "*" },
        body: Buffer.from(
          "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScL/nwAAAABJRU5ErkJggg==",
          "base64",
        ),
      });
    },
  );
}

test("activity detail exposes the activity surface in Bike", async ({
  browser,
}) => {
  for (const target of targets) {
    const context = await browser.newContext({ colorScheme: "light" });
    const page = await context.newPage();
    const activityRequests = [];
    await stubMapTiles(page);

    page.on("request", (request) => {
      if (request.url().includes("/api/activities/")) {
        activityRequests.push(request.url());
      }
    });

    await openActivityDetail(page, target, activityId);

    await expect(
      page.getByText("Relative effort", { exact: true }),
    ).toBeVisible();
    const routeMap = page.getByRole("img", { name: "Activity route map" });
    await expect(routeMap).toBeVisible();
    const routeMapControls = routeMap.locator("xpath=..");
    const streetLayer = routeMapControls.getByRole("button", {
      name: "Street",
      exact: true,
    });
    await streetLayer.click();
    await expect(streetLayer).toHaveAttribute("aria-pressed", "true");
    const topoLayer = routeMapControls.getByRole("button", {
      name: "Topo",
      exact: true,
    });
    await topoLayer.click();
    await expect(topoLayer).toHaveAttribute("aria-pressed", "true");
    await routeMapControls.getByRole("button", { name: "Zoom in" }).click();

    const matchedSegment = page
      .getByRole("button", { name: /^Jump to .* matches$/ })
      .first();
    await expect(matchedSegment).toBeVisible();
    await matchedSegment.focus();
    await page.keyboard.press("Enter");
    await expect(matchedSegment).toHaveAttribute("aria-pressed", "true");

    if (expectedClimbCount > 0) {
      const climb = page
        .getByRole("button", { name: /^Show climb \d+ details$/ })
        .first();
      await climb.focus();
      await page.keyboard.press("Enter");
      await expect(climb).toHaveAttribute("aria-pressed", "true");
    }
    await expect(page.getByRole("heading", { name: "Climbs" })).toBeVisible();
    await expect(
      page.getByText(
        `${expectedClimbCount} climb${expectedClimbCount === 1 ? "" : "s"}`,
        { exact: true },
      ),
    ).toBeVisible();
    if (expectedClimbCount === 0) {
      await expect(page.getByText("No sustained climbs found.")).toBeVisible();
    }
    await expect(
      page.getByRole("heading", { name: "Matched segments" }),
    ).toBeVisible();
    await expect(
      page.getByRole("heading", { name: "Zone distribution" }),
    ).toHaveCount(expectZoneDistribution ? 1 : 0);
    await expect(
      page.getByRole("heading", { name: "Ride signals" }),
    ).toBeVisible();
    await expect(page.getByLabel("Activity data details")).toBeVisible();
    await expect(
      page.getByText("Training profile", { exact: true }),
    ).toHaveCount(0);

    expect(
      activityRequests.some((url) => /\/activities\/0(?:\/|$)/.test(url)),
      `${target.name} requested an invalid activity id: ${activityRequests.join(", ")}`,
    ).toBe(false);

    await context.close();
  }
});
