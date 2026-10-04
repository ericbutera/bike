import { randomInt } from "node:crypto";
import { setTimeout as sleep } from "node:timers/promises";
import grpc from "@grpc/grpc-js";
import protoLoader from "@grpc/proto-loader";
import { fileURLToPath } from "node:url";

process.env.MAP_IMAGE_CACHE_TTL_SECONDS = "2";
await import("./server.mjs");

let ready = false;
for (let attempt = 0; attempt < 40; attempt += 1) {
  try {
    if ((await fetch("http://127.0.0.1:3100/")).ok) {
      ready = true;
      break;
    }
  } catch {
    /* Browser is starting. */
  }
  await sleep(250);
}
if (!ready) throw new Error("Renderer did not start");

const input = {
  activityId: 1,
  points: [
    {
      latitude: 44.7631 + randomInt(1_000_000_000) / 1e11,
      longitude: -85.6206,
    },
    { latitude: 44.769, longitude: -85.59 },
    { latitude: 44.782, longitude: -85.576 },
  ],
};

async function capture(theme, dpr, variant = "full") {
  const response = await fetch("http://127.0.0.1:3100/render", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      ...(process.env.MAP_SERVICE_TOKEN
        ? { Authorization: `Bearer ${process.env.MAP_SERVICE_TOKEN}` }
        : {}),
    },
    body: JSON.stringify({ ...input, theme, dpr, variant }),
  });
  if (!response.ok)
    throw new Error(
      `${theme}/${dpr}: ${response.status} ${await response.text()}`,
    );
  const png = Buffer.from(await response.arrayBuffer());
  if (
    png.subarray(0, 8).toString("hex") !== "89504e470d0a1a0a" ||
    png.readUInt32BE(16) !== (variant === "full" ? 1000 : 288) * dpr ||
    png.readUInt32BE(20) !== (variant === "full" ? 300 : 192) * dpr
  ) {
    throw new Error(`${theme}/${dpr}: invalid PNG`);
  }
  console.log(`${theme}/${variant}/${dpr}x: ${png.length} bytes`);
  return { png, cacheStatus: response.headers.get("x-map-cache") };
}

async function captureGrpc() {
  const protoPath =
    process.env.MAP_SERVICE_PROTO_PATH ??
    fileURLToPath(new URL("../proto/bike/maps/v1/maps.proto", import.meta.url));
  const mapsPackage = grpc.loadPackageDefinition(
    protoLoader.loadSync(protoPath, { defaults: true }),
  ).bike.maps.v1;
  const client = new mapsPackage.MapService(
    "127.0.0.1:50051",
    grpc.credentials.createInsecure(),
  );
  const metadata = new grpc.Metadata();
  if (process.env.MAP_SERVICE_TOKEN) {
    metadata.set("authorization", `Bearer ${process.env.MAP_SERVICE_TOKEN}`);
  }
  try {
    if (process.env.MAP_SERVICE_TOKEN) {
      const denied = await new Promise((resolve) => {
        client.render(
          {
            points: input.points,
            theme: "light",
            variant: "full",
            dpr: 1,
          },
          new grpc.Metadata(),
          (error) => resolve(error),
        );
      });
      if (denied?.code !== grpc.status.UNAUTHENTICATED) {
        throw new Error("gRPC accepted an unsigned request");
      }
    }
    const result = await new Promise((resolve, reject) => {
      client.render(
        {
          points: input.points,
          theme: "light",
          variant: "full",
          dpr: 1,
        },
        metadata,
        { deadline: Date.now() + 60000 },
        (error, response) => (error ? reject(error) : resolve(response)),
      );
    });
    if (result.png.subarray(0, 8).toString("hex") !== "89504e470d0a1a0a") {
      throw new Error("gRPC did not return a PNG");
    }
    const cached = await new Promise((resolve, reject) => {
      client.render(
        {
          points: input.points,
          theme: "light",
          variant: "full",
          dpr: 1,
        },
        metadata,
        { deadline: Date.now() + 60000 },
        (error, response) => (error ? reject(error) : resolve(response)),
      );
    });
    if (!cached.cacheHit || !result.png.equals(cached.png)) {
      throw new Error("gRPC image was not served from the shared cache");
    }
    console.log(`gRPC light/full/1x: ${result.png.length} bytes`);
  } finally {
    client.close();
  }
}

try {
  if (process.env.MAP_SERVICE_TOKEN) {
    const denied = await fetch("http://127.0.0.1:3100/render", {
      method: "POST",
    });
    if (denied.status !== 401)
      throw new Error("Renderer accepted an unsigned request");
  }
  const first = await capture("light", 1);
  const cached = await capture("light", 1);
  if (
    first.cacheStatus !== "miss" ||
    cached.cacheStatus !== "hit" ||
    !first.png.equals(cached.png)
  ) {
    throw new Error("Image was not served from the cache");
  }
  await sleep(2100);
  const refreshed = await capture("light", 1);
  if (refreshed.cacheStatus !== "miss")
    throw new Error("Expired image was not regenerated");
  await capture("dark", 2);
  await capture("light", 2, "thumbnail");
  await capture("dark", 1, "thumbnail");
  await captureGrpc();
  console.log("Renderer smoke check passed");
  process.exit(0);
} catch (error) {
  console.error(error);
  process.exit(1);
}
