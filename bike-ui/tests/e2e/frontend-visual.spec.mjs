import { test, expect } from "./helpers/test.mjs";
import { selectedFrontendRoutes } from "./helpers/frontend-routes.mjs";
import {
  dynamicVisualMasks,
  openFrontendRoute,
  stabilizePage,
  visualViewport,
  visualThemes,
  waitForVisualReady,
} from "./helpers/frontend.mjs";
import { snapshotSet, targets } from "./helpers/targets.mjs";

test("Rust owns the canonical frontend screenshots", async ({ browser }) => {
  test.setTimeout(10 * 60 * 1000);
  test.skip(
    process.env.PLAYWRIGHT_VISUAL !== "1",
    "Set PLAYWRIGHT_VISUAL=1 to run or update the canonical screenshot set.",
  );

  const rust = targets.find(({ name }) => name === "rust");
  const { viewport, suffix } = visualViewport();
  for (const route of selectedFrontendRoutes()) {
    for (const theme of visualThemes()) {
      const context = await browser.newContext({
        colorScheme: theme,
        locale: "en-US",
        timezoneId: "America/Detroit",
        reducedMotion: "reduce",
        viewport,
      });
      const page = await context.newPage();
      await openFrontendRoute(page, rust, route, {
        retryAuthRedirect: true,
        settleMs: 2_500,
      });
      await waitForVisualReady(page, route.name);
      const accountButton = page.getByRole("button", { name: "Account" });
      if ((await accountButton.count()) > 0) {
        await accountButton.click();
      }
      const controller = page.locator("input.theme-controller");
      if (
        (await controller.count()) > 0 &&
        (await controller.isChecked()) !== (theme === "dark")
      ) {
        await controller.click();
      } else if ((await controller.count()) === 0) {
        await page.locator("html").evaluate((element, selectedTheme) => {
          element.setAttribute("data-theme", selectedTheme);
        }, theme);
      }
      await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
      await page.keyboard.press("Escape");
      await stabilizePage(page);
      await expect(page).toHaveScreenshot(
        `${snapshotSet}/${route.name}-${theme}${suffix}.png`,
        {
          fullPage: true,
          animations: "disabled",
          caret: "hide",
          mask: dynamicVisualMasks(page),
          maxDiffPixels: 5_000,
          maxDiffPixelRatio: 0.001,
        },
      );
      await context.close();
    }
  }
});
