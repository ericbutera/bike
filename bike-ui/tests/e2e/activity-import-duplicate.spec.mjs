import { test, expect } from "@playwright/test";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { targets } from "./helpers/targets.mjs";
import { openRoute } from "./helpers/ui.mjs";

const fixture = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../../../bike-rs/api/tests/fixtures/platform/uploads/activity-imports/synthetic-1109.gpx",
);

test("uploading an existing GPX shows the existing import in Bike", async ({
  browser,
}) => {
  test.setTimeout(90_000);
  for (const target of targets) {
    const context = await browser.newContext();
    const page = await context.newPage();
    await openRoute(page, target, "/upload");
    await expect(
      page.getByRole("heading", { name: "Upload raw activity files" }),
    ).toBeVisible();

    await page.getByLabel("Activity file").setInputFiles(fixture);
    const uploaded = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname === "/api/activity-imports" &&
        response.request().method() === "POST",
    );
    await page
      .getByRole("button", { name: "Upload activity", exact: true })
      .click();

    const response = await uploaded;
    expect(response.status(), `${target.name} duplicate upload`).toBe(200);
    const body = await response.json();
    expect(body).toMatchObject({
      id: 1,
      status: "processed",
      activity_id: 1109,
    });
    await expect(
      page.getByText("Already had synthetic-1109.gpx in your activity feed."),
    ).toBeVisible();
    await context.close();
  }
});
