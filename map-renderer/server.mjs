import { createHash, timingSafeEqual } from "node:crypto";
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { chromium } from "playwright";
import { createImageCache } from "./cache.mjs";
import { dimensions, imageKey, parseRenderRequest } from "./request.mjs";
import { logError } from "./logging.mjs";
import grpc from "@grpc/grpc-js";
import protoLoader from "@grpc/proto-loader";
import { fileURLToPath } from "node:url";
import { performance } from "node:perf_hooks";
import { SpanStatusCode, trace } from "@opentelemetry/api";
import {
  Counter,
  Gauge,
  Histogram,
  Registry,
  collectDefaultMetrics,
} from "prom-client";
import { shutdownTracing } from "./instrumentation.mjs";

const cacheDir = process.env.MAP_IMAGE_CACHE_DIR ?? "/cache";
const cache = createImageCache(cacheDir);
const styles = new Set(["route-light-v1", "fiord-v1"]);
const serviceToken = process.env.MAP_SERVICE_TOKEN;
const metricRegistry = new Registry();
collectDefaultMetrics({
  register: metricRegistry,
  prefix: "bike_maps_process_",
});
const httpRequests = new Counter({
  name: "bike_maps_http_requests_total",
  help: "Map renderer HTTP requests.",
  labelNames: ["method", "route", "status_code"],
  registers: [metricRegistry],
});
const httpDuration = new Histogram({
  name: "bike_maps_http_request_duration_seconds",
  help: "Map renderer HTTP request duration in seconds.",
  labelNames: ["method", "route", "status_code"],
  buckets: [0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1, 2.5, 5, 10, 30, 60],
  registers: [metricRegistry],
});
const grpcRequests = new Counter({
  name: "bike_maps_grpc_requests_total",
  help: "Map renderer gRPC requests.",
  labelNames: ["method", "status_code"],
  registers: [metricRegistry],
});
const grpcDuration = new Histogram({
  name: "bike_maps_grpc_request_duration_seconds",
  help: "Map renderer gRPC request duration in seconds.",
  labelNames: ["method", "status_code"],
  buckets: [0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1, 2.5, 5, 10, 30, 60],
  registers: [metricRegistry],
});
const cacheHits = new Counter({
  name: "bike_maps_cache_hits_total",
  help: "Cached map images returned.",
  registers: [metricRegistry],
});
const cacheMisses = new Counter({
  name: "bike_maps_cache_misses_total",
  help: "Map renders that missed the image cache.",
  registers: [metricRegistry],
});
const imagesGenerated = new Counter({
  name: "bike_maps_images_generated_total",
  help: "Successfully generated map images after a cache miss.",
  registers: [metricRegistry],
});
const renderFailures = new Counter({
  name: "bike_maps_render_failures_total",
  help: "Map rendering failures by bounded stage.",
  labelNames: ["stage"],
  registers: [metricRegistry],
});
const cacheWriteFailures = new Counter({
  name: "bike_maps_cache_write_failures_total",
  help: "Generated maps that could not be written to cache.",
  registers: [metricRegistry],
});
const pruneDeletions = new Counter({
  name: "bike_maps_cache_pruned_images_total",
  help: "Expired cached images deleted.",
  registers: [metricRegistry],
});
const renderDuration = new Histogram({
  name: "bike_maps_render_duration_seconds",
  help: "Render pipeline duration, including cache hits and misses.",
  labelNames: ["outcome"],
  buckets: [0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1, 2.5, 5, 10, 30, 60],
  registers: [metricRegistry],
});
const queueWait = new Histogram({
  name: "bike_maps_render_queue_wait_seconds",
  help: "Time map renders wait in the serialized browser queue.",
  buckets: [0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1, 2.5, 5, 10, 30, 60],
  registers: [metricRegistry],
});
const queueDepth = new Gauge({
  name: "bike_maps_render_queue_depth",
  help: "Map renders waiting for the browser queue.",
  registers: [metricRegistry],
});
const rendersInFlight = new Gauge({
  name: "bike_maps_renders_in_flight",
  help: "Map renders currently using the browser.",
  registers: [metricRegistry],
});
const cacheDiskUsage = new Gauge({
  name: "bike_maps_cache_disk_usage_bytes",
  help: "Bytes used by files in the map cache directory.",
  registers: [metricRegistry],
});
const cacheImageCount = new Gauge({
  name: "bike_maps_cache_image_count",
  help: "PNG images currently in the map cache directory.",
  registers: [metricRegistry],
});
const tracer = trace.getTracer("bike-map-renderer");

