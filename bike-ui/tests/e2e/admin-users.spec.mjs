import { test, expect } from "@playwright/test";
import { openFrontendRoute } from "./helpers/frontend.mjs";
import { targets } from "./helpers/targets.mjs";

const apiUrls = {
  rust: process.env.BIKE_API_URL ?? "http://localhost:3030",
};

test("admin users filter and account status match in Bike", async ({
  browser,
}) => {
  for (const target of targets) {
    const context = await browser.newContext({
      colorScheme: "light",
      locale: "en-US",
      timezoneId: "America/Detroit",
      reducedMotion: "reduce",
    });
    const page = await context.newPage();
    await openFrontendRoute(
      page,
      target,
      { name: "admin-users", path: "/admin/users" },
      { retryAuthRedirect: true, settleMs: 1_500 },
    );

    const table = page.locator("table");
    try {
      const statusFilter = page.locator("select").first();
      await expect(statusFilter).toBeVisible();
      await statusFilter.selectOption("true");
      await page.getByRole("button", { name: "Search" }).click();
      await expect(table.locator("tbody tr")).toHaveCount(1);
      await expect(table.locator("tbody tr").first()).toContainText(
        "disabled-rider@bike.local",
      );

      await page.getByRole("button", { name: "Clear" }).click();
      await expect(table.locator("tbody tr")).toHaveCount(3);

      await table
        .locator("tbody tr")
        .filter({ hasText: "active-rider@bike.local" })
        .click();
      const dialog = page.getByRole("dialog");
      await expect(dialog).toBeVisible();
      await dialog.getByRole("button", { name: "Disable Account" }).click();
      await expect(
        dialog.getByRole("button", { name: "Account Disabled" }),
      ).toBeVisible();
    } finally {
      await page.evaluate(async (apiUrl) => {
        const response = await fetch(`${apiUrl}/api/admin/users/2/disable`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ disabled: false }),
        });
        if (!response.ok) {
          throw new Error(
            `Failed to restore active fixture user: ${response.status}`,
          );
        }
      }, apiUrls[target.name]);
      await context.close();
    }
  }
});
