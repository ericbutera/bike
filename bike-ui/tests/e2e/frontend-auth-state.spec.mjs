import { test, expect } from "./helpers/test.mjs";
import { protectedFrontendRoutes } from "./helpers/frontend-routes.mjs";
import { openRoute } from "./helpers/ui.mjs";
import { fakeProductApi } from "./helpers/product-fixtures.mjs";
import { fakeApiResponse } from "./helpers/api-response.mjs";
import {
  activityId,
  raceEffortIds,
  segmentId,
  targets,
} from "./helpers/targets.mjs";

test("protected routes expose the shared sign-in state when auth is unavailable", async ({
  createContext,
}) => {
  test.setTimeout(240_000);

  for (const target of targets) {
    for (const route of protectedFrontendRoutes) {
      const context = await createContext({ colorScheme: "light" });
      const page = await context.newPage();
      await fakeApiResponse(page, ["/api/auth/current"], () => ({
        status: 401,
        json: { message: "Unauthorized" },
      }));

      await openRoute(page, target, route.path);
      await expect(
        page.getByRole("link", { name: "Sign in" }).first(),
        `${target.name}/${route.name} should show the sign-in action`,
      ).toBeVisible();
      await context.close();
    }
  }
});

test("authenticated non-admins cannot open admin pages", async ({
  createContext,
}) => {
  for (const target of targets) {
    const context = await createContext({ colorScheme: "light" });
    const page = await context.newPage();

    const state = await fakeProductApi(page, target);
    await page.route("**/api/auth/current", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          pid: "2",
          name: "Parity Rider",
          email: "parity-rider@example.test",
          is_admin: false,
          verified: true,
        }),
      });
    });

    await openRoute(page, target, "/admin/tasks");
    await expect(page).toHaveURL(new URL("/", target.url).toString());
    await expect(
      page.getByRole("heading", { name: "Recent activities" }),
    ).toBeVisible();

    expect(state.unexpected).toEqual([]);
    await context.close();
  }
});

test.describe("forbidden activity", () => {
  test.use({ expectedConsoleErrors: ["[API Error] Forbidden undefined"] });
  test("forbidden activity reads show the same access state", async ({
    createContext,
  }) => {
    for (const target of targets) {
      const context = await createContext({ colorScheme: "light" });
      const page = await context.newPage();

      await page.route("**/api/auth/current", async (route) => {
        await route.fulfill({
          status: 200,
          contentType: "application/json",
          body: JSON.stringify({
            pid: "2",
            name: "Parity Rider",
            email: "parity-rider@example.test",
            is_admin: false,
            verified: true,
          }),
        });
      });
      await fakeProductApi(page, target);
      await fakeApiResponse(page, [`/api/activities/${activityId}`], () => ({
        status: 403,
        json: { message: "Forbidden" },
      }));

      await openRoute(page, target, `/activities/${activityId}`);
      await expect(page.locator(".alert-error")).toHaveText(
        "Unable to load activity.",
      );

      await context.close();
    }
  });
});

test.describe("forbidden comparison", () => {
  test.use({
    expectedConsoleErrors: Array(2).fill("[API Error] Forbidden undefined"),
  });
  test("forbidden segment comparison reads show the same access state", async ({
    createContext,
  }) => {
    const deniedRoutes = [
      `/segments/${segmentId}`,
      `/segments/${segmentId}/race?efforts=${encodeURIComponent(raceEffortIds)}`,
    ];

    for (const target of targets) {
      const context = await createContext({ colorScheme: "light" });
      const page = await context.newPage();

      const state = await fakeProductApi(page, target);
      await page.route(`**/api/segments/${segmentId}`, async (route) => {
        await route.fulfill({
          json: {
            id: Number(segmentId),
            title: "Fixture segment",
            mode: "xc",
            distance_meters: 1800,
            route_points: [],
            efforts: [],
          },
        });
      });

      await page.route("**/api/auth/current", async (route) => {
        await route.fulfill({
          status: 200,
          contentType: "application/json",
          body: JSON.stringify({
            pid: "2",
            name: "Parity Rider",
            email: "parity-rider@example.test",
            is_admin: false,
            verified: true,
          }),
        });
      });
      await fakeApiResponse(
        page,
        [`/api/segments/${segmentId}/comparison`],
        () => ({ status: 403, json: { message: "Forbidden" } }),
      );

      for (const route of deniedRoutes) {
        await openRoute(page, target, route);
        await expect(page.locator(".alert-error")).toHaveText(
          "Unable to load segment comparison.",
        );
      }

      expect(state.unexpected).toEqual([]);

      await context.close();
    }
  });
});
