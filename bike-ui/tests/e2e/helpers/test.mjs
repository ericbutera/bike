import { test as base, expect } from "@playwright/test";
import { spawn } from "node:child_process";
import { cp, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { fakeExternalBasemaps } from "./basemaps.mjs";
import { run, stop, waitForService } from "./processes.mjs";

const root = fileURLToPath(new URL("../../../../", import.meta.url));
const fixtures = path.join(root, "bike-rs/api/tests/fixtures/platform");
const target =
  process.env.CARGO_TARGET_DIR ?? path.join(root, "bike-rs/target");
const scenarioFixtures = {
  "activity-climb": ["climb-route.sql"],
  "activity-laps": ["activity-laps.sql"],
  "activity-zone-without-chart": ["activity-zone-without-chart.sql"],
  "admin-users": ["admin-users.sql"],
  "admin-task-cancel": ["admin-processing-task.sql"],
  "admin-manual-processing": ["admin-manual-task-users.sql"],
  "archive-import-states": ["admin-users.sql", "archive-import-states.sql"],
  "segment-page-two-race": ["segment-eleven-efforts.sql"],
  "segment-partial-race": ["segment-partial-race-samples.sql"],
  heatmap: ["heatmaps.sql"],
};

async function startScenario(testInfo) {
  const scenario = path.basename(testInfo.file, ".spec.mjs");
  const uploads = await mkdtemp(path.join(tmpdir(), "bike-e2e-uploads-"));
  const env = {
    ...process.env,
    DATABASE_URL: process.env.BIKE_E2E_DATABASE_URL,
    LOCAL_ADMIN_ENABLED: "true",
    APP_ENV: "test",
    OAUTH_PROVIDERS: "dev",
    OAUTH_DEV_ENABLED: "true",
    OAUTH_DEV_EMAIL: "developer@bike.local",
    OAUTH_DEV_NAME: "Local Developer",
    OAUTH_DEV_SUBJECT: "bike-local-dev",
    FRONTEND_URL: "http://127.0.0.1:3001",
    CORS_ALLOWED_ORIGINS: "http://127.0.0.1:3001",
    API_URL: "http://127.0.0.1:3000",
    UPLOADS_DIR: uploads,
    OTEL_TRACES_EXPORTER: "none",
    WORKER_POLL_INTERVAL: "1",
  };
  const children = [];
  try {
    await cp(path.join(fixtures, "uploads"), uploads, { recursive: true });
    await run(
      path.join(target, "debug/e2e-fixture"),
      scenarioFixtures[scenario] ?? [],
      { env },
    );
    const api = spawn(path.join(target, "debug/api"), [], {
      env,
      stdio: "inherit",
    });
    children.push(api);
    await waitForService("http://127.0.0.1:3000/api/health", api);
    if (["archive-import-worker", "heatmap"].includes(scenario)) {
      children.push(
        spawn(path.join(target, "debug/worker"), [], {
          env,
          stdio: "inherit",
        }),
      );
    }
    return async () => {
      await Promise.all(children.map(stop));
      await rm(uploads, { recursive: true });
    };
  } catch (error) {
    await Promise.all(children.map(stop));
    await rm(uploads, { recursive: true });
    throw error;
  }
}

export const test = base.extend({
  scenario: [
    async ({ browserName }, runTest, testInfo) => {
      if (browserName !== "chromium") {
        throw new Error("Disposable browser scenarios require Chromium");
      }
      if (process.env.BIKE_E2E_DISPOSABLE !== "1") {
        await runTest();
        return;
      }
      const cleanup = await startScenario(testInfo);
      try {
        await runTest();
      } finally {
        await cleanup();
      }
    },
    { auto: true },
  ],
  browser: async ({ browser }, runTest) => {
    if (process.env.BIKE_E2E_DISPOSABLE !== "1") {
      await runTest(browser);
      return;
    }
    await runTest(
      new Proxy(browser, {
        get(target, property) {
          if (property !== "newContext") {
            const value = Reflect.get(target, property);
            return typeof value === "function" ? value.bind(target) : value;
          }
          return async (options) => {
            const context = await target.newContext(options);
            await fakeExternalBasemaps(context);
            return context;
          };
        },
      }),
    );
  },
});

export { expect };
