import { test, expect } from "@playwright/test";
import { protectedFrontendRoutes } from "./helpers/frontend-routes.mjs";
import { openRoute } from "./helpers/ui.mjs";
import {
  activityId,
  raceEffortIds,
  segmentId,
  targets,
} from "./helpers/targets.mjs";

test("protected routes expose the shared sign-in state when auth is unavailable", async ({
  browser,
}) => {
  test.setTimeout(240_000);
  test.skip(
    process.env.PLAYWRIGHT_AUTH_STATE !== "1",
    "Set PLAYWRIGHT_AUTH_STATE=1 to run the isolated unauthorized-state matrix.",
  );

  for (const target of targets) {
    for (const route of protectedFrontendRoutes) {
      const context = await browser.newContext({ colorScheme: "light" });
      const page = await context.newPage();
      await page.route("**/api/auth/current", async (routeHandler) => {
        await routeHandler.fulfill({
          status: 401,
          contentType: "application/json",
          body: JSON.stringify({ message: "Unauthorized" }),
        });
      });

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
  browser,
}) => {
  for (const target of targets) {
    const context = await browser.newContext({ colorScheme: "light" });
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

    await openRoute(page, target, "/admin/tasks");
    await expect(page).toHaveURL(new URL("/", target.url).toString());
    await expect(
      page.getByRole("heading", { name: "Recent activities" }),
    ).toBeVisible();

    await context.close();
  }
});

test("forbidden activity reads show the same access state", async ({
  browser,
}) => {
  for (const target of targets) {
    const context = await browser.newContext({ colorScheme: "light" });
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
    await page.route(`**/api/activities/${activityId}`, async (route) => {
      await route.fulfill({
        status: 403,
        contentType: "application/json",
        body: JSON.stringify({ message: "Forbidden" }),
      });
    });

    await openRoute(page, target, `/activities/${activityId}`);
    await expect(page.locator(".alert-error")).toHaveText(
      "Unable to load activity.",
    );

    await context.close();
  }
});

test("forbidden segment comparison reads show the same access state", async ({
  browser,
}) => {
  const deniedRoutes = [
    `/segments/${segmentId}`,
    `/segments/${segmentId}/race?efforts=${encodeURIComponent(raceEffortIds)}`,
  ];

  for (const target of targets) {
    const context = await browser.newContext({ colorScheme: "light" });
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
    await page.route("**/api/segments/*/comparison**", async (route) => {
      await route.fulfill({
        status: 403,
        contentType: "application/json",
        body: JSON.stringify({ message: "Forbidden" }),
      });
    });

    for (const route of deniedRoutes) {
      await openRoute(page, target, route);
      await expect(page.locator(".alert-error")).toHaveText(
        "Unable to load segment comparison.",
      );
    }

    await context.close();
  }
});
