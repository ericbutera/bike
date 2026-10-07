import { test, expect } from "./helpers/test.mjs";
import { segmentId, targets } from "./helpers/targets.mjs";
import { openRoute } from "./helpers/ui.mjs";

test("deleting a segment removes its detail and returns to the list", async ({
  browser,
}) => {
  for (const target of targets) {
    const context = await browser.newContext();
    const page = await context.newPage();
    await openRoute(page, target, `/segments/${segmentId}`);
    await expect(
      page.getByRole("heading", { name: "Synthetic northbound segment" }),
    ).toBeVisible();

    page.once("dialog", (dialog) => dialog.accept());
    await page.getByRole("button", { name: "Open segment actions" }).click();
    const deleted = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname === `/api/segments/${segmentId}` &&
        response.request().method() === "DELETE",
    );
    await page.getByRole("button", { name: "Delete segment" }).click();
    expect((await deleted).status(), `${target.name} delete segment`).toBe(204);
    await expect(page).toHaveURL(/\/segments\/?$/);
    expect(
      (
        await page.request.get(
          new URL(
            `/api/segments/${segmentId}`,
            process.env.BIKE_API_URL ?? "http://localhost:3000",
          ).toString(),
        )
      ).status(),
    ).toBe(404);
    await context.close();
  }
});
