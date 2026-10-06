import { expect } from "@playwright/test";
import path from "node:path";
import { fileURLToPath } from "node:url";

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
