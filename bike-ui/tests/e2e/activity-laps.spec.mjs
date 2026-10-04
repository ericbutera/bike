import { test, expect } from "@playwright/test";
import { activityId, targets } from "./helpers/targets.mjs";
import { openActivityDetail } from "./helpers/ui.mjs";

test("explicit lap rollups render on each activity detail", async ({
  browser,
}) => {
  for (const target of targets) {
    const context = await browser.newContext({ colorScheme: "light" });
    const page = await context.newPage();
    await openActivityDetail(page, target, activityId);

    const laps = page
      .getByRole("heading", { name: "Lap splits" })
      .locator(
        "xpath=ancestor::*[contains(concat(' ', normalize-space(@class), ' '), ' card ')][1]",
      );
    await expect(laps.getByText("2 laps", { exact: true })).toBeVisible();
    await expect(laps.getByRole("heading", { name: "Warmup" })).toBeVisible();
    await expect(laps.getByRole("heading", { name: "Tempo" })).toBeVisible();
    await expect(
      laps.getByText("This upload did not contain explicit lap data."),
    ).toHaveCount(0);
    await context.close();
  }
});
