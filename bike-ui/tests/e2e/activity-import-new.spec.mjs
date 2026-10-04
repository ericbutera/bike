import { test, expect } from "@playwright/test";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { targets } from "./helpers/targets.mjs";
import { openRoute } from "./helpers/ui.mjs";

const source = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../../../bike-rs/api/tests/fixtures/platform/uploads/activity-imports/synthetic-1109.gpx",
);

test("a new GPX upload enters the manual processing queue", async ({
  browser,
}) => {
  const xml = (await readFile(source, "utf8"))
    .replaceAll("2025-06-01", "2025-06-05")
    .replace("Synthetic northbound A", "Fresh parity upload");

  for (const target of targets) {
    const context = await browser.newContext();
    const page = await context.newPage();
    await openRoute(page, target, "/upload");
    await expect(
      page.getByRole("heading", { name: "Upload raw activity files" }),
    ).toBeVisible();
    await page.getByLabel("Activity file").setInputFiles({
      name: "parity-new.gpx",
      mimeType: "application/gpx+xml",
      buffer: Buffer.from(xml),
    });

    const uploaded = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname === "/api/activity-imports" &&
        response.request().method() === "POST",
    );
    await page
      .getByRole("button", { name: "Upload activity", exact: true })
      .click();
    const response = await uploaded;
    expect(response.status(), `${target.name} new upload`).toBe(202);
    const body = await response.json();
    expect(body).toMatchObject({
      original_filename: "parity-new.gpx",
      processing_stage: "raw_stored",
      mime_type: "application/gpx+xml",
    });
    expect(["pending", "processing"]).toContain(body.status);
    await expect(
      page.getByText("Queued parity-new.gpx for processing."),
    ).toBeVisible();
    const row = page
      .getByTestId("manual-upload-queue-scroll")
      .locator("tbody tr")
      .filter({ hasText: "parity-new.gpx" });
    await expect(row.locator("td").first()).toHaveText("queued");
    await context.close();
  }
});
