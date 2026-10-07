import { test, expect } from "./helpers/test.mjs";
import { activityId, targets } from "./helpers/targets.mjs";
import { openActivityDetail } from "./helpers/ui.mjs";

test("activity editor saves and restores the same ride in Bike", async ({
  browser,
}) => {
  test.setTimeout(120_000);

  for (const target of targets) {
    const context = await browser.newContext();
    const page = await context.newPage();
    const updates = [];
    let activityApiUrl;
    let originalTitle;

    page.on("response", (response) => {
      if (
        new URL(response.url()).pathname === `/api/activities/${activityId}`
      ) {
        if (response.request().method() === "GET")
          activityApiUrl = response.url();
        if (response.request().method() === "PATCH")
          updates.push(response.status());
      }
    });

    async function editTitle(title) {
      await page.getByRole("button", { name: "Open activity actions" }).click();
      await page.getByRole("button", { name: "Edit activity" }).click();
      const modal = page.locator(".modal-open");
      await expect(
        modal.getByRole("heading", { name: "Edit activity" }),
      ).toBeVisible();
      await modal.getByRole("textbox", { name: "Title" }).fill(title);
      await modal.getByRole("button", { name: "Save" }).click();
      await expect(modal).toHaveCount(0);
      await expect(
        page.getByRole("heading", { name: title, level: 1 }),
      ).toBeVisible();
    }

    try {
      await openActivityDetail(page, target, activityId);
      originalTitle = await page.getByRole("heading", { level: 1 }).innerText();
      await editTitle("Parity edited ride");
      await page.reload({ waitUntil: "domcontentloaded" });
      await expect(
        page.getByRole("heading", { name: "Parity edited ride", level: 1 }),
      ).toBeVisible();
      await editTitle(originalTitle);
      expect(updates).toEqual([200, 200]);
    } finally {
      if (activityApiUrl && originalTitle) {
        const response = await page.request.patch(activityApiUrl, {
          data: { title: originalTitle, activity_type: "training" },
        });
        expect(response.status()).toBe(200);
      }
      await context.close();
    }
  }
});
