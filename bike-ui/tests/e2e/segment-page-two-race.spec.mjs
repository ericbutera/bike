import { test, expect } from "./helpers/test.mjs";
import { segmentId, targets } from "./helpers/targets.mjs";
import { openRoute } from "./helpers/ui.mjs";

test("a second-page effort reaches the race viewer through its URL", async ({
  browser,
}) => {
  for (const target of targets) {
    const context = await browser.newContext({ colorScheme: "light" });
    const page = await context.newPage();
    await openRoute(page, target, `/segments/${segmentId}`);
    await expect(page.getByText("Showing 1-10 of 11 efforts")).toBeVisible();

    const efforts = page.getByLabel("Segment efforts table");
    await page.getByRole("button", { name: "Next page" }).click();
    await expect(page.getByText("Showing 11-11 of 11 efforts")).toBeVisible();
    await expect(efforts.locator("tbody tr")).toHaveCount(1);
    await efforts
      .getByRole("button", { name: /^Add .* to comparison$/ })
      .click();

    const raceLink = page.getByRole("link", { name: "Open race viewer" });
    await expect
      .poll(async () =>
        new URL(await raceLink.getAttribute("href"), target.url).searchParams
          .get("efforts")
          ?.split(",")
          .includes("6008"),
      )
      .toBe(true);
    await raceLink.click();
    await expect(page).toHaveURL(
      new RegExp(`/segments/${segmentId}/race\\?efforts=`),
    );
    await expect(
      page.getByRole("button", { name: /^Remove .* from race viewer$/ }),
    ).toHaveCount(4);
    await expect(page.getByRole("link", { name: "Back" })).toHaveAttribute(
      "href",
      /6008/,
    );
    await context.close();
  }
});
