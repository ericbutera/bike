import { test, expect } from "@playwright/test";
import { openRoute } from "./helpers/ui.mjs";
import {
  activityId,
  raceEffortIds,
  segmentId,
  targets,
} from "./helpers/targets.mjs";

test("the priority views expose accessible loading state in Bike", async ({
  browser,
}) => {
  const scenarios = [
    {
      path: "/",
      apiPath: "/api/activities",
      transform: (activities) => ({ ...activities, data: [] }),
      loading: (page) => page.getByRole("status", { name: "Loading" }),
      ready: (page) => page.getByText(/No activities yet\./),
    },
    {
      path: `/activities/${activityId}`,
      apiPath: `/api/activities/${activityId}`,
      transform: (activity) => ({ ...activity, route_points: [] }),
      loading: (page) => page.getByRole("status", { name: "Loading" }),
      ready: (page) =>
        page.getByText(
          "This activity does not have enough stored route points for the map yet.",
        ),
    },
    {
      path: `/segments/${segmentId}`,
      apiPath: `/api/segments/${segmentId}/comparison`,
      transform: (comparison) => ({
        ...comparison,
        efforts: [],
        route_points: [],
      }),
      loading: (page) => page.getByLabel("Loading segment efforts"),
      ready: (page) => page.getByText("No efforts match this time window."),
    },
    {
      path: `/segments/${segmentId}/race?efforts=${encodeURIComponent(
        raceEffortIds,
      )}`,
      apiPath: `/api/segments/${segmentId}/comparison`,
      transform: (comparison) => ({
        ...comparison,
        efforts: [],
        route_points: [],
      }),
      loading: (page) =>
        page.getByRole("status", { name: "Loading segment race viewer" }),
      ready: (page) =>
        page.getByText("Segment route geometry is not available yet."),
    },
  ];

  for (const target of targets) {
    for (const scenario of scenarios) {
      const context = await browser.newContext({ colorScheme: "light" });
      const page = await context.newPage();
      let releaseResponse;
      let markRequestPaused;
      const responseGate = new Promise((resolve) => {
        releaseResponse = resolve;
      });
      const requestPaused = new Promise((resolve) => {
        markRequestPaused = resolve;
      });

      await page.route("**/api/**", async (route) => {
        if (new URL(route.request().url()).pathname !== scenario.apiPath) {
          await route.continue();
          return;
        }

        markRequestPaused();
        await responseGate;
        const response = await route.fetch();
        await route.fulfill({
          response,
          json: scenario.transform(await response.json()),
        });
      });

      await openRoute(page, target, scenario.path);
      await requestPaused;
      await expect(scenario.loading(page)).toBeVisible();
      releaseResponse();
      await expect(scenario.ready(page)).toBeVisible();

      await context.close();
    }
  }
});

test("the priority detail views show the same missing-record state", async ({
  browser,
}) => {
  const missingRoutes = [
    ["/activities/2147483647", "Unable to load activity."],
    ["/segments/2147483647", "Unable to load segment."],
    ["/segments/2147483647/race", "Unable to load segment."],
  ];

  for (const target of targets) {
    const context = await browser.newContext({ colorScheme: "light" });
    const page = await context.newPage();

    for (const [route, message] of missingRoutes) {
      await openRoute(page, target, route);
      await expect(page.locator(".alert-error")).toHaveText(message);
    }

    await context.close();
  }
});

test("the activity list exposes the same API failure state", async ({
  browser,
}) => {
  for (const target of targets) {
    const context = await browser.newContext({ colorScheme: "light" });
    const page = await context.newPage();

    await page.route("**/api/activities**", async (route) => {
      await route.fulfill({
        status: 503,
        contentType: "application/json",
        body: JSON.stringify({ message: "Service unavailable" }),
      });
    });

    await openRoute(page, target);
    await expect(page.locator(".alert-error")).toHaveText(
      "Unable to load activities.",
    );

    await context.close();
  }
});

test("the priority views preserve their empty-data states", async ({
  browser,
}) => {
  const scenarios = [
    {
      path: "/",
      apiPath: "/api/activities",
      fulfill: async (route) =>
        route.fulfill({
          status: 200,
          contentType: "application/json",
          body: JSON.stringify({
            data: [],
            metadata: { page: 1, per_page: 10, total: 0, total_pages: 0 },
          }),
        }),
      assert: (page) =>
        expect(
          page.getByText(
            "No activities yet. Upload a GPX, TCX, or FIT file below to seed your stream.",
          ),
        ).toBeVisible(),
    },
    {
      path: `/activities/${activityId}`,
      apiPath: `/api/activities/${activityId}`,
      transform: (activity) => ({
        ...activity,
        route_points: [],
        chart_points: [],
        heart_rate_zones: [],
        estimated_ftp_watts: null,
      }),
      assert: async (page) => {
        await expect(
          page.getByText(
            "This activity does not have enough stored route points for the map yet.",
          ),
        ).toBeVisible();
        await expect(
          page.getByRole("heading", { name: "Zone distribution" }),
        ).toHaveCount(0);
        await expect(
          page.getByRole("img", { name: "Activity signals chart" }),
        ).toHaveCount(0);
        await expect(
          page.getByText(
            "Turn on at least one signal layer to render the chart.",
          ),
        ).toBeVisible();
      },
    },
    {
      path: `/segments/${segmentId}`,
      apiPath: `/api/segments/${segmentId}/comparison`,
      transform: (comparison) => ({
        ...comparison,
        efforts: [],
        route_points: [],
      }),
      assert: async (page) => {
        await expect(
          page.getByText("No efforts match this time window."),
        ).toBeVisible();
        await expect(
          page.getByText("Segment route geometry is not available yet."),
        ).toBeVisible();
      },
    },
    {
      path: `/segments/${segmentId}/race?efforts=${encodeURIComponent(
        raceEffortIds,
      )}`,
      apiPath: `/api/segments/${segmentId}/comparison`,
      transform: (comparison) => ({
        ...comparison,
        efforts: [],
        route_points: [],
      }),
      assert: async (page) => {
        await expect(
          page.getByText("Segment route geometry is not available yet."),
        ).toBeVisible();
        await expect(
          page.getByRole("button", { name: "Play race playback" }),
        ).toBeDisabled();
      },
    },
  ];

  for (const target of targets) {
    for (const scenario of scenarios) {
      const context = await browser.newContext({ colorScheme: "light" });
      const page = await context.newPage();

      await page.route("**/api/**", async (route) => {
        if (new URL(route.request().url()).pathname !== scenario.apiPath) {
          await route.continue();
          return;
        }

        if (scenario.fulfill) {
          await scenario.fulfill(route);
          return;
        }

        const response = await route.fetch();
        await route.fulfill({
          response,
          json: scenario.transform(await response.json()),
        });
      });

      await openRoute(page, target, scenario.path);
      await scenario.assert(page);

      await context.close();
    }
  }
});
