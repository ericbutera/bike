import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import { setTimeout as sleep } from "node:timers/promises";
import { PNG } from "pngjs";
import grpc from "@grpc/grpc-js";
import { loadSync } from "@grpc/proto-loader";
import { fileURLToPath } from "node:url";

const definition = loadSync(
  fileURLToPath(
    new URL("../../proto/bike/maps/v1/maps.proto", import.meta.url),
  ),
);
const { MapService } = grpc.loadPackageDefinition(definition).bike.maps.v1;
const client = new MapService(
  process.env.PARITY_GO_ADDRESS ?? "golang:50051",
  grpc.credentials.createInsecure(),
);

const endpoints = {
  golang: "grpc",
  ...(process.env.PARITY_BASELINE_URL
    ? { legacy: process.env.PARITY_BASELINE_URL }
    : {}),
};
const token = process.env.MAP_SERVICE_TOKEN;
const headers = {
  "Content-Type": "application/json",
  ...(token ? { Authorization: `Bearer ${token}` } : {}),
};
const points = [
  { latitude: 44.7631, longitude: -85.6206 },
  { latitude: 44.769, longitude: -85.59 },
  { latitude: 44.782, longitude: -85.576 },
];

async function ready(url) {
  if (url === "grpc") {
    await new Promise((resolve, reject) =>
      client.waitForReady(Date.now() + 30_000, (error) =>
        error ? reject(error) : resolve(),
      ),
    );
    return;
  }
  let lastError;
  for (let attempt = 0; attempt < 120; attempt++) {
    try {
      const response = await fetch(`${url}/healthz`);
      if (response.ok) return;
      lastError = new Error(`Health status ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await sleep(250);
  }
  throw lastError;
}

async function render(url, request) {
  const started = performance.now();
  const response =
    url === "grpc"
      ? undefined
      : await fetch(`${url}/render`, {
          method: "POST",
          headers,
          body: JSON.stringify(request),
        });
  if (response) {
    assert.equal(response.status, 200, await response.clone().text());
    assert.equal(response.headers.get("content-type"), "image/png");
    assert.equal(response.headers.get("cache-control"), "no-store");
  }
  const png = response
    ? Buffer.from(await response.arrayBuffer())
    : await snapshot(request);
  const decoded = PNG.sync.read(png);
  const [width, height] = request.variant === "full" ? [1000, 300] : [288, 192];
  assert.equal(decoded.width, width * request.dpr);
  assert.equal(decoded.height, height * request.dpr);
  let routePixels = 0;
  for (let i = 0; i < decoded.data.length; i += 4) {
    const [red, green, blue] = decoded.data.subarray(i, i + 3);
    if (blue > red + 100 && blue > green + 50) routePixels++;
  }
  assert.ok(routePixels > 100, "Rendered PNG has no route");
  return {
    png,
    decoded,
    duration_ms: performance.now() - started,
    cache: response?.headers.get("x-map-cache") ?? null,
  };
}

async function snapshot(request, authenticated = true) {
  const metadata = new grpc.Metadata();
  if (authenticated && token) metadata.set("authorization", `Bearer ${token}`);
  const result = await new Promise((resolve, reject) =>
    client.render(
      request,
      metadata,
      { deadline: Date.now() + 60_000 },
      (error, response) => (error ? reject(error) : resolve(response)),
    ),
  );
  assert.equal(result.cacheHit ?? false, false);
  return result.png;
}

async function capture(url, request) {
  const miss = await render(url, request);
  const legacy = url === endpoints.legacy;
  assert.equal(miss.cache, legacy ? "miss" : null);
  const hit = await render(url, request);
  assert.equal(hit.cache, legacy ? "hit" : null);
  assert.deepEqual(miss.decoded.data, hit.decoded.data);
  return { ...miss, hit_ms: hit.duration_ms };
}

for (const url of Object.values(endpoints)) {
  await ready(url);
  if (token) {
    if (url === "grpc") {
      await assert.rejects(
        snapshot({}, false),
        (error) => error.code === grpc.status.UNAUTHENTICATED,
      );
    } else {
      const response = await fetch(`${url}/render`, { method: "POST" });
      assert.equal(response.status, 401);
    }
  }
}
const cases =
  process.env.PARITY_MODE === "live"
    ? [
        { theme: "light", variant: "full", dpr: 1 },
        { theme: "dark", variant: "thumbnail", dpr: 2 },
      ]
    : ["light", "dark"].flatMap((theme) =>
        ["thumbnail", "full"].flatMap((variant) =>
          [1, 2].map((dpr) => ({ theme, variant, dpr })),
        ),
      );
const measurements = [];
for (const options of cases) {
  const request = { ...options, points };
  const golang = await capture(endpoints.golang, request);
  const legacy = endpoints.legacy
    ? await capture(endpoints.legacy, request)
    : undefined;
  if (legacy) {
    assert.deepEqual(
      golang.decoded.data,
      legacy.decoded.data,
      "PNG pixels differ",
    );
  }
  measurements.push({
    ...options,
    golang: {
      miss_ms: golang.duration_ms,
      hit_ms: golang.hit_ms,
      png_bytes: golang.png.length,
    },
    ...(legacy
      ? {
          legacy: {
            miss_ms: legacy.duration_ms,
            hit_ms: legacy.hit_ms,
            png_bytes: legacy.png.length,
          },
        }
      : {}),
  });
  console.log(
    `${options.theme}/${options.variant}/${options.dpr}x: dimensions, route, repeat snapshot${legacy ? " and exact pixels" : ""} passed`,
  );
}

await sleep(2100);
for (const url of Object.values(endpoints)) {
  assert.equal(
    (await render(url, { ...cases[0], points })).cache,
    url === endpoints.legacy ? "miss" : null,
  );
}
if (process.env.PARITY_OUTPUT) {
  await writeFile(
    process.env.PARITY_OUTPUT,
    JSON.stringify(measurements, null, 2) + "\n",
  );
}
client.close();
