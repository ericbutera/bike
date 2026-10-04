import { test, expect } from "@playwright/test";
import { raceEffortIds, segmentId, targets } from "./helpers/targets.mjs";
import { visualViewport } from "./helpers/frontend.mjs";
import { openRoute } from "./helpers/ui.mjs";

test("race viewer controls and selected efforts work in Bike", async ({
  browser,
}) => {
  test.setTimeout(120_000);
  const expectedEfforts = raceEffortIds.split(",");
  expect(expectedEfforts).toHaveLength(2);
  const { viewport } = visualViewport();

  for (const target of targets) {
    const context = await browser.newContext({
      colorScheme: "light",
      viewport,
    });
    const page = await context.newPage();
    // Keep control interactions independent of third-party tile outages, which
    // otherwise raise Next's dev error overlay above the playback controls.
    await page.route(
      /^https:\/\/(?:tiles\.openfreemap\.org|tile\.waymarkedtrails\.org|server\.arcgisonline\.com)\//,
      async (route) => {
        if (route.request().url().includes("/styles/")) {
          await route.fulfill({
            contentType: "application/json",
            headers: { "access-control-allow-origin": "*" },
            body: JSON.stringify({ version: 8, sources: {}, layers: [] }),
          });
          return;
        }

        await route.fulfill({
          contentType: "image/png",
          headers: { "access-control-allow-origin": "*" },
          body: Buffer.from(
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScL/nwAAAABJRU5ErkJggg==",
            "base64",
          ),
        });
      },
    );
    await openRoute(
      page,
      target,
      `/segments/${segmentId}/race?efforts=${raceEffortIds}&pace=0.25`,
    );

    await expect(
      page.locator('[aria-label="Segment race viewer map"]'),
    ).toBeVisible();
    const removeButtons = page.getByRole("button", {
      name: /^Remove .* from race viewer$/,
    });
    await expect(removeButtons).toHaveCount(2);
    await expect(page.getByText("Lead", { exact: true })).toBeVisible();

    const speed = page.getByRole("button", { name: "Race playback speed" });
    await expect(speed).toContainText("0.25x");
    await speed.click();
    await page.getByRole("button", { name: "2x", exact: true }).click();
    await expect(speed).toContainText("2x");

    const timeline = page.getByLabel("Race playback timeline");
    await page.getByRole("button", { name: "Play race playback" }).click();
    await expect(
      page.getByRole("button", { name: "Pause race playback" }),
    ).toBeVisible();
    await expect
      .poll(async () => Number(await timeline.inputValue()))
      .toBeGreaterThan(0);
    await page.getByRole("button", { name: "Pause race playback" }).click();
    await expect(timeline).toBeInViewport();
    await timeline.click({ position: { x: 20, y: 5 } });
    await timeline.fill("20");
    await expect(timeline).toHaveValue("20");

    await page.getByRole("button", { name: "Street", exact: true }).click();
    await expect(
      page.getByRole("button", { name: "Street", exact: true }),
    ).toHaveAttribute("aria-pressed", "true");
    await page.getByRole("button", { name: "Topo", exact: true }).click();
    await page.getByRole("button", { name: "Zoom in" }).click();

    await removeButtons.first().click();
    await expect(removeButtons).toHaveCount(1);
    const back = page.getByRole("link", { name: "Back" });
    const remaining =
      new URL(await back.getAttribute("href"), target.url).searchParams
        .get("efforts")
        ?.split(",") ?? [];
    expect(remaining).toHaveLength(1);

    await page.reload({ waitUntil: "domcontentloaded" });
    await expect(removeButtons).toHaveCount(2);
    await expect(speed).toContainText("0.25x");
    expect(
      new URL(await back.getAttribute("href"), target.url).searchParams.get(
        "efforts",
      ),
    ).toBe(raceEffortIds);
    await back.click();
    await expect(page).toHaveURL(
      new RegExp(`/segments/${segmentId}\\?efforts=`),
    );

    await context.close();
  }
});
