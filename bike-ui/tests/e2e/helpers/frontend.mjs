import { expect } from "@playwright/test";
import { openRoute, stabilize } from "./ui.mjs";
import { expectedEffortCount } from "./targets.mjs";

const NEXT_ERROR_MARKERS = [
  "Application error: a client-side exception has occurred",
  "Unhandled Runtime Error",
  "This page could not be found",
];

export async function openFrontendRoute(page, target, route) {
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
  const expected = new URL(route.path, `${target.url.replace(/\/$/, "")}/`);

  page.on("request", requestListener);
  page.on("response", responseListener);
  page.on("pageerror", pageErrorListener);
  await openRoute(page, target, route.path);
  if (route.name === "race-viewer") {
    await expect(page.getByLabel("Race playback timeline")).toBeVisible();
  } else if (expected.pathname === "/maps") {
    await expect(
      page.getByRole("toolbar", { name: "Heatmap controls" }),
    ).toBeVisible();
  } else {
    await expect(page.getByRole("heading").first()).toBeVisible();
  }
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

  await expect(
    page,
    `${target.name}/${route.name} retains its URL state`,
  ).toHaveURL(expected.toString());

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

export async function stabilizePage(page) {
  await stabilize(page);
  await page.evaluate(
    () =>
      new Promise((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(resolve)),
      ),
  );
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
  const themes = (process.env.PLAYWRIGHT_THEMES ?? "light,dark")
    .split(",")
    .map((theme) => theme.trim());
  if (themes.some((theme) => !["light", "dark"].includes(theme))) {
    throw new Error("PLAYWRIGHT_THEMES must contain only light and dark");
  }
  return [...new Set(themes)];
}

export async function waitForVisualReady(
  page,
  routeName,
  effortCount = expectedEffortCount,
) {
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
        `Showing 1-${Math.min(10, effortCount)} of ${effortCount} efforts`,
      ),
    ).toBeVisible({ timeout });
    const comparisonMap = page.locator('[aria-label="Segment comparison map"]');
    await expect(comparisonMap).toBeVisible({ timeout });
    await expect(
      comparisonMap.locator(".maplibregl-ctrl-attrib"),
    ).toContainText("OpenFreeMap", { timeout });
    await expect(comparisonMap.locator("canvas")).toBeVisible();
  } else if (routeName === "race-viewer") {
    await expect(
      page.locator('[aria-label="Segment race viewer map"]'),
    ).toBeVisible({ timeout });
    await expect(
      page.locator('[aria-label="Segment race viewer map"] canvas'),
    ).toBeVisible();
  } else if (routeName === "segment-progress") {
    await expect(page.getByText("Overall change:")).toBeVisible({ timeout });
  } else if (routeName === "segment-analysis") {
    await expect(
      page.locator('[aria-label$="analysis sections map"]'),
    ).toBeVisible({ timeout });
    await expect(
      page.locator('[aria-label$="analysis sections map"] canvas'),
    ).toBeVisible();
  } else if (routeName === "xc-training") {
    await expect(
      page.getByRole("heading", { name: "XC goals & progress" }),
    ).toBeVisible({ timeout });
  }
}
