import { test, expect } from "@playwright/test";
import { targets } from "./helpers/targets.mjs";
import { openRoute } from "./helpers/ui.mjs";

test("the shared UI queues a new remote archive in Bike", async ({
  browser,
}) => {
  for (const target of targets) {
    const context = await browser.newContext();
    const page = await context.newPage();
    await openRoute(page, target, "/upload");
    await expect(
      page.getByRole("heading", { name: "Upload raw activity files" }),
    ).toBeVisible();
    await page
      .getByPlaceholder("https://.../export.zip")
      .fill("https://example.com/exports/parity.zip");

    const queued = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname ===
          "/api/activity-imports/archive-url" &&
        response.request().method() === "POST",
    );
    await page.getByRole("button", { name: "Queue archive import" }).click();
    const response = await queued;
    expect(response.status(), `${target.name} archive queue`).toBe(202);
    expect(await response.json()).toMatchObject({
      archive_url: "https://example.com/exports/parity.zip",
      status: "queued",
      total_entries: 0,
      imported_count: 0,
    });

    await context.close();
  }
});
