import { connectedTest as test, expect } from "./helpers/test.mjs";
import { resetScenario, environmentCommand } from "./helpers/environment.mjs";

test("mutated data resets while services and pooled database objects stay alive", async ({
  request,
  playwright,
}, testInfo) => {
  const before = await environmentCommand("identity");
  const deleted = await request.delete(
    `${process.env.BIKE_API_URL}/activities/1109`,
  );
  expect(deleted.ok()).toBe(true);
  const changed = await request.get(`${process.env.BIKE_API_URL}/activities`);
  expect(changed.ok()).toBe(true);
  expect(
    (await changed.json()).data.map((activity) => activity.id),
  ).not.toContain(1109);

  const storageState = await resetScenario("baseline", request, testInfo);
  const restored = await playwright.request.newContext({ storageState });
  try {
    const activities = await restored.get(
      `${process.env.BIKE_API_URL}/activities`,
    );
    expect(activities.ok()).toBe(true);
    expect(
      (await activities.json()).data.map((activity) => activity.id).sort(),
    ).toEqual([1109, 1110]);
    expect((await environmentCommand("identity")).stdout).toBe(before.stdout);
  } finally {
    await restored.dispose();
  }
});
