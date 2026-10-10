import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { randomUUID } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { expect } from "@playwright/test";
import {
  baselineData,
  scenarioData,
  scenarios,
  sessionData,
} from "./seeds.mjs";

const execute = promisify(execFile);
const identityFile = ".artifacts/playwright/runtime.json";
const attemptStarts = new WeakMap();

export async function environmentCommand(...args) {
  if (!process.env.BIKE_E2E_PROJECT)
    throw new Error("Disposable connected tests require mise run e2e");
  const result = await execute(
    "bash",
    ["/workspace/scripts/e2e-state.sh", ...args],
    {
      timeout: 120_000,
      maxBuffer: 8 * 1024 * 1024,
    },
  );
  expect(
    result.stdout + result.stderr,
    "Fixture command diagnostics",
  ).not.toMatch(/warning:|notice:|fatal:|error:|"level":"(?:warn|error)"/i);
  return result;
}

function composeArgs(...args) {
  return [
    "compose",
    "--project-name",
    process.env.BIKE_E2E_PROJECT,
    "--file",
    "/workspace/compose.runtime.yaml",
    "--file",
    "/workspace/compose.e2e.yaml",
    ...args,
  ];
}

async function runtimeIdentity() {
  const { stdout: ids } = await execute(
    "docker",
    composeArgs("ps", "--all", "--quiet"),
  );
  const { stdout } = await execute("docker", [
    "inspect",
    ...ids.trim().split(/\s+/),
  ]);
  const containers = JSON.parse(stdout)
    .map((container) => ({
      service: container.Config.Labels["com.docker.compose.service"],
      id: container.Id,
      startedAt: container.State.StartedAt,
      restarts: container.RestartCount,
      running: container.State.Running,
    }))
    .sort((first, second) => first.service.localeCompare(second.service));
  expect(containers.map((container) => container.service)).not.toContain(
    "worker",
  );
  for (const service of ["postgres", "bike-rs", "ui", "bike-maps"])
    expect(
      containers.find((container) => container.service === service)?.running,
      `${service} remains running`,
    ).toBe(true);
  const { stdout: database } = await environmentCommand("identity");
  return { containers, database };
}

async function waitForServices(request) {
  for (const url of [
    "http://api.e2e.test:3000/api/health",
    "http://ui.e2e.test:3000",
  ]) {
    await expect
      .poll(
        async () => {
          try {
            return (await request.get(url, { timeout: 2_000 })).status();
          } catch {
            return 0;
          }
        },
        { message: `Service ready at ${url}`, timeout: 60_000 },
      )
      .toBe(200);
  }
  await expect
    .poll(
      async () => {
        try {
          await environmentCommand("health");
          return true;
        } catch {
          return false;
        }
      },
      { message: "Snapshot worker gRPC health", timeout: 60_000 },
    )
    .toBe(true);
}

export async function prepareBaseline(request, testInfo) {
  attemptStarts.set(testInfo, new Date().toISOString());
  const started = performance.now();
  await environmentCommand("prepare");
  await environmentCommand("seed", JSON.stringify(baselineData()));
  await environmentCommand("snapshot", "baseline");
  // Each named variant is built once by Playwright and snapshotted from the
  // current migrated schema. Attempts restore data, never re-run seed scripts.
  for (const scenario of scenarios.filter((name) => name !== "baseline")) {
    await environmentCommand("restore", "baseline");
    await environmentCommand("seed", JSON.stringify(scenarioData(scenario)));
    await environmentCommand("snapshot", scenario);
  }
  await waitForServices(request);
  await writeFile(
    identityFile,
    JSON.stringify(await runtimeIdentity(), null, 2),
  );
  await testInfo.attach("baseline-timing", {
    body: JSON.stringify({ setupMs: performance.now() - started }),
    contentType: "application/json",
  });
}

export async function resetScenario(scenario, request, testInfo) {
  if (!scenarios.includes(scenario))
    throw new Error(`Unknown E2E scenario: ${scenario}`);
  if (!attemptStarts.has(testInfo))
    attemptStarts.set(testInfo, new Date().toISOString());
  const started = performance.now();
  const token = randomUUID();
  // Millisecond epoch provides non-overlapping revision ranges across attempts
  // even if snapshot data repeats, keeping the live API tile cache coherent.
  const revision = Date.now() * 1000;
  const result = await environmentCommand(
    "restore",
    scenario,
    String(revision),
  );
  const restored = performance.now();
  await environmentCommand("seed", JSON.stringify(sessionData(token)));
  await environmentCommand("files");
  const seeded = performance.now();
  await testInfo.attach("scenario-setup", {
    body: result.stdout + result.stderr,
    contentType: "text/plain",
  });
  const headers = { cookie: `refresh_token=${token}` };
  const response = await request.get(
    "http://api.e2e.test:3000/api/auth/current",
    { headers },
  );
  expect(response.status(), "Normal session authentication").toBe(200);
  expect(await response.json()).toMatchObject({ is_admin: true });
  for (const [key, enabled] of [
    ["enhanced_maps", true],
    ["heatmaps", scenario === "heatmap"],
  ]) {
    const flag = await request.post(
      `http://api.e2e.test:3000/api/admin/feature-flags/${key}`,
      { headers, data: { enabled } },
    );
    expect(flag.status(), `Normal flag update refreshes ${key} cache`).toBe(
      200,
    );
  }
  if (scenario === "heatmap") {
    const metadata = await request.get(
      "http://api.e2e.test:3000/api/maps/heatmap",
      { headers },
    );
    expect(metadata.status()).toBe(200);
    expect(await metadata.json()).toMatchObject({ ready: 2, pending: 0 });
  }
  await testInfo.attach("scenario-timing", {
    body: JSON.stringify({
      restoreMs: restored - started,
      sessionAndFilesMs: seeded - restored,
      totalMs: performance.now() - started,
    }),
    contentType: "application/json",
  });
  return {
    cookies: [
      {
        name: "refresh_token",
        value: token,
        domain: ".e2e.test",
        path: "/",
        httpOnly: true,
        secure: false,
        sameSite: "Strict",
        expires: Math.floor(Date.now() / 1000) + 24 * 60 * 60,
      },
    ],
    origins: [],
  };
}

export async function attachServiceLogs(testInfo) {
  const since = attemptStarts.get(testInfo);
  const { stdout, stderr } = await execute(
    "docker",
    composeArgs(
      "logs",
      "--no-color",
      "--timestamps",
      ...(since ? ["--since", since] : []),
    ),
  );
  await testInfo.attach("services", {
    body: stdout + stderr,
    contentType: "text/plain",
  });
  expect(stdout + stderr, "Services must not emit diagnostic logs").not.toMatch(
    /warning:|notice:|fatal:|error:|"level":"(?:warn|error)"|⨯|unhandledRejection/i,
  );
  const result = await environmentCommand("verify");
  expect(result.stderr, "Snapshot verification diagnostics").toBe("");
  const identity = await runtimeIdentity();
  expect(
    identity,
    "Containers and database objects remain identical across attempts",
  ).toEqual(JSON.parse(await readFile(identityFile, "utf8")));
  await testInfo.attach("runtime-identity", {
    body: JSON.stringify(identity),
    contentType: "application/json",
  });
}
