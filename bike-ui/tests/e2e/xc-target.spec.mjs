import { test, expect } from "./helpers/test.mjs";
import { targets } from "./helpers/targets.mjs";
import { openRoute } from "./helpers/ui.mjs";

test("XC event target saves and clears through the shared UI", async ({
  browser,
}) => {
  test.setTimeout(120_000);
  for (const target of targets) {
    const context = await browser.newContext();
    const page = await context.newPage();
    await openRoute(page, target, "/xc");
    await expect(
      page.getByRole("heading", { name: "Event target" }),
    ).toBeVisible();
    await page.getByRole("button", { name: "Set target" }).click();
    await page
      .getByRole("textbox", { name: "Event name" })
      .fill("Parity XC target");
    await page.getByLabel("Training start").fill("2026-09-01");
    await page.getByLabel("Target date").fill("2026-12-31");
    await page.getByRole("spinbutton", { name: "Distance target" }).fill("20");
    await page
      .getByRole("spinbutton", { name: "Climbing target" })
      .fill("1000");

    const saved = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname === "/api/preferences" &&
        response.request().method() === "PUT",
    );
    await page.getByRole("button", { name: "Save target" }).click();
    expect((await saved).status(), `${target.name} save target`).toBe(200);
    await expect(
      page.getByRole("button", { name: "Edit target" }),
    ).toBeVisible();

    await page.getByRole("button", { name: "Edit target" }).click();
    const cleared = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname === "/api/preferences" &&
        response.request().method() === "PUT",
    );
    await page.getByRole("button", { name: "Clear target" }).click();
    expect((await cleared).status(), `${target.name} clear target`).toBe(200);
    await expect(
      page.getByRole("button", { name: "Set target" }),
    ).toBeVisible();
    await context.close();
  }
});
