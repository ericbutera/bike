// @vitest-environment node
import { afterEach, describe, expect, it, vi } from "vitest";
import { GET } from "./route";

const context = {
  params: Promise.resolve({ variant: "full", styleVersion: "1" }),
};

describe("activity map images", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllEnvs();
  });

  it("forwards one image request to Rust with owner credentials and cache headers", async () => {
    vi.stubEnv("INTERNAL_API_URL", "http://api:8080/api");
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(
      new Response(new Uint8Array([137, 80, 78, 71]), {
        status: 200,
        headers: {
          "Content-Type": "image/png",
          "Cache-Control": "private, no-cache",
          Vary: "Cookie, Authorization",
          ETag: '"map-etag"',
          "X-Map-Cache": "hit",
        },
      }),
    );
    const response = await GET(
      new Request(
        "http://localhost/activity-map-images/full/1?activityId=7&theme=dark&dpr=2",
        {
          headers: {
            authorization: "Bearer owner",
            cookie: "bike-session=owner",
            "if-none-match": '"older-map"',
            "x-bike-synthetic-key": "internal-test-key",
            "x-forwarded-for": "10.1.2.3",
          },
        },
      ),
      context,
    );
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://api:8080/api/activity-map-images/full/1?activityId=7&theme=dark&dpr=2",
    );
    const forwarded = new Headers(fetchMock.mock.calls[0]?.[1]?.headers);
    expect(forwarded.get("authorization")).toBe("Bearer owner");
    expect(forwarded.get("cookie")).toBe("bike-session=owner");
    expect(forwarded.get("if-none-match")).toBe('"older-map"');
    expect(forwarded.get("x-bike-synthetic-key")).toBe("internal-test-key");
    expect(fetchMock.mock.calls[0]?.[1]?.body).toBeUndefined();
    expect(response.headers.get("Content-Type")).toBe("image/png");
    expect(response.headers.get("Cache-Control")).toBe("private, no-cache");
    expect(response.headers.get("ETag")).toBe('"map-etag"');
    expect(response.headers.get("X-Map-Cache")).toBe("hit");
  });

  it("preserves an API-authorized conditional response", async () => {
    vi.stubEnv("INTERNAL_API_URL", "http://api:8080/api");
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(
      new Response(null, {
        status: 304,
        headers: { ETag: '"map-etag"', "Cache-Control": "private, no-cache" },
      }),
    );
    const response = await GET(
      new Request(
        "http://localhost/activity-map-images/full/1?activityId=7&theme=light&dpr=1",
        { headers: { "if-none-match": '"map-etag"' } },
      ),
      context,
    );
    expect(response.status).toBe(304);
    expect(await response.text()).toBe("");
    expect(response.headers.get("ETag")).toBe('"map-etag"');
  });

  it("does not render an activity denied by the API", async () => {
    vi.stubEnv("INTERNAL_API_URL", "http://api:8080/api");
    const fetchMock = vi
      .spyOn(globalThis, "fetch")
      .mockResolvedValueOnce(new Response(null, { status: 403 }));
    const response = await GET(
      new Request(
        "http://localhost/activity-map-images/full/1?activityId=7&theme=light&dpr=1",
        { headers: { "if-none-match": '"previous-owner-image"' } },
      ),
      context,
    );
    expect(response.status).toBe(403);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("rejects a synthetic credential from a forwarded public request before fetching", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch");
    const response = await GET(
      new Request(
        "http://localhost/activity-map-images/full/1?activityId=7&theme=light&dpr=1",
        {
          headers: {
            "x-bike-synthetic-key": "internal-test-key",
            forwarded: "",
          },
        },
      ),
      context,
    );
    expect(response.status).toBe(403);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("uses the local API fallback after a connection failure", async () => {
    vi.stubEnv("INTERNAL_API_URL", "");
    vi.stubEnv("API_URL", "http://localhost:3000/api");
    const fetchMock = vi
      .spyOn(globalThis, "fetch")
      .mockRejectedValueOnce(new TypeError("connection refused"))
      .mockResolvedValueOnce(
        new Response(new Uint8Array([137, 80, 78, 71]), {
          headers: { "Content-Type": "image/png" },
        }),
      );
    const response = await GET(
      new Request("http://localhost/activity-map-images/full/1?activityId=7"),
      context,
    );
    expect(response.status).toBe(200);
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it("returns an uncached gateway error when every API address is unavailable", async () => {
    vi.stubEnv("INTERNAL_API_URL", "http://api:8080/api");
    vi.spyOn(globalThis, "fetch").mockRejectedValue(
      new TypeError("connection refused"),
    );
    const response = await GET(
      new Request("http://localhost/activity-map-images/full/1?activityId=7"),
      context,
    );
    expect(response.status).toBe(502);
    expect(response.headers.get("Cache-Control")).toBe("no-store");
  });

  it("maps upstream failures to an uncached gateway error", async () => {
    vi.stubEnv("INTERNAL_API_URL", "http://api:8080/api");
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(
      new Response(null, { status: 503 }),
    );
    const response = await GET(
      new Request("http://localhost/activity-map-images/full/1?activityId=7"),
      context,
    );
    expect(response.status).toBe(502);
    expect(response.headers.get("Cache-Control")).toBe("no-store");
  });
});
