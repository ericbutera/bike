import { PNG } from "pngjs";
import { readFile } from "node:fs/promises";

export const blankPng = PNG.sync.write({
  width: 1,
  height: 1,
  data: Buffer.alloc(4),
});
const styles = await Promise.all(
  ["fiord-v1", "route-light-v1"].map(async (name) =>
    JSON.parse(
      await readFile(
        new URL(`../../../public/map-styles/${name}.json`, import.meta.url),
        "utf8",
      ),
    ),
  ),
);
const vectorLayers = [
  ...new Set(
    styles.flatMap((style) =>
      style.layers.map((layer) => layer["source-layer"]).filter(Boolean),
    ),
  ),
].map((id) => ({ id, fields: {} }));

// Controls browser traffic only. The renderer mounts its own synthetic style.
export async function fakeExternalBasemaps(context) {
  await context.route(
    /^https:\/\/(?:tiles\.openfreemap\.org|tile\.waymarkedtrails\.org|server\.arcgisonline\.com)\//,
    async (route) => {
      const url = new URL(route.request().url());
      const headers = { "access-control-allow-origin": "*" };
      if (url.pathname.endsWith("/planet")) {
        await route.fulfill({
          headers,
          json: {
            tilejson: "3.0.0",
            tiles: ["https://tiles.openfreemap.org/fixture/{z}/{x}/{y}.pbf"],
            minzoom: 0,
            maxzoom: 14,
            vector_layers: vectorLayers,
          },
        });
      } else if (url.pathname.endsWith(".pbf")) {
        await route.fulfill({
          headers,
          contentType: "application/x-protobuf",
          body: Buffer.alloc(0),
        });
      } else if (url.pathname.includes("/styles/")) {
        await route.fulfill({
          headers,
          json: { version: 8, sources: {}, layers: [] },
        });
      } else if (url.pathname.endsWith(".json")) {
        await route.fulfill({ headers, json: {} });
      } else {
        await route.fulfill({
          headers,
          contentType: "image/png",
          body: blankPng,
        });
      }
    },
  );
}
