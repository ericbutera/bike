// @vitest-environment node
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { GET } from "./route";

vi.mock("../../../../../lib/serverApi", () => ({
  apiBaseUrls: () => ["http://api:3000/api"],
}));
const fetchMock = vi.fn();
const params = { params: Promise.resolve({ z: "14", x: "500", y: "600.png" }) };
beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});
afterEach(() => vi.unstubAllGlobals());

describe("private heatmap tile proxy", () => {
  it("forwards viewer credentials and conditional headers to the API", async () => {
    fetchMock.mockResolvedValue(
      new Response(new Uint8Array([1, 2]), {
        headers: { "content-type": "image/png", etag: '"tile"' },
      }),
    );
    const request = new Request(
      "http://ui/heatmap-tiles/14/500/600.png?revision=7&sport=run&user_id=99",
      {
        headers: {
          cookie: "session=viewer",
          authorization: "Bearer viewer",
          "if-none-match": '"old"',
          traceparent:
            "00-01234567890123456789012345678901-0123456789012345-01",
        },
      },
    );
    const response = await GET(request, params);
    const [url, options] = fetchMock.mock.calls.at(-1)!;
    expect(url).toBe(
      "http://api:3000/api/maps/heatmap/tiles/14/500/600.png?sport=run&revision=7",
    );
    expect(options.cache).toBe("no-store");
    expect(options.headers.get("cookie")).toBe("session=viewer");
    expect(options.headers.get("authorization")).toBe("Bearer viewer");
    expect(options.headers.get("if-none-match")).toBe('"old"');
    expect(options.headers.get("traceparent")).toBe(
      request.headers.get("traceparent"),
    );
    expect(response.headers.get("cache-control")).toBe("private, no-cache");
    expect(response.headers.get("vary")).toBe("Cookie, Authorization");
  });
  it.each([401, 404, 409, 503])(
    "preserves API status %s without caching errors",
    async (status) => {
      fetchMock.mockResolvedValue(
        new Response(null, { status, headers: { "retry-after": "2" } }),
      );
      const response = await GET(
        new Request("http://ui/heatmap-tiles/14/500/600.png?revision=7"),
        params,
      );
      expect(response.status).toBe(status);
      expect(response.headers.get("cache-control")).toBe("no-store");
      expect(response.headers.get("retry-after")).toBe("2");
    },
  );
  it("always checks upstream authorization before returning 304", async () => {
    fetchMock.mockResolvedValue(
      new Response(null, { status: 304, headers: { etag: '"tile"' } }),
    );
    const response = await GET(
      new Request("http://ui/heatmap-tiles/14/500/600.png?revision=7", {
        headers: { "if-none-match": '"tile"' },
      }),
      params,
    );
    expect(response.status).toBe(304);
    expect(await response.text()).toBe("");
  });
});
