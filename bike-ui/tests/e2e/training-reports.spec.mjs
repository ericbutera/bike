import { connectedTest as test, expect } from "./helpers/test.mjs";
import { openFrontendRoute } from "./helpers/frontend.mjs";
import { targets } from "./helpers/targets.mjs";

test("report selection fetches only the selected report in Bike", async ({
  createContext,
}) => {
  test.setTimeout(90_000);

  for (const target of targets) {
    const context = await createContext({
      viewport: { width: 1440, height: 900 },
    });
    const page = await context.newPage();
    const evidence = await openFrontendRoute(page, target, {
      name: "training-reports",
      path: "/training/reports?range=week",
    });
    const reportResponses = () =>
      evidence.responses.filter(
        (response) =>
          new URL(response.url()).pathname === "/api/training/reports",
      );
    await expect.poll(() => reportResponses().length).toBe(1);
    expect(reportResponses()[0].status()).toBe(200);

    const menu = page.getByRole("navigation", { name: "Report menu" });
    await menu.getByRole("button", { name: /Ride Summary/ }).click();
    await expect(page).toHaveURL(/report=ride_summary/);
    await expect(
      page.getByRole("heading", { name: "Ride Summary" }),
    ).toBeVisible();
    await expect.poll(() => reportResponses().length).toBe(2);
    const statuses = reportResponses().map((response) => response.status());
    expect(statuses).toEqual([200, 200]);
    expect(new URL(reportResponses()[1].url()).searchParams.get("report")).toBe(
      "ride_summary",
    );

    await evidence.close();
    await context.close();
  }
});
