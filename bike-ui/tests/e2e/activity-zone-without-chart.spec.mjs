import { test, expect } from "@playwright/test";
import { activityId, targets } from "./helpers/targets.mjs";
import { openActivityDetail } from "./helpers/ui.mjs";

test("stored heart-rate zones render without signal chart samples", async ({
  browser,
}) => {
  for (const target of targets) {
    const context = await browser.newContext();
    const page = await context.newPage();
    await openActivityDetail(page, target, activityId);
    await expect(
      page.getByRole("heading", { name: "Zone distribution" }),
    ).toBeVisible();
    await expect(page.getByText("25%", { exact: true })).toBeVisible();
    await expect(page.getByText("75%", { exact: true })).toBeVisible();
    await expect(
      page.getByRole("heading", { name: "Ride signals" }),
    ).toBeVisible();
    await expect(
      page.getByRole("img", { name: "Activity signals chart" }),
    ).toHaveCount(0);
    await expect(
      page.getByText("Turn on at least one signal layer to render the chart."),
    ).toBeVisible();
    await context.close();
  }
});
