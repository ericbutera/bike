import { createHash } from "node:crypto";
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { chromium } from "playwright";
import { createImageCache } from "./cache.mjs";

const cacheDir = process.env.MAP_IMAGE_CACHE_DIR ?? "/cache";
const cache = createImageCache(cacheDir);
const styles = new Set(["route-light-v1", "fiord-v1"]);
const dimensions = { thumbnail: [288, 192], full: [1000, 300] };
const browser = await chromium.launch({
  headless: true,
  args: ["--no-sandbox", "--use-gl=angle", "--use-angle=swiftshader"],
});
await cache.init();
await cache.prune();
let renderQueue = Promise.resolve();
setInterval(
  () => {
    renderQueue = renderQueue
      .then(() => cache.prune())
      .catch((error) => {
        console.error("Map cache cleanup failed:", error);
      });
  },
  60 * 60 * 1000,
).unref();

function validPayload(payload) {
  return (
    payload &&
    (payload.theme === "light" || payload.theme === "dark") &&
    dimensions[payload.variant] &&
    (payload.dpr === 1 || payload.dpr === 2) &&
    typeof payload.scope === "string" &&
    payload.scope.length === 64 &&
    Array.isArray(payload.points) &&
    payload.points.length >= 2 &&
    payload.points.length <= 100000 &&
    payload.points.every(
      (point) =>
        point &&
        Number.isFinite(point.latitude) &&
        Number.isFinite(point.longitude) &&
        Math.abs(point.latitude) <= 90 &&
        Math.abs(point.longitude) <= 180,
    )
  );
}

async function render(payload) {
  const [width, height] = dimensions[payload.variant];
  const key = createHash("sha256")
    .update(JSON.stringify({ revision: 1, width, height, ...payload }))
    .digest("hex");
  const cached = await cache.read(key);
  if (cached) return { png: cached, cacheStatus: "hit" };
  const page = await browser.newPage({
    viewport: { width, height },
    deviceScaleFactor: payload.dpr,
  });
  page.on("pageerror", (error) => console.error("Map page error:", error));
  try {
    await page.goto("http://127.0.0.1:3100/", {
      waitUntil: "domcontentloaded",
    });
    await page.waitForFunction(() => typeof window.renderMap === "function");
    await page.evaluate(
      (data) =>
        Promise.race([
          window.renderMap(data),
          new Promise((_, reject) =>
            setTimeout(() => reject(new Error("Map render timed out")), 45000),
          ),
        ]),
      payload,
    );
    const png = await page.screenshot({ type: "png" });
    try {
      await cache.write(key, png);
    } catch (error) {
      console.error("Map image cache write failed:", error);
    }
    return { png, cacheStatus: "miss" };
  } finally {
    await page.close();
  }
}

const server = createServer(async (request, response) => {
  const pathname = new URL(request.url, "http://localhost").pathname;
  if (request.method === "GET" && pathname === "/") {
    response
      .writeHead(200, { "Content-Type": "text/html" })
      .end(await readFile("/app/index.html"));
    return;
  }
  if (request.method === "GET" && pathname === "/vendor/maplibre-gl.css") {
    response
      .writeHead(200, { "Content-Type": "text/css" })
      .end(
        await readFile("/app/node_modules/maplibre-gl/dist/maplibre-gl.css"),
      );
    return;
  }
  if (
    request.method === "GET" &&
    /^\/vendor\/maplibre-gl(?:-shared|-worker)?\.mjs$/.test(pathname)
  ) {
    const file = pathname.slice("/vendor/".length);
    response
      .writeHead(200, { "Content-Type": "text/javascript" })
      .end(await readFile(join("/app/node_modules/maplibre-gl/dist", file)));
    return;
  }
  if (
    request.method === "GET" &&
    /^\/styles\/(route-light-v1|fiord-v1)\.json$/.test(pathname)
  ) {
    const name = pathname.slice(8, -5);
    if (styles.has(name)) {
      response
        .writeHead(200, { "Content-Type": "application/json" })
        .end(await readFile(join("/app/styles", `${name}.json`)));
      return;
    }
  }
  if (request.method !== "POST" || pathname !== "/render") {
    response.writeHead(404).end();
    return;
  }
  try {
    let body = "";
    for await (const chunk of request) {
      body += chunk;
      if (body.length > 10_000_000) throw new Error("Payload too large");
    }
    const payload = JSON.parse(body);
    if (!validPayload(payload)) {
      response.writeHead(400).end("Invalid map request");
      return;
    }
    const pending = renderQueue.then(() => render(payload));
    renderQueue = pending.catch(() => {});
    const { png, cacheStatus } = await pending;
    response
      .writeHead(200, {
        "Content-Type": "image/png",
        "Cache-Control": "no-store",
        "X-Map-Cache": cacheStatus,
      })
      .end(png);
  } catch (error) {
    console.error(error);
    response.writeHead(500).end("Map rendering failed");
  }
});
server.listen(3100, "0.0.0.0");
