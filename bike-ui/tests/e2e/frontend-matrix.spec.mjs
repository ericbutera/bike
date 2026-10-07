import { test, expect } from "./helpers/test.mjs";
import { selectedFrontendRoutes } from "./helpers/frontend-routes.mjs";
import {
  apiOperationEvidence,
  assertKeyboardNavigation,
  openFrontendRoute,
  stabilizePage,
} from "./helpers/frontend.mjs";
import { targets } from "./helpers/targets.mjs";

test.describe("shared frontend route and interaction matrix", () => {
  for (const route of selectedFrontendRoutes()) {
    test(`${route.name} has the same navigable route in Bike`, async ({
      browser,
    }) => {
      for (const target of targets) {
        const context = await browser.newContext({
          colorScheme: "light",
          locale: "en-US",
          timezoneId: "America/Detroit",
          reducedMotion: "reduce",
        });
        const page = await context.newPage();
        const evidence = await openFrontendRoute(page, target, route, {
          retryAuthRedirect: true,
        });
        await stabilizePage(page);
        await assertKeyboardNavigation(page);

        // This records the concrete API requests behind each UI route. It is
        // deliberately kept as evidence instead of guessing operation IDs
        // from browser text.
        const operations = apiOperationEvidence(evidence.responses);
        if (process.env.BIKE_TEST_EXPECT_AUTHENTICATED === "true") {
          expect(
            operations.some(
              ({ operationId, status }) =>
                operationId === "current" && status === 200,
            ),
            `${target.name}/${route.name} did not establish the fixture session`,
          ).toBe(true);
          if (route.name === "activity-list") {
            await expect(
              page.getByRole("heading", { name: "Recent activities" }),
            ).toBeVisible();
          }
          if (route.name === "training-reports") {
            const reports = operations.filter(
              ({ operationId }) => operationId === "get_training_reports",
            );
            expect(
              reports,
              `${target.name} fetched more than the selected report`,
            ).toHaveLength(1);
            expect(
              reports[0].status,
              `${target.name} failed to load the selected report`,
            ).toBe(200);
          }
        }
        expect(
          operations.filter(
            ({ method, operationId }) =>
              method !== "OPTIONS" && operationId === null,
          ),
          `${target.name}/${route.name} requested an undocumented API route: ${JSON.stringify(operations)}`,
        ).toEqual([]);
        expect(
          operations.every(({ status }) => status < 500),
          `${target.name}/${route.name} returned a server error: ${JSON.stringify(operations)}`,
        ).toBe(true);

        await evidence.close();
        await context.close();
      }
    });
  }
});
