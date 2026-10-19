import { connectedTest as test, expect } from "./helpers/test.mjs";
import {
  activityId,
  expectedClimbCount,
  expectZoneDistribution,
  targets,
} from "./helpers/targets.mjs";
import { openActivityDetail } from "./helpers/ui.mjs";

test("activity detail exposes the activity surface in Bike", async ({
  createContext,
}) => {
  for (const target of targets) {
    const context = await createContext({ colorScheme: "light" });
    const page = await context.newPage();
    const activityRequests = [];

    page.on("request", (request) => {
      if (request.url().includes("/api/activities/")) {
        activityRequests.push(request.url());
      }
    });

    await openActivityDetail(page, target, activityId);

    const header = page
      .getByRole("heading", { level: 1 })
      .locator("xpath=ancestor::header[1]");
    const location = header.locator("p > span");
    if (await location.count()) {
      const positions = await header.locator("p").evaluate((row) => {
        const textBounds = (node) => {
          const range = document.createRange();
          range.selectNodeContents(node);
          return range.getBoundingClientRect().toJSON();
        };
        return {
          location: textBounds(row.querySelector("span").lastChild),
          time: textBounds(row.querySelector("time")),
        };
      });
      expect(
        Math.abs(positions.location.y - positions.time.y),
      ).toBeLessThanOrEqual(0.5);
    }

    await expect(
      page.getByText("Relative effort", { exact: true }),
    ).toBeVisible();
    const routeMap = page.getByRole("img", { name: "Activity route map" });
    await expect(routeMap).toBeVisible();
    // The enhanced map uses the owned style; basemap controls have separate coverage.
    await expect(
      routeMap.getByRole("button", { name: "Street", exact: true }),
    ).toHaveCount(0);
    await routeMap.getByRole("button", { name: "Zoom in" }).click();

    const matchedSegment = page
      .getByRole("button", { name: /^Highlight / })
      .first();
    await expect(matchedSegment).toBeVisible();
    await matchedSegment.focus();
    await page.keyboard.press("Enter");
    await expect(matchedSegment).toHaveAttribute("aria-pressed", "true");

    if (expectedClimbCount > 0) {
      await page.getByRole("tab", { name: /^Climbs/ }).click();
      const climb = page
        .getByRole("button", { name: /^Show climb \d+ details$/ })
        .first();
      await climb.focus();
      await page.keyboard.press("Enter");
      await expect(climb).toHaveAttribute("aria-pressed", "true");
      await expect(
        page.getByRole("tab", {
          name: `Climbs (${expectedClimbCount})`,
          exact: true,
        }),
      ).toHaveAttribute("aria-selected", "true");
      await page.getByRole("tab", { name: /^Segments/ }).click();
    } else {
      await expect(page.getByRole("tab", { name: /^Climbs/ })).toHaveCount(0);
    }
    await expect(
      page.getByRole("tabpanel", { name: /^Segments/ }),
    ).toBeVisible();
    await expect(
      page.getByRole("heading", { name: "Zone distribution" }),
    ).toHaveCount(expectZoneDistribution ? 1 : 0);
    await expect(
      page.getByRole("heading", { name: "Ride signals" }),
    ).toBeVisible();
    await expect(
      page.getByRole("region", { name: "Activity summary" }),
    ).toBeVisible();
    await expect(
      page.getByRole("region", { name: "Activity data" }),
    ).toBeVisible();
    await expect(page.getByLabel("Activity data details")).toHaveCount(0);
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
