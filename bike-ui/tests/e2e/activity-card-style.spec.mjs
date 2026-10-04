import { test } from "@playwright/test";
import { openActivityList, selectTheme } from "./helpers/ui.mjs";
import { targets } from "./helpers/targets.mjs";

test("inspect light activity-card rendering styles", async ({ browser }) => {
  const selectedTarget = process.env.PLAYWRIGHT_TARGET;
  const selectedTargets = selectedTarget
    ? targets.filter(({ name }) => name === selectedTarget)
    : targets;

  if (selectedTargets.length === 0) {
    throw new Error(`Unknown PLAYWRIGHT_TARGET: ${selectedTarget}`);
  }

  for (const target of selectedTargets) {
    const context = await browser.newContext({ colorScheme: "light" });
    const page = await context.newPage();
    await openActivityList(page, target);
    await selectTheme(page, "light");

    const styles = await page
      .locator("article")
      .first()
      .evaluate((card) => {
        const title = card.querySelector("h3");
        const metricLabel = card.querySelector(".stat-title");
        const metricValue = card.querySelector(".stat-value");
        const chip = card.querySelector("span.inline-flex");
        const image = card.querySelector("img");
        const computed = (element) => {
          if (!element) return null;
          const style = getComputedStyle(element);
          return {
            fontFamily: style.fontFamily,
            fontSize: style.fontSize,
            fontWeight: style.fontWeight,
            lineHeight: style.lineHeight,
            letterSpacing: style.letterSpacing,
          };
        };

        return {
          documentFonts: document.fonts.status,
          interLoaded: document.fonts.check('16px "Inter"'),
          title: computed(title),
          metricLabel: computed(metricLabel),
          metricValue: computed(metricValue),
          chip: computed(chip),
          image: image
            ? { complete: image.complete, naturalWidth: image.naturalWidth }
            : null,
        };
      });

    console.log(`${target.name} ${JSON.stringify(styles)}`);
    await context.close();
  }
});
