import { createHash } from "node:crypto";
import { setTimeout as sleep } from "node:timers/promises";

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

const scope = createHash("sha256").update(String(Date.now())).digest("hex");
const input = {
  scope,
  activityId: 1,
  points: [
    { latitude: 44.7631, longitude: -85.6206 },
    { latitude: 44.769, longitude: -85.59 },
    { latitude: 44.782, longitude: -85.576 },
  ],
};

async function capture(theme, dpr, variant = "full") {
  const response = await fetch("http://127.0.0.1:3100/render", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
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

try {
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
  console.log("Renderer smoke check passed");
  process.exit(0);
} catch (error) {
  console.error(error);
  process.exit(1);
}
