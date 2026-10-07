import { test, expect } from "./helpers/test.mjs";
import { openFrontendRoute } from "./helpers/frontend.mjs";
import { targets } from "./helpers/targets.mjs";

test("admin tasks expose the same filters, timing, and detail in Bike", async ({
  browser,
}) => {
  test.skip(
    process.env.BIKE_TEST_ADMIN_TASK_FIXTURE !== "true",
    "Requires the pinned synthetic task fixture",
  );

  for (const target of targets) {
    const context = await browser.newContext({
      colorScheme: "light",
      locale: "en-US",
      timezoneId: "America/Detroit",
      reducedMotion: "reduce",
    });
    const page = await context.newPage();
    const observation = await openFrontendRoute(
      page,
      target,
      { name: "admin-tasks", path: "/admin/tasks" },
      { retryAuthRedirect: true, settleMs: 1_500 },
    );

    await expect(
      page.getByRole("heading", { name: "Background Tasks" }),
    ).toBeVisible();
    const table = page.locator("table");
    for (const header of [
      "ID",
      "Type",
      "Status",
      "Duration",
      "Trend",
      "Attempts",
      "Error",
      "Created",
    ]) {
      await expect(
        table.getByRole("columnheader", { name: header }),
      ).toBeVisible();
    }
    await expect(table.locator("tbody tr")).toHaveCount(5);
    await expect(table.getByText("Slower")).toBeVisible();
    await table
      .locator("tbody tr")
      .filter({ hasText: "regenerate_segment_efforts" })
      .first()
      .click();
    const detail = page.getByRole("dialog");
    await expect(detail).toBeVisible();
    await expect(detail).toContainText("Task #3");
    await expect(detail.locator("pre")).toContainText("segment_id");
    await detail.getByRole("button", { name: "Close" }).click();

    await page.locator("select").nth(1).selectOption("completed");
    await page.getByRole("button", { name: "Search" }).click();
    await expect(table.locator("tbody tr")).toHaveCount(3);
    await page.getByRole("button", { name: "Clear" }).click();
    await expect(table.locator("tbody tr")).toHaveCount(5);

    await page
      .locator("select")
      .first()
      .selectOption("regenerate_segment_efforts");
    await page.getByRole("button", { name: "Search" }).click();
    await expect(table.locator("tbody tr")).toHaveCount(3);
    await page.getByRole("button", { name: "Clear" }).click();

    await page.locator('input[type="date"]').first().fill("2025-06-05");
    await page.locator('input[type="date"]').last().fill("2025-06-06");
    await page.getByRole("button", { name: "Search" }).click();
    await expect(table.locator("tbody tr")).toHaveCount(1);
    await expect(table.locator("tbody tr")).toContainText(
      "regenerate_segment_efforts",
    );
    await page.getByRole("button", { name: "Clear" }).click();

    await page.getByPlaceholder("Filter error text").fill("synthetic");
    await page.getByRole("button", { name: "Search" }).click();
    await expect(table.locator("tbody tr")).toHaveCount(1);
    await expect(table.locator("tbody tr")).toContainText(
      "synthetic worker failure",
    );

    await observation.close();
    await context.close();
  }
});
