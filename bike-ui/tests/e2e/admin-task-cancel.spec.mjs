import { connectedTest as test, expect } from "./helpers/test.mjs";
test.use({ scenario: "task-cancel" });
import { targets } from "./helpers/targets.mjs";
import { openRoute } from "./helpers/ui.mjs";

test("the shared admin task page cancels a processing task", async ({
  createContext,
}) => {
  for (const target of targets) {
    const context = await createContext();
    const page = await context.newPage();
    await openRoute(page, target, "/admin/tasks");
    await expect(
      page.getByRole("heading", { name: "Background Tasks" }),
    ).toBeVisible();

    const task = page
      .locator("tbody tr")
      .filter({ hasText: "process_activity_import" });
    await expect(task).toContainText("processing");
    await task.getByLabel("Task 5 actions").click();
    const canceled = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname === "/api/admin/tasks/5/cancel" &&
        response.request().method() === "POST",
    );
    await task.getByRole("button", { name: "Cancel" }).click();

    const response = await canceled;
    expect(response.status(), `${target.name} task cancellation`).toBe(200);
    const body = await response.json();
    expect(body).toMatchObject({
      id: 5,
      status: "canceled",
      error: "Canceled by admin",
    });
    expect(new Date(body.scheduled_for).toISOString()).toBe(
      "2099-01-01T00:00:00.000Z",
    );
    expect(new Date(body.started_at).toISOString()).toBe(
      "2025-06-07T12:01:00.000Z",
    );
    await expect(task).toContainText("canceled");
    await context.close();
  }
});
