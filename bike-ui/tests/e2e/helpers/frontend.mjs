import { expect } from "@playwright/test";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { openRoute, stabilize } from "./ui.mjs";
import { expectedEffortCount } from "./targets.mjs";

const NEXT_ERROR_MARKERS = [
  "Application error: a client-side exception has occurred",
  "Unhandled Runtime Error",
  "This page could not be found",
];

const rustOpenApiPath =
  process.env.BIKE_RUST_OPENAPI_FILE ??
  fileURLToPath(
    new URL("../../../../contracts/openapi/openapi.json", import.meta.url),
  );
const rustOpenApi = JSON.parse(readFileSync(rustOpenApiPath, "utf8"));
const apiBasePath = new URL(
  rustOpenApi.servers[0].url,
  "http://bike.local",
).pathname.replace(/\/$/, "");

function operationPathPattern(path) {
  const segments = `${apiBasePath}${path}`.split("/").slice(1);
  const pattern = segments.map((segment) =>
    segment.startsWith("{") && segment.endsWith("}")
      ? "[^/]+"
      : segment.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"),
  );
  return new RegExp(`^/${pattern.join("/")}$`);
}

const apiOperations = Object.entries(rustOpenApi.paths).flatMap(
  ([path, methods]) =>
    Object.entries(methods)
      .filter(([, operation]) => operation.operationId)
      .map(([method, operation]) => ({
        method: method.toUpperCase(),
        operationId: operation.operationId,
        path: operationPathPattern(path),
      })),
);

export async function openFrontendRoute(page, target, route, options = {}) {
  const requests = [];
  const responses = [];
  const pageErrors = [];
  const requestListener = (request) => {
    if (new URL(request.url()).pathname.startsWith("/api/")) {
      requests.push(request);
    }
  };
  const responseListener = (response) => {
    if (new URL(response.url()).pathname.startsWith("/api/")) {
      responses.push(response);
    }
  };
  const pageErrorListener = (error) => pageErrors.push(error.message);
  const settleMs =
    options.settleMs ?? Number(process.env.PLAYWRIGHT_ROUTE_SETTLE_MS ?? "750");
  const expected = new URL(route.path, `${target.url.replace(/\/$/, "")}/`);

  page.on("request", requestListener);
  page.on("response", responseListener);
  page.on("pageerror", pageErrorListener);
  await openRoute(page, target, route.path);
  // Do not wait for networkidle: reports and preview maps intentionally keep
  // background requests active. A bounded settle window is enough for auth
  // redirects and hydration without turning the matrix into a load test.
  await page.waitForTimeout(settleMs);
  // The race viewer intentionally owns its full-screen shell and does not
  // render the normal Layout/main wrapper. Body is the common route root.
  await expect(page.locator("body")).toBeVisible();
  const body = page.locator("body");
  await expect(
    body,
    `${target.name}/${route.name} pageErrors=${pageErrors.join(" | ")}`,
  ).not.toContainText(NEXT_ERROR_MARKERS[0]);
  await expect(body, `${target.name}/${route.name}`).not.toContainText(
    NEXT_ERROR_MARKERS[1],
  );
  await expect(body, `${target.name}/${route.name}`).not.toContainText(
    NEXT_ERROR_MARKERS[2],
  );

  let settledUrl = new URL(page.url());
  if (options.retryAuthRedirect) {
    for (
      let attempt = 0;
      attempt < 3 &&
      settledUrl.pathname !== expected.pathname &&
      ["/", "/login"].includes(settledUrl.pathname);
      attempt += 1
    ) {
      await page.waitForTimeout(500);
      await openRoute(page, target, route.path);
      await page.waitForTimeout(settleMs);
      settledUrl = new URL(page.url());
    }
  }
  expect(
    settledUrl.pathname,
    `${target.name}/${route.name} redirected unexpectedly`,
  ).toBe(expected.pathname);
  expect(settledUrl.search, `${target.name}/${route.name} lost URL state`).toBe(
    expected.search,
  );

  return {
    requests,
    responses,
    async close() {
      page.removeListener("request", requestListener);
      page.removeListener("response", responseListener);
      page.removeListener("pageerror", pageErrorListener);
    },
  };
}

