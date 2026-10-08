import { connectedTest as test, expect } from "./helpers/test.mjs";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { activityId, targets } from "./helpers/targets.mjs";
import { openActivityDetail } from "./helpers/ui.mjs";

test("activity source download and admin import trace work in Bike", async ({
  createContext,
}) => {
  test.setTimeout(90_000);
  const downloads = [];

  for (const target of targets) {
    const context = await createContext({ acceptDownloads: true });
    const page = await context.newPage();
    await openActivityDetail(page, target, activityId);

    await page.getByRole("button", { name: "Open activity actions" }).click();
    await page.getByRole("button", { name: "View import trace" }).click();
    const trace = page.getByRole("dialog");
    await expect(
      trace.getByRole("heading", { name: "Import trace" }),
    ).toBeVisible();
    await expect(trace.getByText(/Import #\d+/)).toBeVisible();
    await expect(trace.getByRole("heading", { name: "DAG" })).toBeVisible();
    await trace.getByRole("button", { name: "Close", exact: true }).click();
    await expect(trace).toHaveCount(0);

    await page.getByRole("button", { name: "Open activity actions" }).click();
    const downloadPromise = page.waitForEvent("download");
    await page.getByRole("link", { name: "Download original source" }).click();
    const download = await downloadPromise;
    const digest = createHash("sha256")
      .update(await readFile(await download.path()))
      .digest("hex");
    downloads.push({ name: download.suggestedFilename(), digest });
    await context.close();
  }

  const source = await readFile(
    new URL(
      "../../../bike-rs/api/tests/fixtures/platform/uploads/activity-imports/synthetic-1109.gpx",
      import.meta.url,
    ),
  );
  expect(downloads).toEqual(
    targets.map(() => ({
      name: "synthetic-1109.gpx",
      digest: createHash("sha256").update(source).digest("hex"),
    })),
  );
});
