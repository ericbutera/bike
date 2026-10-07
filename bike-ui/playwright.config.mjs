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
  webServer:
    process.env.BIKE_E2E_DISPOSABLE === "1"
      ? [
          {
            command: "mise run dev -- --hostname 127.0.0.1 --port 3001",
            url: "http://127.0.0.1:3001/login",
            reuseExistingServer: false,
            env: {
              API_URL: "http://127.0.0.1:3000/api",
              INTERNAL_API_URL: "http://127.0.0.1:3000/api",
              MAP_RENDERER_URL: "http://127.0.0.1:3100",
              LOCAL_ADMIN_AUTO_LOGIN: "false",
            },
          },
          {
            command: "node tests/e2e/helpers/renderer.mjs",
            url: "http://127.0.0.1:3100/healthz",
            reuseExistingServer: false,
            env: {
              MAP_IMAGE_CACHE_DIR: ".artifacts/playwright/map-cache",
              OTEL_TRACES_EXPORTER: "none",
            },
          },
        ]
      : undefined,
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