export async function assertKeyboardNavigation(page) {
  const focusableCount = await page
    .locator(
      "a[href]:visible,button:not([disabled]):visible,input:not([disabled]):visible,select:not([disabled]):visible,textarea:not([disabled]):visible",
    )
    .count();
  if (focusableCount === 0) return;

  // Start from the document body instead of a component-specific control.
  // Map widgets can intentionally move focus to their own canvas after a
  // control receives focus, which is not a failure of page-level tab order.
  await page.locator("body").focus();
  await page.keyboard.press("Tab");
  await expect
    .poll(() => page.evaluate(() => document.activeElement !== document.body))
    .toBe(true);
}

export async function stabilizePage(page) {
  await stabilize(page);
  await page.waitForTimeout(50);
}

export function apiOperationEvidence(responses) {
  return responses.map((response) => ({
    url: new URL(response.url()).pathname,
    method: response.request().method(),
    operationId:
      apiOperations.find(
        (operation) =>
          operation.method === response.request().method() &&
          operation.path.test(new URL(response.url()).pathname),
      )?.operationId ?? null,
    status: response.status(),
    requestId:
      response.headers()["x-request-id"] ??
      response.headers()["x-correlation-id"] ??
      null,
  }));
}

export function dynamicVisualMasks(page) {
  return [page.locator("span").filter({ hasText: /^Updated / })];
}

export function visualViewport() {
  const raw = process.env.PLAYWRIGHT_VIEWPORT?.trim();
  if (!raw) return { viewport: { width: 1440, height: 900 }, suffix: "" };

  const match = /^(\d+)x(\d+)$/.exec(raw);
  if (!match)
    throw new Error(
      `Invalid PLAYWRIGHT_VIEWPORT=${raw}; expected WIDTHxHEIGHT`,
    );

  const viewport = { width: Number(match[1]), height: Number(match[2]) };
  if (viewport.width < 1 || viewport.height < 1) {
    throw new Error(
      `Invalid PLAYWRIGHT_VIEWPORT=${raw}; dimensions must be positive`,
    );
  }

  const suffix =
    viewport.width === 1440 && viewport.height === 900
      ? ""
      : `-${viewport.width}x${viewport.height}`;
  return { viewport, suffix };
}

export function visualThemes() {
  const themes = (process.env.PLAYWRIGHT_THEMES ?? "light")
    .split(",")
    .map((theme) => theme.trim());
  if (themes.some((theme) => !["light", "dark"].includes(theme))) {
    throw new Error("PLAYWRIGHT_THEMES must contain only light and dark");
  }
  return [...new Set(themes)];
}

export async function waitForVisualReady(page, routeName) {
  const timeout = 15_000;
  if (routeName === "activity-list") {
    await expect(
      page.getByRole("heading", { name: "Recent activities" }),
    ).toBeVisible({ timeout });
    const activityCard = page.locator("article").first();
    await expect(activityCard).toBeVisible({ timeout });
    const previewImages = activityCard.locator("img");
    if ((await previewImages.count()) > 0) {
      await expect
        .poll(() =>
          previewImages.evaluateAll((images) =>
            images.every((image) => image.complete && image.naturalWidth > 0),
          ),
        )
        .toBe(true);
    }
  } else if (routeName === "activity-detail") {
    await expect(
      page.getByRole("img", { name: "Activity route map" }),
    ).toBeVisible({ timeout });
  } else if (routeName === "segment-detail") {
    await expect(
      page.getByText(
        `Showing 1-${Math.min(10, expectedEffortCount)} of ${expectedEffortCount} efforts`,
      ),
    ).toBeVisible({ timeout });
    const comparisonMap = page.locator('[aria-label="Segment comparison map"]');
    await expect(comparisonMap).toBeVisible({ timeout });
    await expect(
      comparisonMap.locator(".maplibregl-ctrl-attrib"),
    ).toContainText("Waymarked Trails", { timeout });
    await page.waitForTimeout(2_000);
  } else if (routeName === "race-viewer") {
    await expect(
      page.locator('[aria-label="Segment race viewer map"]'),
    ).toBeVisible({ timeout });
    await page.waitForTimeout(8_000);
  } else if (routeName === "segment-progress") {
    await expect(page.getByText("Overall change:")).toBeVisible({ timeout });
  } else if (routeName === "segment-analysis") {
    await expect(
      page.locator('[aria-label$="analysis sections map"]'),
    ).toBeVisible({ timeout });
    await page.waitForTimeout(8_000);
  } else if (routeName === "xc-training") {
    await expect(
      page.getByRole("heading", { name: "XC goals & progress" }),
    ).toBeVisible({ timeout });
  }
}
