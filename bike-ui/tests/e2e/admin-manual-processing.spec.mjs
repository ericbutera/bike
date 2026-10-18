import { connectedTest as test, expect } from "./helpers/test.mjs";
test.use({ scenario: "manual-processing" });
import { targets } from "./helpers/targets.mjs";
import { openRoute } from "./helpers/ui.mjs";

function waitForMutation(page, path) {
  return page.waitForResponse(
    (response) =>
      new URL(response.url()).pathname === path &&
      response.request().method() === "POST",
  );
}

async function runRowTask(page, kind, id, label, workflowType) {
  await page
    .getByRole("button", { name: new RegExp(`Tasks for .*\\(#${id}\\)$`) })
    .click();
  if (kind === "users")
    await page.getByText("Task management", { exact: true }).click();
  await page.getByRole("menuitem", { name: label, exact: true }).click();
  const dialog = page.getByRole("dialog");
  const response = waitForMutation(page, `/api/admin/${kind}/${id}/tasks`);
  await dialog.getByRole("button", { name: "Run task" }).click();
  const result = await response;
  expect(result.status()).toBe(202);
  const accepted = await result.json();
  expect(accepted).toMatchObject({
    status: "accepted",
    job: { workflow_type: workflowType, run_id: expect.any(String) },
  });
  await expect(dialog.getByRole("status")).toContainText("Task accepted");
  await dialog.getByRole("button", { name: "Close", exact: true }).click();
  return accepted.job;
}

async function inspectTaskHistory(page, target, job) {
  await openRoute(page, target, "/admin/tasks");
  const row = page
    .getByRole("row")
    .filter({ has: page.getByTitle(`${job.job_id} / ${job.run_id}`) });
  await expect
    .poll(
      async () => {
        await page
          .getByRole("button", { name: "Refresh", exact: true })
          .click();
        return row.count();
      },
      { timeout: 15_000 },
    )
    .toBe(1);
  await row.getByRole("button", { name: "Details" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toContainText('"activity_id": 1109');
  await dialog.getByRole("button", { name: "Close", exact: true }).click();
}

test("shared admin tools queue matching import and training work in Bike", async ({
  createContext,
}) => {
  test.setTimeout(60_000);

  for (const target of targets) {
    const context = await createContext();
    const page = await context.newPage();
    await openRoute(page, target, "/admin/users");
    await runRowTask(
      page,
      "users",
      91003,
      "Backfill XC training",
      "backfill_user_xc_training",
    );
    await runRowTask(
      page,
      "users",
      91002,
      "Regenerate segment efforts",
      "regenerate_user_segments",
    );
    await runRowTask(
      page,
      "users",
      91004,
      "Reprocess user imports",
      "reprocess_user_activity_imports",
    );
    await openRoute(page, target, "/admin/activities?q=1109");
    const job = await runRowTask(
      page,
      "activities",
      1109,
      "Reprocess activity",
      "reprocess_activity_import",
    );
    await inspectTaskHistory(page, target, job);
    await openRoute(page, target, "/admin/users");
    await page.getByRole("button", { name: /Tasks for .*\(#91005\)$/ }).click();
    await page.getByText("Task management", { exact: true }).click();
    await page
      .getByRole("menuitem", {
        name: "Clean up duplicate activities",
        exact: true,
      })
      .click();
    const cleanupDialog = page.getByRole("dialog");
    const cleanupResponse = waitForMutation(
      page,
      "/api/admin/activity-imports/cleanup-duplicates",
    );
    await cleanupDialog
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
    await expect(cleanupDialog).toContainText(
      "No duplicate activities matched the current dedupe rules.",
    );

    await context.close();
  }
});
