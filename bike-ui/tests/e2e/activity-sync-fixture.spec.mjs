import { test, expect } from "./helpers/test.mjs";
import { fakeProductApi, ride } from "./helpers/product-fixtures.mjs";
import { targets } from "./helpers/targets.mjs";

for (const target of targets) {
  test(`${target.name}: synced ride fixture appears in list and detail independently of Strava`, async ({
    page,
  }) => {
    const state = await fakeProductApi(page, target);
    await page.goto(target.url);
    const card = page.locator("article");
    await expect(card).toHaveCount(1);
    await expect(card.getByRole("link", { name: ride.title })).toBeVisible();
    await expect(card.getByText("0.1 km", { exact: true })).toBeVisible();
    await card.getByRole("link", { name: ride.title }).click();
    await expect(page.getByRole("heading", { name: ride.title })).toBeVisible();
    await expect(
      page.getByRole("img", { name: "Activity route map" }),
    ).toBeVisible();
    expect(state.requests).toContain("/activities");
    expect(state.requests).toContain(`/activities/${ride.id}`);
    expect(state.unexpected).toEqual([]);
  });
}
