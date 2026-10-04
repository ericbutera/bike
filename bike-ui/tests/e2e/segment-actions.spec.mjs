import { test, expect } from "@playwright/test";
import { segmentId, targets } from "./helpers/targets.mjs";
import { openRoute } from "./helpers/ui.mjs";

test("segment mode and name edits work through each detail page", async ({
  browser,
}) => {
  for (const target of targets) {
    const context = await browser.newContext();
    const page = await context.newPage();
    await openRoute(page, target, `/segments/${segmentId}`);
    const mode = page.getByLabel("Segment mode");
    await expect(mode).toHaveValue("xc");
    await mode.selectOption("dh");
    await expect(mode).toHaveValue("dh");
    await mode.selectOption("xc");
    await expect(mode).toHaveValue("xc");

    await page.getByRole("button", { name: "Open segment actions" }).click();
    await page.getByRole("button", { name: "Rename", exact: true }).click();
    const name = page.getByRole("textbox", { name: "Segment name" });
    await name.fill("Temporary parity segment");
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expect(
      page.getByRole("heading", { name: "Temporary parity segment" }),
    ).toBeVisible();
    await page.getByRole("button", { name: "Open segment actions" }).click();
    await page.getByRole("button", { name: "Rename", exact: true }).click();
    await name.fill("Synthetic northbound segment");
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expect(
      page.getByRole("heading", { name: "Synthetic northbound segment" }),
    ).toBeVisible();
    await context.close();
  }
});
