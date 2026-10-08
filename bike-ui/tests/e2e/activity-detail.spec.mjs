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
