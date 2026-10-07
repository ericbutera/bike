import { test, expect } from "./helpers/test.mjs";
import { targets } from "./helpers/targets.mjs";
import { openRoute } from "./helpers/ui.mjs";

test("archive import states and queue response match in Bike", async ({
  browser,
}) => {
  for (const target of targets) {
    const context = await browser.newContext();
    const page = await context.newPage();
    let jobsResponse;
    page.on("response", (response) => {
      if (
        new URL(response.url()).pathname ===
        "/api/activity-imports/archive-jobs"
      ) {
        jobsResponse = response;
      }
    });

    await openRoute(page, target, "/upload");
    await expect(
      page.getByRole("heading", { name: "Upload raw activity files" }),
    ).toBeVisible();
    await expect.poll(() => jobsResponse).toBeTruthy();
    expect(await jobsResponse.json()).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ id: 701, status: "queued" }),
        expect.objectContaining({
          id: 702,
          status: "running",
          imported_count: 3,
        }),
        expect.objectContaining({
          id: 703,
          status: "succeeded",
          imported_count: 6,
        }),
        expect.objectContaining({
          id: 704,
          status: "failed",
          failure_message: "Archive download returned HTTP 404",
        }),
      ]),
    );

    const archiveList = page
      .locator("details")
      .filter({ hasText: "Recent archive imports" })
      .locator(".collapse-content");
    await page.getByText("Recent archive imports", { exact: true }).click();
    for (const [status, url] of [
      ["queued", "https://example.invalid/exports/queued.zip"],
      ["running", "https://cdn.example.invalid/exports/running.zip"],
      ["succeeded", "https://cdn.example.invalid/exports/succeeded.zip"],
      ["failed", "https://example.invalid/exports/failed.zip"],
    ]) {
      const card = archiveList
        .getByText(url, { exact: true })
        .locator("xpath=../../..");
      await expect(card.getByText(status, { exact: true })).toBeVisible();
    }
    await expect(
      archiveList.getByText("Archive download returned HTTP 404"),
    ).toBeVisible();
    const succeededCard = archiveList
      .getByText("https://cdn.example.invalid/exports/succeeded.zip", {
        exact: true,
      })
      .locator("xpath=../../..");
    await expect(succeededCard.getByText("6", { exact: true })).toBeVisible();

    await context.close();
  }
});
