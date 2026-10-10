import { PNG } from "pngjs";
import { connectedTest as test, expect } from "./helpers/test.mjs";
import { activityId, targets } from "./helpers/targets.mjs";

test("private map PNGs persist in the API cache and support revalidation", async ({
  request,
}) => {
  for (const target of targets) {
    const url = new URL(
      `/activity-map-images/thumbnail/1?activityId=${activityId}&theme=light&dpr=1`,
      target.url,
    ).toString();
    const rendered = await request.get(url);
    expect(rendered.status()).toBe(200);
    expect(rendered.headers()["content-type"]).toContain("image/png");
    expect(rendered.headers()["cache-control"]).toBe("private, no-cache");
    expect(rendered.headers()["x-map-cache"]).toBe("miss");
    const bytes = await rendered.body();
    const png = PNG.sync.read(bytes);
    expect(png.width).toBeGreaterThan(1);
    expect(png.height).toBeGreaterThan(1);
    const etag = rendered.headers().etag;
    expect(etag).toMatch(/^"[a-f0-9]+"$/);

    const cached = await request.get(url);
    expect(cached.status()).toBe(200);
    expect(cached.headers()["x-map-cache"]).toBe("hit");
    expect(cached.headers().etag).toBe(etag);
    expect(await cached.body()).toEqual(bytes);

    const unchanged = await request.get(url, {
      headers: { "if-none-match": etag },
    });
    expect(unchanged.status()).toBe(304);
    expect(unchanged.headers()["x-map-cache"]).toBe("hit");
    expect(unchanged.headers().etag).toBe(etag);
    expect(await unchanged.body()).toHaveLength(0);
  }
});
