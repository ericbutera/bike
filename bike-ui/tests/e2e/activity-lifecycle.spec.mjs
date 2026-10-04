import { test, expect } from "@playwright/test";
import { activityId, targets } from "./helpers/targets.mjs";
import { openActivityDetail } from "./helpers/ui.mjs";

const apiUrls = {
  rust: process.env.BIKE_API_URL ?? "http://localhost:3030",
};

test("activity regeneration and deletion work through the shared actions", async ({
  browser,
}) => {
  test.setTimeout(120_000);
  for (const target of targets) {
    const context = await browser.newContext();
    const page = await context.newPage();
    await openActivityDetail(page, target, activityId);

    await page.getByRole("button", { name: "Open activity actions" }).click();
    const regenerated = page.waitForResponse((response) =>
      response.url().endsWith(`/api/activities/${activityId}/regenerate`),
    );
    await page.getByRole("button", { name: "Regenerate derived data" }).click();
    expect((await regenerated).status(), `${target.name} regeneration`).toBe(
      200,
    );
    await page.reload({ waitUntil: "domcontentloaded" });
    await expect(
      page.locator('[aria-label="Activity route map"]'),
    ).toBeVisible();

    const detail = await page.request.get(
      `${apiUrls[target.name]}/api/activities/${activityId}`,
    );
    expect(detail.status()).toBe(200);
    const activity = await detail.json();
    expect(activity.distance_meters).toBeGreaterThan(1_100);
    expect(activity.training_analysis?.ride_focus).toBe("other");

    page.once("dialog", (dialog) => dialog.accept());
    await page.getByRole("button", { name: "Open activity actions" }).click();
    const deleted = page.waitForResponse(
      (response) =>
        response.url().endsWith(`/api/activities/${activityId}`) &&
        response.request().method() === "DELETE",
    );
    await page.getByRole("button", { name: "Delete activity" }).click();
    expect((await deleted).status(), `${target.name} deletion`).toBe(204);
    await expect(page).toHaveURL(target.url.replace(/\/$/, "") + "/");
    expect(
      (
        await page.request.get(
          `${apiUrls[target.name]}/api/activities/${activityId}`,
        )
      ).status(),
    ).toBe(404);
    await context.close();
  }
});
