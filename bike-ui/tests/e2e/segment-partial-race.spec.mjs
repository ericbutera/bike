import { connectedTest as test, expect } from "./helpers/test.mjs";
test.use({ scenario: "partial-race" });
import { segmentId, targets } from "./helpers/targets.mjs";
import { openRoute } from "./helpers/ui.mjs";

test("race gaps interpolate uneven, partial effort samples in Bike", async ({
  createContext,
}) => {
  for (const target of targets) {
    const context = await createContext({ colorScheme: "light" });
    const page = await context.newPage();
    let comparisonResponse;
    page.on("response", (response) => {
      if (
        new URL(response.url()).pathname ===
        `/api/segments/${segmentId}/comparison`
      ) {
        comparisonResponse = response;
      }
    });

    await openRoute(
      page,
      target,
      `/segments/${segmentId}/race?efforts=5895,5912`,
    );
    await expect(
      page.locator('[aria-label="Segment race viewer map"]'),
    ).toBeVisible();
    await expect.poll(() => comparisonResponse).toBeTruthy();

    const comparison = await comparisonResponse.json();
    const efforts = new Map(
      comparison.efforts.map((effort) => [effort.id, effort]),
    );
    expect(efforts.get(5895)?.route_points).toHaveLength(11);
    expect(efforts.get(5912)?.route_points).toHaveLength(4);

    const timeline = page.getByLabel("Race playback timeline");
    await timeline.fill("15");
    await expect(page.getByText("+3s", { exact: true })).toBeVisible();
    await timeline.fill("75");
    await expect(page.getByText("+15s", { exact: true })).toBeVisible();

    await context.close();
  }
});
