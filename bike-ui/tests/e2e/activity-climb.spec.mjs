import { connectedTest as test, expect } from "./helpers/test.mjs";
test.use({ scenario: "climb" });
import { activityId, targets } from "./helpers/targets.mjs";
import { openActivityDetail } from "./helpers/ui.mjs";

test("a sustained climb opens its details and elevation chart", async ({
  createContext,
}) => {
  test.setTimeout(90_000);
  for (const target of targets) {
    const context = await createContext();
    const page = await context.newPage();
    await openActivityDetail(page, target, activityId);
    await page.getByRole("tab", { name: "Climbs (1)", exact: true }).click();

    const climb = page.getByRole("button", { name: "Show climb 1 details" });
    await climb.focus();
    await page.keyboard.press("Enter");
    await expect(climb).toHaveAttribute("aria-pressed", "true");
    const details = page.getByRole("region", {
      name: "Climb 1 details",
      exact: true,
    });
    const heading = details.getByRole("heading", {
      name: "Climb 1",
      exact: true,
    });
    await expect(heading).toBeVisible();
    await expect(details.getByText(/^Activity distance:/)).toBeVisible();
    const headingFontSize = await heading.evaluate((node) =>
      parseFloat(getComputedStyle(node).fontSize),
    );
    const valueFontSizes = await details
      .locator(".grid > div > div:first-child")
      .evaluateAll((nodes) =>
        nodes.map((node) => parseFloat(getComputedStyle(node).fontSize)),
      );
    expect(valueFontSizes).toHaveLength(7);
    expect(Math.max(...valueFontSizes)).toBeLessThan(headingFontSize);
    const profile = page.getByRole("img", {
      name: "Climb 1 elevation profile",
    });
    await expect(profile).toBeVisible();

    const chart = profile.locator(".recharts-surface");
    await expect(chart).toBeVisible();
    for (const width of [1280, 390]) {
      await page.setViewportSize({ width, height: 900 });
      await expect
        .poll(() =>
          profile.evaluate((node) => {
            const curve = node.querySelector(".recharts-area-curve");
            const grid = node.querySelector(
              ".recharts-cartesian-grid-horizontal line",
            );
            return Math.abs(
              curve.getPointAtLength(curve.getTotalLength()).x -
                grid.x2.baseVal.value,
            );
          }),
        )
        .toBeLessThan(1);
      await expect
        .poll(async () => {
          const profileBounds = await profile.boundingBox();
          const chartBounds = await chart.boundingBox();
          return (
            chartBounds.x +
            chartBounds.width -
            profileBounds.x -
            profileBounds.width
          );
        })
        .toBeLessThanOrEqual(0);
    }
    await chart.hover({ position: { x: 160, y: 80 } });
    await expect(profile.locator(".recharts-tooltip-wrapper")).toBeVisible();
    await context.close();
  }
});
