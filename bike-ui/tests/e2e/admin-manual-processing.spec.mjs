import { test, expect } from "./helpers/test.mjs";
import { targets } from "./helpers/targets.mjs";
import { openRoute } from "./helpers/ui.mjs";

function waitForMutation(page, path) {
  return page.waitForResponse(
    (response) =>
      new URL(response.url()).pathname === path &&
      response.request().method() === "POST",
  );
}

function sectionWithHeading(page, name) {
  return page.locator("section").filter({
    has: page.getByRole("heading", { name }),
  });
}

test("shared admin tools queue matching import and training work in Bike", async ({
  browser,
}) => {
  test.setTimeout(60_000);

  for (const target of targets) {
    const context = await browser.newContext();
    const page = await context.newPage();
    await openRoute(page, target, "/admin/manual-tasks");
    await expect(
      page.getByRole("heading", { name: "Prewarm analytics caches" }),
    ).toBeVisible();

    const backfillSection = sectionWithHeading(
      page,
      "Prewarm analytics caches",
    );
    const backfillResponse = waitForMutation(
      page,
      "/api/admin/analytics/backfill",
    );
    await backfillSection
      .getByRole("button", { name: "Queue analytics backfill" })
      .click();
    const backfill = await backfillResponse;
    expect(backfill.status(), target.name + " analytics backfill").toBe(202);
    const backfillBody = await backfill.json();
    expect(backfillBody.total_tasks_enqueued).toBeGreaterThan(0);
    expect(backfillBody.total_tasks_enqueued).toBe(
      backfillBody.fitness_task_count + backfillBody.segment_task_count,
    );
    await expect(page.getByText("Total tasks", { exact: true })).toBeVisible();

    const effortSection = sectionWithHeading(
      page,
      "Regenerate segment efforts",
    );
    await effortSection
      .getByRole("spinbutton", { name: /Segment id/ })
      .fill("5");
    const effortResponse = waitForMutation(
      page,
      "/api/admin/segments/regenerate-efforts",
    );
    await effortSection
      .getByRole("button", { name: "Regenerate segment efforts" })
      .click();
    const effort = await effortResponse;
    expect(effort.status(), target.name + " segment effort regeneration").toBe(
      202,
    );
    expect(await effort.json()).toMatchObject({
      segment_id: 5,
      status: "queued",
      message: "Segment effort regeneration queued.",
      task_status: "pending",
    });
    await expect(effortSection).toContainText(
      "Segment effort regeneration queued.",
    );

    const xcSection = sectionWithHeading(page, "Backfill XC training history");
    await xcSection.getByRole("spinbutton", { name: /User id/ }).fill("91003");
    const xcResponse = waitForMutation(page, "/api/admin/training/xc-backfill");
    await xcSection.getByRole("button", { name: "Queue XC backfill" }).click();
    const xc = await xcResponse;
    expect(xc.status(), target.name + " XC backfill").toBe(202);
    expect(await xc.json()).toMatchObject({
      user_id: 91003,
      status: "queued",
      message:
        "XC training backfill queued. Historical rides will repopulate in the background.",
    });
    await expect(xcSection).toContainText("XC training backfill queued");

    const segmentsSection = sectionWithHeading(
      page,
      "Regenerate user segments",
    );
    await segmentsSection
      .getByRole("spinbutton", { name: /User id/ })
      .fill("91002");
    const segmentsResponse = waitForMutation(
      page,
      "/api/admin/segments/regenerate",
    );
    await segmentsSection
      .getByRole("button", { name: "Regenerate user segments" })
      .click();
    const segments = await segmentsResponse;
    expect(segments.status(), target.name + " user segment regeneration").toBe(
      202,
    );
    expect(await segments.json()).toMatchObject({
      user_id: 91002,
      status: "queued",
      message: "Segment regeneration queued.",
    });
    await expect(segmentsSection).toContainText("Segment regeneration queued.");

    const importsSection = sectionWithHeading(
      page,
      "Reprocess imported activity files",
    );
    await importsSection
      .getByRole("spinbutton", { name: /User id/ })
      .fill("91004");
    const importsResponse = waitForMutation(
      page,
      "/api/admin/activity-imports/reprocess",
    );
    await importsSection
      .getByRole("button", { name: "Queue import reprocess" })
      .click();
    const imports = await importsResponse;
    expect(imports.status(), target.name + " user import reprocess").toBe(202);
    expect(await imports.json()).toMatchObject({
      user_id: 91004,
      status: "queued",
      message: "Activity reprocessing queued.",
    });
    await expect(importsSection).toContainText("Activity reprocessing queued.");

    const activitySection = sectionWithHeading(
      page,
      "Reprocess one imported activity",
    );
    await activitySection
      .getByRole("spinbutton", { name: /Activity id/ })
      .fill("1109");
    const activityResponse = waitForMutation(
      page,
      "/api/admin/activity-imports/reprocess-activity",
    );
    await activitySection
      .getByRole("button", { name: "Queue activity reprocess" })
      .click();
    const activity = await activityResponse;
    expect(activity.status(), target.name + " activity import reprocess").toBe(
      202,
    );
    expect(await activity.json()).toMatchObject({
      activity_id: 1109,
      user_id: 1,
      status: "queued",
      message: "Activity reprocessing queued.",
      task_status: "pending",
    });
    await expect(activitySection).toContainText(
      "Activity reprocessing queued.",
    );

    const cleanupSection = sectionWithHeading(
      page,
      "Clean up duplicate activities",
    );
    await cleanupSection
      .getByRole("spinbutton", { name: /User id/ })
      .fill("91005");
    const cleanupResponse = waitForMutation(
      page,
      "/api/admin/activity-imports/cleanup-duplicates",
    );
    await cleanupSection
      .getByRole("button", { name: "Clean duplicates" })
      .click();
    const cleanup = await cleanupResponse;
    expect(cleanup.status(), target.name + " duplicate cleanup").toBe(200);
    expect(await cleanup.json()).toMatchObject({
      user_id: 91005,
      status: "completed",
      duplicate_group_count: 0,
      deleted_activity_count: 0,
      retained_activity_count: 0,
    });
    await expect(cleanupSection).toContainText(
      "No duplicate activities matched the current dedupe rules.",
    );

    await context.close();
  }
});
