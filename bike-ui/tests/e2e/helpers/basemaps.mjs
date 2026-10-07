import { PNG } from "pngjs";

const blank = PNG.sync.write({ width: 1, height: 1, data: Buffer.alloc(4) });

export async function fakeExternalBasemaps(context) {
  await context.route(
    /^https:\/\/(?:tiles\.openfreemap\.org|tile\.waymarkedtrails\.org|server\.arcgisonline\.com)\//,
    async (route) => {
      const url = route.request().url();
      const headers = { "access-control-allow-origin": "*" };
      if (url.endsWith("/planet")) {
        await route.fulfill({
          headers,
          json: {
            tilejson: "3.0.0",
            tiles: ["https://tiles.openfreemap.org/fixture/{z}/{x}/{y}.pbf"],
            minzoom: 0,
            maxzoom: 14,
            vector_layers: [],
          },
        });
      } else if (url.includes("/styles/")) {
        await route.fulfill({
          headers,
          json: { version: 8, sources: {}, layers: [] },
        });
      } else if (url.endsWith(".json")) {
        await route.fulfill({ headers, json: {} });
      } else if (url.endsWith(".pbf")) {
        await route.fulfill({
          headers,
          contentType: "application/x-protobuf",
          body: Buffer.alloc(0),
        });
      } else {
        await route.fulfill({ headers, contentType: "image/png", body: blank });
      }
    },
  );
}
