import { test, expect } from "@playwright/test";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { targets } from "./helpers/targets.mjs";
import { openFrontendRoute } from "./helpers/frontend.mjs";

const source = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../../../bike-rs/api/tests/fixtures/platform/uploads/segments/synthetic-segment.gpx",
);

test("segment route-file import creates the same manual segment in Bike", async ({
  browser,
}) => {
  const gpx = await readFile(source);

  for (const target of targets) {
    const context = await browser.newContext();
    const page = await context.newPage();
    let apiBase;
    let segmentId;

    try {
      await openFrontendRoute(
        page,
        target,
        { name: "segments", path: "/segments" },
        { retryAuthRedirect: true },
      );
      await expect(
        page.getByRole("heading", { name: "Import Segment" }),
      ).toBeVisible();
      apiBase = await page.evaluate(() =>
        window.__APP_CONFIG__.API_URL.replace(/\/$/, ""),
      );
      await page
        .getByLabel("Segment route file", { exact: true })
        .setInputFiles({
          name: "synthetic-segment.gpx",
          mimeType: "application/gpx+xml",
          buffer: gpx,
        });

      const imported = page.waitForResponse(
        (response) =>
          new URL(response.url()).pathname === "/api/segments" &&
          response.request().method() === "POST",
      );
      await page.getByRole("button", { name: "Import segment" }).click();
      const response = await imported;
      expect(response.status(), `${target.name} segment import`).toBe(201);
      const body = await response.json();
      expect(body).toMatchObject({
        source: "manual_segment_import",
        original_filename: "synthetic-segment.gpx",
        format: "gpx",
      });
      expect(body.title).toEqual(expect.any(String));
      expect(body.title.length).toBeGreaterThan(0);
      expect(body.distance_meters).toBeGreaterThan(0);
      expect(body.processing_task_id).toEqual(expect.any(String));
      segmentId = body.id;

      await expect(
        page.getByText(
          `Imported ${body.title}. Segment matching queued as task ${body.processing_task_id}.`,
        ),
      ).toBeVisible();
      await expect(page.getByRole("link", { name: body.title })).toBeVisible();
    } finally {
      if (apiBase && segmentId) {
        await page.evaluate(
          async ({ apiBase: base, id }) => {
            const response = await fetch(`${base}/segments/${id}`, {
              method: "DELETE",
              credentials: "include",
            });
            if (response.status !== 204 && response.status !== 404) {
              throw new Error(
                `Failed to restore imported segment fixture: ${response.status}`,
              );
            }
          },
          { apiBase, id: segmentId },
        );
      }
      await context.close();
    }
  }
});
