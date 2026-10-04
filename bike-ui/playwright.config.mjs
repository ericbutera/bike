import { defineConfig } from "@playwright/test";

const browserName = process.env.PLAYWRIGHT_BROWSER ?? "chromium";

export default defineConfig({
  testDir: "./tests/e2e",
  outputDir: ".artifacts/playwright/test-results",
  timeout: 45_000,
  expect: {
    timeout: 10_000,
  },
  fullyParallel: false,
  workers: 1,
  reporter: "list",
  use: {
    browserName,
    headless: true,
    viewport: { width: 1440, height: 900 },
    colorScheme: "light",
    locale: "en-US",
    timezoneId: "America/Detroit",
    reducedMotion: "reduce",
    trace: process.env.BIKE_SYNTHETIC_KEY ? "off" : "retain-on-failure",
  },
  snapshotPathTemplate: "{testDir}/snapshots/{arg}{ext}",
});
