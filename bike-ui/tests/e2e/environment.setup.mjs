import { test as setup } from "@playwright/test";
import {
  attachServiceLogs,
  prepareBaseline,
  resetScenario,
} from "./helpers/environment.mjs";

setup(
  "verify backend commands, then snapshot browser fixtures without a worker",
  async ({ request }, testInfo) => {
    setup.setTimeout(120_000);
    await prepareBaseline(request, testInfo);
    await resetScenario("baseline", request, testInfo);
    await attachServiceLogs(testInfo);
  },
);