async function refreshCacheMetrics() {
  const stats = await cache.stats();
  cacheDiskUsage.set(stats.bytes);
  cacheImageCount.set(stats.images);
}
const browser = await chromium.launch({
  headless: true,
  args: ["--no-sandbox", "--use-gl=angle", "--use-angle=swiftshader"],
});
await cache.init();
pruneDeletions.inc(await cache.prune());
await refreshCacheMetrics();
let renderQueue = Promise.resolve();
setInterval(
  () => {
    renderQueue = renderQueue
      .then(async () => {
        pruneDeletions.inc(await cache.prune());
        await refreshCacheMetrics();
      })
      .catch((error) => {
        logError("Map cache cleanup failed", error, { stage: "cache_cleanup" });
      });
  },
  60 * 60 * 1000,
).unref();

function authorized(authorization) {
  if (!serviceToken) return true;
  const supplied = authorization?.replace(/^Bearer /, "") ?? "";
  const digest = (value) => createHash("sha256").update(value).digest();
  return timingSafeEqual(digest(supplied), digest(serviceToken));
}

async function render(payload) {
  const started = performance.now();
  let outcome = "error";
  return tracer.startActiveSpan("bike.maps.render", async (span) => {
    span.setAttribute("map.variant", payload.variant);
    span.setAttribute("map.style", payload.style);
    let stage = "cache_read";
    let page;
    try {
      const [width, height] = dimensions[payload.variant];
      const key = imageKey(payload);
      const cached = await tracer.startActiveSpan(
        "bike.maps.cache.lookup",
        async (cacheSpan) => {
          try {
            return await cache.read(key);
          } catch (error) {
            cacheSpan.recordException(error);
            cacheSpan.setStatus({ code: SpanStatusCode.ERROR });
            throw error;
          } finally {
            cacheSpan.end();
          }
        },
      );
      if (cached) {
        cacheHits.inc();
        outcome = "hit";
        span.setAttribute("cache.outcome", outcome);
        return { png: cached, cacheStatus: "hit" };
      }

      cacheMisses.inc();
      span.setAttribute("cache.outcome", "miss");
      stage = "browser_render";
      page = await browser.newPage({
        viewport: { width, height },
        deviceScaleFactor: payload.dpr,
      });
      page.on("pageerror", (error) =>
        logError("Map page error", error, { stage: "browser_page" }),
      );
      await tracer.startActiveSpan(
        "bike.maps.browser_render",
        async (browserSpan) => {
          try {
            await page.goto("http://127.0.0.1:3100/", {
              waitUntil: "domcontentloaded",
            });
            await page.waitForFunction(
              () => typeof window.renderMap === "function",
            );
            await page.evaluate(
              (data) =>
                Promise.race([
                  window.renderMap(data),
                  new Promise((_, reject) =>
                    setTimeout(
                      () => reject(new Error("Map render timed out")),
                      45000,
                    ),
                  ),
                ]),
              payload,
            );
          } catch (error) {
            browserSpan.recordException(error);
            browserSpan.setStatus({ code: SpanStatusCode.ERROR });
            throw error;
          } finally {
            browserSpan.end();
          }
        },
      );

      stage = "screenshot";
      const png = await tracer.startActiveSpan(
        "bike.maps.screenshot",
        async (screenshotSpan) => {
          try {
            return await page.screenshot({ type: "png" });
          } catch (error) {
            screenshotSpan.recordException(error);
            screenshotSpan.setStatus({ code: SpanStatusCode.ERROR });
            throw error;
          } finally {
            screenshotSpan.end();
          }
        },
      );
      imagesGenerated.inc();
      outcome = "miss";

      stage = "cache_write";
      try {
        await tracer.startActiveSpan(
          "bike.maps.cache.write",
          async (cacheSpan) => {
            try {
              await cache.write(key, png);
            } catch (error) {
              cacheSpan.recordException(error);
              cacheSpan.setStatus({ code: SpanStatusCode.ERROR });
              throw error;
            } finally {
              cacheSpan.end();
            }
          },
        );
        void refreshCacheMetrics().catch((error) =>
          logError("Map cache stats failed", error, { stage: "cache_stats" }),
        );
      } catch (error) {
        cacheWriteFailures.inc();
        renderFailures.inc({ stage: "cache_write" });
        logError("Map image cache write failed", error, {
          stage: "cache_write",
        });
      }
      return { png, cacheStatus: "miss" };
    } catch (error) {
      renderFailures.inc({ stage });
      span.recordException(error);
      span.setStatus({ code: SpanStatusCode.ERROR });
      throw error;
    } finally {
      if (page) {
        try {
          await page.close();
        } catch (error) {
          span.recordException(error);
          logError("Map browser page cleanup failed", error, {
            stage: "browser_cleanup",
          });
        }
      }
      renderDuration.observe({ outcome }, (performance.now() - started) / 1000);
      span.end();
    }
  });
}

