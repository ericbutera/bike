import { test as base, expect } from "@playwright/test";
import { attachServiceLogs, resetScenario } from "./environment.mjs";
import { fakeExternalBasemaps } from "./external-providers.mjs";

function observeContext(context, diagnostics) {
  context.on("page", (page) => {
    page.on("console", (message) => {
      if (["warning", "error"].includes(message.type()))
        diagnostics.push(`${message.type()}: ${message.text()}`);
    });
    page.on("pageerror", (error) =>
      diagnostics.push(`pageerror: ${error.stack}`),
    );
  });
}

export const test = base.extend({
  expectedConsoleErrors: [[], { option: true }],
  browserDiagnostics: [
    async ({ expectedConsoleErrors }, runTest, testInfo) => {
      const diagnostics = [];
      await runTest(diagnostics);
      await testInfo.attach("browser-diagnostics", {
        body: JSON.stringify(diagnostics, null, 2),
        contentType: "application/json",
      });
      if (process.env.BIKE_E2E_PROJECT && testInfo.project.name === "mocked")
        await attachServiceLogs(testInfo);
      expect(
        diagnostics,
        "Browser diagnostics must match the negative case's exact asserted errors",
      ).toEqual(expectedConsoleErrors.map((message) => `error: ${message}`));
    },
    { auto: true },
  ],
  context: async ({ context, browserDiagnostics }, runTest) => {
    observeContext(context, browserDiagnostics);
    await fakeExternalBasemaps(context);
    await runTest(context);
  },
  createContext: async (
    { browser, browserDiagnostics, storageState },
    runTest,
  ) => {
    const contexts = [];
    await runTest(async (options = {}) => {
      const context = await browser.newContext({ storageState, ...options });
      observeContext(context, browserDiagnostics);
      await fakeExternalBasemaps(context);
      contexts.push(context);
      return context;
    });
    for (const context of contexts) await context.close();
  },
});

export const connectedTest = test.extend({
  scenario: ["baseline", { option: true }],
  session: [
    async ({ scenario, playwright }, runTest, testInfo) => {
      // An administration context cannot depend on the session it is creating.
      const request = await playwright.request.newContext({
        storageState: { cookies: [], origins: [] },
      });
      try {
        const session = await resetScenario(scenario, request, testInfo);
        await runTest(session);
      } finally {
        await request.dispose();
        await attachServiceLogs(testInfo);
      }
    },
    { auto: true, timeout: 120_000 },
  ],
  storageState: async ({ session }, runTest) => runTest(session),
});

// The production monitor retains its explicit, read-only external environment.
export const platformTest = process.env.BIKE_E2E_PROJECT ? connectedTest : test;

export { expect };
