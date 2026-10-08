import { connectedTest as test, expect } from "./helpers/test.mjs";
import { openFrontendRoute } from "./helpers/frontend.mjs";
import { targets } from "./helpers/targets.mjs";

test("admin users filter and account status match in Bike", async ({
  createContext,
}) => {
  for (const target of targets) {
    const context = await createContext({
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
    await context.close();
  }
});
