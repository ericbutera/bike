import { randomUUID } from "node:crypto";
import { PNG } from "pngjs";
import { connectedTest as test, expect } from "./helpers/test.mjs";
import { environmentCommand } from "./helpers/environment.mjs";
import {
  heatmapPolicyData,
  heatmapPaginationData,
  sessionData,
} from "./helpers/seeds.mjs";

test.use({ scenario: "heatmap" });
const api = process.env.BIKE_API_URL;

async function metadata(request, filters = {}) {
  const response = await request.get(`${api}/maps/heatmap`, {
    params: filters,
  });
  expect(response.status()).toBe(200);
  return response.json();
}

async function zones(request, state, filters = {}) {
  const response = await request.get(`${api}/maps/heatmap/zones`, {
    params: { revision: state.revision, ...filters },
  });
  expect(response.status()).toBe(200);
  return (await response.json()).zones;
}

async function tile(request, state, filters = {}, path = "0/0/0.png") {
  const response = await request.get(`${api}/maps/heatmap/tiles/${path}`, {
    params: { revision: state.revision, ...filters },
  });
  expect(response.status()).toBe(200);
  return { png: PNG.sync.read(await response.body()), response };
}

async function expectStale(request, state, etag) {
  const params = { revision: state.revision };
  const centers = await request.get(`${api}/maps/heatmap/zones`, { params });
  expect(centers.status()).toBe(409);
  const cached = await request.get(`${api}/maps/heatmap/tiles/0/0/0.png`, {
    params,
    headers: { "if-none-match": etag },
  });
  expect(cached.status()).toBe(409);
}

test("heatmap reads isolate users and exclude nonrides, unavailable and virtual recordings", async ({
  request,
  playwright,
}) => {
  const original = await tile(request, await metadata(request));
  await environmentCommand("seed", JSON.stringify(heatmapPolicyData()));
  const owner = await metadata(request);
  expect(owner).toMatchObject({ ready: 2, pending: 0, skipped: 6 });
  expect(owner.bounds[0]).toBeCloseTo(-83);
  expect(owner.bounds[1]).toBeCloseTo(42);
  expect((await zones(request, owner)).length).toBe(2);
  const current = await tile(request, owner);
  expect(current.png.data).toEqual(original.png.data);

  const filters = {
    sport: "road_ride",
    from: "2025-06-01T00:00:00Z",
    to: "2025-06-02T00:00:00Z",
  };
  const period = await metadata(request, filters);
  expect(period.ready).toBe(1);
  expect((await zones(request, period, filters)).length).toBe(1);

  const token = randomUUID();
  await environmentCommand("seed", JSON.stringify(sessionData(token, 2)));
  const other = await playwright.request.newContext({
    storageState: { cookies: [], origins: [] },
    extraHTTPHeaders: { cookie: `refresh_token=${token}` },
  });
  try {
    const state = await metadata(other);
    expect(state).toMatchObject({ ready: 1, pending: 0, skipped: 0 });
    expect(state.bounds[0]).toBeCloseTo(-103);
    expect((await zones(other, state)).length).toBe(1);
    const otherTile = await tile(other, state);
    expect(otherTile.png.data).not.toEqual(current.png.data);
    expect((await metadata(request)).ready).toBe(2);
  } finally {
    await other.dispose();
  }
});

test("heatmap tiles include the last chunk after a full database page", async ({
  request,
}) => {
  await environmentCommand("seed", JSON.stringify(heatmapPaginationData()));
  const filters = { sport: "mountain_bike" };
  const state = await metadata(request, filters);
  expect(state.ready).toBe(1);
  expect((await zones(request, state, filters)).length).toBe(1);
  const { png } = await tile(request, state, filters, "14/4000/6000.png");
  for (const y of [128, 384]) {
    const offset = (y * png.width + 256) * 4;
    expect(
      png.data[offset + 3],
      `page paints the route at y=${y}`,
    ).toBeGreaterThan(0);
    expect([...png.data.subarray(offset, offset + 3)]).toEqual([0, 96, 223]);
  }
});

test("editing and deleting activities invalidate cached tiles and route centers", async ({
  request,
}) => {
  const before = await metadata(request);
  const cached = await tile(request, before);
  expect((await zones(request, before)).length).toBe(2);
  const edited = await request.patch(`${api}/activities/1109`, {
    data: { title: "Zwift excluded recording", activity_type: "training" },
  });
  expect(edited.status()).toBe(200);
  await expectStale(request, before, cached.response.headers().etag);
  const changed = await metadata(request);
  expect(changed.revision).not.toBe(before.revision);
  expect(changed).toMatchObject({ ready: 1, pending: 1 });
  expect((await zones(request, changed)).length).toBe(1);
  const updated = await tile(request, changed);
  expect(updated.response.headers().etag).not.toBe(
    cached.response.headers().etag,
  );

  const deleted = await request.delete(`${api}/activities/1110`);
  expect(deleted.status()).toBe(204);
  await expectStale(request, changed, updated.response.headers().etag);
  const empty = await metadata(request);
  expect(empty).toMatchObject({ ready: 0, pending: 1, bounds: null });
  expect(await zones(request, empty)).toEqual([]);
  const transparent = await tile(request, empty);
  expect(
    transparent.png.data.every(
      (value, index) => index % 4 !== 3 || value === 0,
    ),
  ).toBe(true);
});
