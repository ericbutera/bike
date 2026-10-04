import { test, expect } from "@playwright/test";
import { openFrontendRoute } from "./helpers/frontend.mjs";
import { targets } from "./helpers/targets.mjs";

test("report selection fetches only the selected report in Bike", async ({
  browser,
}) => {
  test.setTimeout(90_000);
  const statusesByStack = {};

  for (const target of targets) {
    const context = await browser.newContext({
      viewport: { width: 1440, height: 900 },
    });
    const page = await context.newPage();
    const evidence = await openFrontendRoute(page, target, {
      name: "training-reports",
      path: "/training/reports?range=week",
    });
    const reportResponses = () =>
      evidence.responses.filter(
        (response) =>
          new URL(response.url()).pathname === "/api/training/reports",
      );
    await expect.poll(() => reportResponses().length).toBe(1);
    const initialStatus = reportResponses()[0].status();
    expect([200, 429]).toContain(initialStatus);

    const menu = page.getByRole("navigation", { name: "Report menu" });
    await menu.getByRole("button", { name: /Ride Summary/ }).click();
    await expect(page).toHaveURL(/report=ride_summary/);
    await expect(
      page.getByRole("heading", { name: "Ride Summary" }),
    ).toBeVisible();
    await expect.poll(() => reportResponses().length).toBe(2);
    const statuses = reportResponses().map((response) => response.status());
    expect(statuses).toHaveLength(2);
    expect(statuses.every((status) => status === 200 || status === 429)).toBe(
      true,
    );
    expect(new URL(reportResponses()[1].url()).searchParams.get("report")).toBe(
      "ride_summary",
    );
    statusesByStack[target.name] = statuses;

    await evidence.close();
    await context.close();
  }

  expect(statusesByStack.go).toEqual(statusesByStack.rust);
  expect(statusesByStack.cs).toEqual(statusesByStack.rust);
});
