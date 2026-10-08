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
    await expect(page.getByText("1 climb", { exact: true })).toBeVisible();

    const climb = page.getByRole("button", { name: "Show climb 1 details" });
    await climb.focus();
    await page.keyboard.press("Enter");
    await expect(climb).toHaveAttribute("aria-pressed", "true");
    const profile = page.getByRole("img", {
      name: "Climb 1 elevation profile",
    });
    await expect(profile).toBeVisible();

    const chart = profile.locator(".recharts-surface");
    await expect(chart).toBeVisible();
    await chart.hover({ position: { x: 160, y: 80 } });
    await expect(profile.locator(".recharts-tooltip-wrapper")).toBeVisible();
    await context.close();
  }
});
