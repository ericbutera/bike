import { expect } from "@playwright/test";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import pixelmatch from "pixelmatch";
import { PNG } from "pngjs";

const uiRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../../..",
);

export const artifactRoot = path.resolve(uiRoot, ".artifacts/playwright");

export async function openRoute(page, target, route = "") {
  const url = new URL(route, `${target.url.replace(/\/$/, "")}/`).toString();
  await page.goto(url, { waitUntil: "domcontentloaded" });
}

export async function openActivityList(page, target) {
  await openRoute(page, target);
  await page.waitForLoadState("networkidle");
  await expect(
    page.getByRole("heading", { name: "Recent activities" }),
  ).toBeVisible();
  await expect(
    page.getByRole("link", { name: "Upload Activity" }),
  ).toBeVisible();
  const previewImages = page.locator("article").first().locator("img");
  if ((await previewImages.count()) > 0) {
    await expect
      .poll(() =>
        previewImages.evaluateAll((images) =>
          images.every((image) => image.complete && image.naturalWidth > 0),
        ),
      )
      .toBe(true);
  }
}

export async function openActivityDetail(page, target, id) {
  await openRoute(page, target, `/activities/${id}`);
  await expect(page.locator('[aria-label="Activity route map"]')).toBeVisible();
}

export async function selectTheme(page, theme) {
  await page.getByRole("button", { name: "Account" }).click();

  const controller = page.locator("input.theme-controller");
  await expect(controller).toBeVisible();
  const shouldBeChecked = theme === "dark";
  if ((await controller.isChecked()) !== shouldBeChecked) {
    await controller.click();
  }
  await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
  await page.keyboard.press("Escape");
}

export async function stabilize(page) {
  await page.addStyleTag({
    content: `
      *, *::before, *::after {
        animation: none !important;
        transition: none !important;
        caret-color: transparent !important;
      }
    `,
  });
}

export async function captureElement(element, targetName, artifactName) {
  const screenshotPath = path.join(
    artifactRoot,
    "screenshots",
    `${targetName}-${artifactName}.png`,
  );
  await fs.mkdir(path.dirname(screenshotPath), { recursive: true });
  await element.screenshot({ path: screenshotPath });
  return screenshotPath;
}

export async function compareScreenshots(
  baselinePath,
  candidatePath,
  diffPath,
) {
  const baseline = PNG.sync.read(await fs.readFile(baselinePath));
  const candidate = PNG.sync.read(await fs.readFile(candidatePath));

  if (
    baseline.width !== candidate.width ||
    baseline.height !== candidate.height
  ) {
    return {
      differentPixels: Infinity,
      totalPixels: Math.max(
        baseline.width * baseline.height,
        candidate.width * candidate.height,
      ),
      reason: `dimensions differ: ${baseline.width}x${baseline.height} vs ${candidate.width}x${candidate.height}`,
    };
  }

  const diff = new PNG({ width: baseline.width, height: baseline.height });
  const differentPixels = pixelmatch(
    baseline.data,
    candidate.data,
    diff.data,
    baseline.width,
    baseline.height,
    { threshold: 0.1 },
  );
  await fs.mkdir(path.dirname(diffPath), { recursive: true });
  await fs.writeFile(diffPath, PNG.sync.write(diff));
  return {
    differentPixels,
    totalPixels: baseline.width * baseline.height,
    reason: null,
  };
}
