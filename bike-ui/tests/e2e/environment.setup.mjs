import { test as setup } from "@playwright/test";
import {
  attachServiceLogs,
  prepareBaseline,
  resetScenario,
} from "./helpers/environment.mjs";

setup(
  "prepare fixtures for product browser acceptance",
  async ({ request }, testInfo) => {
    setup.setTimeout(120_000);
    await prepareBaseline(request, testInfo);
    await resetScenario("baseline", request, testInfo);
    await attachServiceLogs(testInfo);
  },
);