function enqueueRender(payload) {
  const enqueuedAt = performance.now();
  queueDepth.inc();
  const pending = renderQueue.then(async () => {
    queueDepth.dec();
    rendersInFlight.inc();
    const waitSeconds = (performance.now() - enqueuedAt) / 1000;
    queueWait.observe(waitSeconds);
    try {
      return await tracer.startActiveSpan(
        "bike.maps.render_queue_wait",
        async (span) => {
          span.setAttribute("queue.wait_seconds", waitSeconds);
          try {
            return await render(payload);
          } finally {
            span.end();
          }
        },
      );
    } finally {
      rendersInFlight.dec();
    }
  });
  renderQueue = pending.catch(() => {});
  return pending;
}

const protoPath =
  process.env.MAP_SERVICE_PROTO_PATH ??
  fileURLToPath(new URL("../proto/bike/maps/v1/maps.proto", import.meta.url));
const mapsPackage = grpc.loadPackageDefinition(
  protoLoader.loadSync(protoPath, { defaults: true }),
).bike.maps.v1;
const grpcServer = new grpc.Server({
  "grpc.max_receive_message_length": 10_000_000,
  "grpc.max_send_message_length": 20_000_000,
});
grpcServer.addService(mapsPackage.MapService.service, {
  render(call, callback) {
    const started = performance.now();
    let statusCode = grpc.status.OK;
    const recordRequest = () => {
      const labels = { method: "Render", status_code: String(statusCode) };
      grpcRequests.inc(labels);
      grpcDuration.observe(labels, (performance.now() - started) / 1000);
    };
    if (!authorized(call.metadata.get("authorization")[0])) {
      statusCode = grpc.status.UNAUTHENTICATED;
      recordRequest();
      callback({ code: grpc.status.UNAUTHENTICATED, message: "Unauthorized" });
      return;
    }
    const payload = parseRenderRequest(call.request);
    if (!payload) {
      statusCode = grpc.status.INVALID_ARGUMENT;
      recordRequest();
      callback({
        code: grpc.status.INVALID_ARGUMENT,
        message: "Invalid map request",
      });
      return;
    }
    enqueueRender(payload).then(
      ({ png, cacheStatus }) => {
        recordRequest();
        callback(null, { png, cacheHit: cacheStatus === "hit" });
      },
      (error) => {
        statusCode = grpc.status.INTERNAL;
        recordRequest();
        logError("gRPC map rendering failed", error, { stage: "grpc_render" });
        callback({
          code: grpc.status.INTERNAL,
          message: "Map rendering failed",
        });
      },
    );
  },
});
await new Promise((resolve, reject) => {
  grpcServer.bindAsync(
    "0.0.0.0:50051",
    grpc.ServerCredentials.createInsecure(),
    (error) => (error ? reject(error) : resolve()),
  );
});

