import { connectedTest as test, expect } from "./helpers/test.mjs";
import { openRoute } from "./helpers/ui.mjs";
import { targets } from "./helpers/targets.mjs";

test.use({ scenario: "upload-result" });

test("a persisted processed import links to its visible activity", async ({
  page,
}) => {
  const response = await page.request.get(
    `${process.env.BIKE_API_URL}/activity-imports`,
  );
  expect(response.status()).toBe(200);
  const processed = (await response.json()).find(
    (candidate) => candidate.id === 3,
  );
  expect(processed).toMatchObject({
    status: "processed",
    processing_stage: "complete",
    activity_id: 1111,
  });
  await openRoute(page, targets[0]);
  const activity = page.locator(
    `article h3 a[href="/activities/${processed.activity_id}"]`,
  );
  await expect(activity).toBeVisible();
  await activity.click();
  await expect(
    page.getByRole("heading", {
      name: "Processed upload journey",
      exact: true,
    }),
  ).toBeVisible();
});
