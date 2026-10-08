import { connectedTest as test, expect } from "./helpers/test.mjs";
import {
  openFrontendRoute,
  stabilizePage,
  visualViewport,
} from "./helpers/frontend.mjs";
import { targets } from "./helpers/targets.mjs";

test("segment lists expose the shared structure and interactions", async ({
  createContext,
}) => {
  const requestedTarget = process.env.PLAYWRIGHT_TARGET?.trim();
  const selectedTargets = requestedTarget
    ? targets.filter(({ name }) => name === requestedTarget)
    : targets;

  if (selectedTargets.length === 0) {
    throw new Error(`Unknown PLAYWRIGHT_TARGET=${requestedTarget}`);
  }

  const { viewport } = visualViewport();

  for (const target of selectedTargets) {
    const context = await createContext({
      colorScheme: "light",
      locale: "en-US",
      timezoneId: "America/Detroit",
      reducedMotion: "reduce",
      viewport,
    });
    const page = await context.newPage();
    await openFrontendRoute(
      page,
      target,
      { name: "segments", path: "/segments" },
      { retryAuthRedirect: true, settleMs: 2_500 },
    );
    await expect(
      page.getByRole("heading", { name: "Import Segment" }),
    ).toBeVisible();
    await expect(page.getByRole("heading", { name: "Segments" })).toBeVisible();
    await expect(
      page.getByLabel("Segment route file", { exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Import segment" }),
    ).toBeVisible();
    await expect(page.getByPlaceholder("Search segments")).toBeVisible();
    await expect(page.getByRole("combobox")).toBeVisible();
    await expect(page.getByRole("button", { name: "Search" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Clear" })).toBeVisible();

    const table = page.locator("table");
    await expect(table).toBeVisible();
    if (viewport.width <= 390) {
      expect(
        await page.evaluate(() => document.documentElement.scrollWidth),
      ).toBeLessThanOrEqual(viewport.width);
      const tableScroll = table.locator("xpath=..");
      expect(
        await tableScroll.evaluate(
          (element) => element.scrollWidth > element.clientWidth,
        ),
      ).toBe(true);
      await tableScroll.evaluate((element) => {
        element.scrollLeft = element.scrollWidth;
      });
    }
    for (const header of [
      "Segment",
      "Type",
      "Format",
      "Efforts",
      "Distance",
      "KOM",
      "Your PR",
    ]) {
      await expect(
        table.getByRole("columnheader", { name: header }),
      ).toBeVisible();
    }

    const segmentRows = table.locator("tbody tr");
    await expect(segmentRows.first()).toBeVisible();
    await expect(
      page.locator(
        'button[aria-label^="Star "], button[aria-label^="Unstar "]',
      ),
    ).toHaveCount(await segmentRows.count());
    await expect(page.locator('button[aria-label^="Rename "]')).toHaveCount(
      await segmentRows.count(),
    );

    const firstTitle = (
      await segmentRows.first().getByRole("link").innerText()
    ).trim();
    await page.getByPlaceholder("Search segments").fill(firstTitle);
    await expect(segmentRows).toHaveCount(1);
    await page.getByRole("button", { name: "Clear" }).click();
    await expect(segmentRows.first()).toBeVisible();

    await page.locator('button[aria-label^="Rename "]').first().click();
    await expect(
      page.getByRole("dialog", { name: "Rename segment" }),
    ).toBeVisible();
    await page.getByRole("button", { name: "Cancel" }).click();
    await expect(
      page.getByRole("dialog", { name: "Rename segment" }),
    ).toHaveCount(0);
    await stabilizePage(page);
    await context.close();
  }
});
