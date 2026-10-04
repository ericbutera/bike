import { test, expect } from "@playwright/test";
import path from "node:path";
import {
  artifactRoot,
  captureElement,
  compareScreenshots,
  openActivityList,
  selectTheme,
  stabilize,
} from "./helpers/ui.mjs";
import { targets } from "./helpers/targets.mjs";

test("activity list renders and light theme persistence works in Bike", async ({
  browser,
}) => {
  for (const target of targets) {
    const context = await browser.newContext({ colorScheme: "light" });
    const page = await context.newPage();

    await openActivityList(page, target);
    await selectTheme(page, "light");
    await page.reload({ waitUntil: "domcontentloaded" });
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light");

    await context.close();
  }
});

test("activity cards are visually comparable with Rust as the baseline", async ({
  browser,
}) => {
  const screenshots = [];

  for (const target of targets) {
    const context = await browser.newContext({ colorScheme: "light" });
    const page = await context.newPage();
    await openActivityList(page, target);
    await selectTheme(page, "light");
    await openActivityList(page, target);
    const card = page.locator("article").first();
    if ((await card.count()) === 0) {
      throw new Error(
        `${target.name} has no activity cards. Seed the same activity fixture in all three databases before running the visual card comparison.`,
      );
    }
    await stabilize(page);
    screenshots.push({
      target,
      path: await captureElement(card, target.name, "activity-card"),
    });
    await context.close();
  }

  const baseline = screenshots[0];
  for (const candidate of screenshots.slice(1)) {
    const diffPath = path.join(
      artifactRoot,
      "diffs",
      `${candidate.target.name}-vs-rust.png`,
    );
    const result = await compareScreenshots(
      baseline.path,
      candidate.path,
      diffPath,
    );
    const ratio = result.differentPixels / result.totalPixels;
    const allowedRatio = Number(
      process.env.PLAYWRIGHT_ALLOWED_DIFF_RATIO ?? "0.005",
    );

    expect(
      ratio,
      `${candidate.target.name} differs from Rust: ${result.reason ?? `${result.differentPixels} pixels`} (diff ${ratio.toFixed(4)}, allowed ${allowedRatio})`,
    ).toBeLessThanOrEqual(allowedRatio);
  }
});
