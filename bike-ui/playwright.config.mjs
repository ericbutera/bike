import { defineConfig } from "@playwright/test";

const browserName = process.env.PLAYWRIGHT_BROWSER ?? "chromium";
const disposable = Boolean(process.env.BIKE_E2E_PROJECT);
const mesaRendering =
  process.platform === "linux" && Boolean(process.env.DISPLAY);
const mockedSpecs =
  /(?:auth-happy-path|activity-sync-fixture|map-rendering|heatmap-controls|frontend-auth-state|import-replay)\.spec\.mjs$/;

export default defineConfig({
  testDir: "./tests/e2e",
  outputDir: ".artifacts/playwright/test-results",
  timeout: 45_000,
  expect: {
    timeout: 10_000,
  },
  fullyParallel: false,
  workers: 1,
  forbidOnly: Boolean(process.env.CI),
  failOnFlakyTests: Boolean(process.env.CI),
  reporter: [
    ["list"],
    ["html", { outputFolder: ".artifacts/playwright/report", open: "never" }],
    ["json", { outputFile: ".artifacts/playwright/results.json" }],
  ],
  projects: disposable
    ? [
        { name: "setup", testMatch: "environment.setup.mjs" },
        {
          name: "connected",
          testMatch: "**/*.spec.mjs",
          testIgnore: [
            mockedSpecs,
            /(?:diagram-rendering|frontend-visual)\.spec\.mjs$/,
          ],
          dependencies: ["setup"],
        },
        { name: "mocked", testMatch: mockedSpecs, dependencies: ["setup"] },
        { name: "standalone", testMatch: "diagram-rendering.spec.mjs" },
        ...(process.env.PLAYWRIGHT_VISUAL === "1"
          ? [
              {
                name: "visual",
                testMatch: "frontend-visual.spec.mjs",
                dependencies: ["setup"],
              },
            ]
          : []),
      ]
    : [{ name: "external", testMatch: "**/*.spec.mjs" }],
  use: {
    browserName,
    headless: true,
    channel:
      mesaRendering && browserName === "chromium" ? "chromium" : undefined,
    // Xvfb and Mesa provide real WebGL without a GPU device or Vulkan readback stalls.
    launchOptions:
      mesaRendering && browserName === "chromium"
        ? {
            args: [
              "--enable-gpu",
              "--use-gl=angle",
              "--use-angle=gl",
              "--ignore-gpu-blocklist",
              ...(disposable
                ? [
                    `--unsafely-treat-insecure-origin-as-secure=${new URL(process.env.BIKE_UI_URL ?? "http://ui.e2e.test:3000").origin}`,
                  ]
                : []),
            ],
          }
        : undefined,
    viewport: { width: 1440, height: 900 },
    colorScheme: "light",
    locale: "en-US",
    timezoneId: "America/Detroit",
    reducedMotion: "reduce",
    trace: process.env.BIKE_SYNTHETIC_KEY ? "off" : "retain-on-failure",
    screenshot: "only-on-failure",
  },
  snapshotPathTemplate: "{testDir}/snapshots/{arg}{ext}",
});