const server = createServer(async (request, response) => {
  const pathname = new URL(request.url, "http://localhost").pathname;
  const started = performance.now();
  const route =
    pathname === "/render"
      ? "/render"
      : pathname === "/healthz"
        ? "/healthz"
        : pathname === "/"
          ? "/"
          : "/static";
  response.once("finish", () => {
    if (
      pathname === "/healthz" ||
      pathname === "/metrics" ||
      pathname === "/" ||
      pathname.startsWith("/vendor/") ||
      pathname.startsWith("/styles/")
    )
      return;
    const method = ["GET", "POST", "PUT", "PATCH", "DELETE"].includes(
      request.method,
    )
      ? request.method
      : "OTHER";
    const labels = { method, route, status_code: String(response.statusCode) };
    httpRequests.inc(labels);
    httpDuration.observe(labels, (performance.now() - started) / 1000);
  });
  if (request.method === "GET" && pathname === "/healthz") {
    response.writeHead(200).end("ok");
    return;
  }
  if (request.method === "GET" && pathname === "/metrics") {
    response
      .writeHead(200, { "Content-Type": metricRegistry.contentType })
      .end(await metricRegistry.metrics());
    return;
  }
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
  if (!authorized(request.headers.authorization)) {
    response.writeHead(401).end();
    return;
  }
  try {
    const chunks = [];
    let size = 0;
    for await (const chunk of request) {
      size += chunk.length;
      if (size > 10_000_000) {
        response.writeHead(413).end("Payload too large");
        return;
      }
      chunks.push(chunk);
    }
    let rawPayload;
    try {
      rawPayload = JSON.parse(Buffer.concat(chunks).toString("utf8"));
    } catch {
      response.writeHead(400).end("Invalid JSON");
      return;
    }
    const payload = parseRenderRequest(rawPayload);
    if (!payload) {
      response.writeHead(400).end("Invalid map request");
      return;
    }
    const { png, cacheStatus } = await enqueueRender(payload);
    response
      .writeHead(200, {
        "Content-Type": "image/png",
        "Cache-Control": "no-store",
        "X-Map-Cache": cacheStatus,
      })
      .end(png);
  } catch (error) {
    logError("Map render request failed", error, { stage: "http_render" });
    response.writeHead(500).end("Map rendering failed");
  }
});
server.listen(3100, "0.0.0.0");

const metricsPort = Number.parseInt(process.env.METRICS_PORT ?? "9090", 10);
if (
  !Number.isSafeInteger(metricsPort) ||
  metricsPort < 1 ||
  metricsPort > 65535
) {
  throw new Error("METRICS_PORT must be a valid TCP port");
}
const metricsServer = createServer(async (request, response) => {
  if (request.method === "GET" && request.url === "/metrics") {
    response.writeHead(200, { "Content-Type": metricRegistry.contentType });
    response.end(await metricRegistry.metrics());
    return;
  }
  if (request.method === "GET" && request.url === "/healthz") {
    response.writeHead(200).end("ok");
    return;
  }
  response.writeHead(404).end();
});
metricsServer.listen(metricsPort, "0.0.0.0");

async function shutdown() {
  await Promise.all([
    new Promise((resolve) => server.close(resolve)),
    new Promise((resolve) => metricsServer.close(resolve)),
    new Promise((resolve) => grpcServer.tryShutdown(resolve)),
    browser.close(),
    shutdownTracing(),
  ]);
}

for (const signalName of ["SIGTERM", "SIGINT"]) {
  process.once(signalName, () => {
    void shutdown().finally(() => process.exit(0));
  });
}
