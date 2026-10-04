import { test, expect } from "@playwright/test";
import { targets } from "./helpers/targets.mjs";
import { openRoute } from "./helpers/ui.mjs";

async function waitForArchiveJob(page, jobId) {
  for (let attempt = 0; attempt < 30; attempt += 1) {
    const jobs = await page.evaluate(async () => {
      const apiUrl = window.__APP_CONFIG__.API_URL.replace(/\/$/, "");
      const response = await fetch(`${apiUrl}/activity-imports/archive-jobs`, {
        credentials: "include",
      });
      if (!response.ok)
        throw new Error(`Archive job list returned ${response.status}`);
      return response.json();
    });
    const job = jobs.find((candidate) => candidate.id === jobId);
    if (job && ["succeeded", "failed"].includes(job.status)) return job;
    await page.waitForTimeout(500);
  }

  throw new Error(`Archive job ${jobId} did not reach a terminal state`);
}

test("archive jobs move from queued to a terminal worker result in Bike", async ({
  browser,
}) => {
  test.setTimeout(60_000);
  for (const target of targets) {
    const context = await browser.newContext();
    const page = await context.newPage();
    await openRoute(page, target, "/upload");
    await expect(
      page.getByRole("heading", { name: "Upload raw activity files" }),
    ).toBeVisible();
    await page
      .getByPlaceholder("https://.../export.zip")
      .fill("https://example.com");

    const queued = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname ===
          "/api/activity-imports/archive-url" &&
        response.request().method() === "POST",
    );
    await page.getByRole("button", { name: "Queue archive import" }).click();
    const response = await queued;
    expect(response.status(), `${target.name} archive queue`).toBe(202);
    const initialJob = await response.json();
    expect(initialJob.status).toBe("queued");

    const finalJob = await waitForArchiveJob(page, initialJob.id);
    expect(finalJob.status, `${target.name} archive worker result`).toBe(
      "failed",
    );
    expect(finalJob.started_at).toBeTruthy();
    expect(finalJob.finished_at).toBeTruthy();
    expect(finalJob.failure_message).toBeTruthy();
    expect(finalJob.total_entries).toBe(0);

    await context.close();
  }
});
