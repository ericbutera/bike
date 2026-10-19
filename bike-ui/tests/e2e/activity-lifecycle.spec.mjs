import { connectedTest as test, expect } from "./helpers/test.mjs";
import { activityId, targets } from "./helpers/targets.mjs";
import { openActivityDetail } from "./helpers/ui.mjs";

const apiUrls = {
  rust: (process.env.BIKE_API_URL ?? "http://localhost:3000/api").replace(
    /\/api\/?$/,
    "",
  ),
};

test("activity regeneration API is accepted and menu deletion removes the activity", async ({
  createContext,
}) => {
  test.setTimeout(120_000);
  for (const target of targets) {
    const context = await createContext();
    const page = await context.newPage();
    await openActivityDetail(page, target, activityId);
    const original = await page.request.get(
      `${apiUrls[target.name]}/api/activities/${activityId}`,
    );
    expect(original.status()).toBe(200);
    const savedActivity = await original.json();

    const regeneration = await page.request.post(
      `${apiUrls[target.name]}/api/activities/${activityId}/regenerate`,
    );
    expect(regeneration.status(), `${target.name} regeneration`).toBe(202);
    expect(await regeneration.json()).toMatchObject({
      id: expect.stringMatching(/^bike-reprocess_activity_import-/),
      status: "running",
    });
    await page.reload({ waitUntil: "domcontentloaded" });
    await expect(
      page.locator('[aria-label="Activity route map"]'),
    ).toBeVisible();

    const detail = await page.request.get(
      `${apiUrls[target.name]}/api/activities/${activityId}`,
    );
    expect(detail.status()).toBe(200);
    const activity = await detail.json();
    expect(activity.distance_meters).toBe(savedActivity.distance_meters);
    expect(activity.training_analysis).toEqual(savedActivity.training_analysis);

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
