import { connectedTest as test, expect } from "./helpers/test.mjs";
import { activityId, targets } from "./helpers/targets.mjs";
import { openRoute } from "./helpers/ui.mjs";

test("segment builder creates fixed geometry and the creator renames it from detail", async ({
  createContext,
}) => {
  test.setTimeout(120_000);

  for (const target of targets) {
    const context = await createContext();
    const page = await context.newPage();
    let apiBase;
    let segmentId;

    try {
      await openRoute(
        page,
        target,
        `/segments/builder?activityId=${activityId}`,
      );
      await expect(
        page.getByRole("heading", { name: "Synthetic northbound A" }),
      ).toBeVisible();
      await expect(
        page.getByRole("img", { name: "Segment builder map" }),
      ).toBeVisible();
      apiBase = await page.evaluate(() =>
        window.__APP_CONFIG__.API_URL.replace(/\/$/, ""),
      );

      const start = page.getByRole("slider", { name: "Start point" });
      const end = page.getByRole("slider", { name: "End point" });
      await page
        .getByRole("button", { name: "Move start point forward one point" })
        .click();
      await page
        .getByRole("button", { name: "Move start point forward one point" })
        .click();
      for (let point = 0; point < 3; point += 1) {
        await page
          .getByRole("button", { name: "Move end point backward one point" })
          .click();
      }
      await expect(start).toHaveValue("2");
      await expect(end).toHaveValue("7");

      await page
        .getByRole("textbox", { name: "Segment name" })
        .fill("Parity UI builder segment");
      const created = page.waitForResponse(
        (response) =>
          new URL(response.url()).pathname === "/api/segments/from-activity" &&
          response.request().method() === "POST",
      );
      await page.getByRole("button", { name: "Save segment" }).click();
      const createResponse = await created;
      expect(createResponse.status(), `${target.name} builder create`).toBe(
        201,
      );
      const createBody = await createResponse.json();
      expect(createBody).toMatchObject({
        title: "Parity UI builder segment",
        builder_source: {
          activity_id: Number(activityId),
          start_route_point_index: 2,
          end_route_point_index: 7,
        },
      });
      segmentId = createBody.id;
      await expect(page).toHaveURL(new RegExp(`/segments/${segmentId}$`));
      await expect(
        page.getByRole("heading", { name: "Parity UI builder segment" }),
      ).toBeVisible();

      await page.getByRole("button", { name: "Open segment actions" }).click();
      await expect(
        page.getByRole("link", { name: "Edit in builder" }),
      ).toHaveCount(0);
      await page.getByRole("button", { name: "Rename", exact: true }).click();
      await page
        .getByRole("textbox", { name: "Segment name" })
        .fill("Renamed fixed segment");
      const renamed = page.waitForResponse(
        (response) =>
          new URL(response.url()).pathname === `/api/segments/${segmentId}` &&
          response.request().method() === "PUT",
      );
      await page.getByRole("button", { name: "Save", exact: true }).click();
      const renameResponse = await renamed;
      expect(renameResponse.status(), `${target.name} creator rename`).toBe(
        200,
      );
      const renameBody = await renameResponse.json();
      expect(renameBody).toMatchObject({
        id: segmentId,
        title: "Renamed fixed segment",
        distance_meters: createBody.distance_meters,
        builder_source: createBody.builder_source,
      });
      expect(renameBody.processing_task_id).toBeUndefined();
      await expect(
        page.getByRole("heading", { name: "Renamed fixed segment" }),
      ).toBeVisible();
      await openRoute(page, target, `/segments/builder?segmentId=${segmentId}`);
      await expect(
        page.getByText(
          "Saved segment geometry is locked. Rename the segment from its detail page.",
          { exact: true },
        ),
      ).toBeVisible();
      await expect(page.getByRole("slider")).toHaveCount(0);
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
                `Failed to restore builder fixture segment: ${response.status}`,
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
