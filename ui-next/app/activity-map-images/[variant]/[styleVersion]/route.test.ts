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

  it("passes only API-authorized geometry to the renderer", async () => {
    vi.stubEnv("INTERNAL_API_URL", "http://api:8080/api");
    vi.stubEnv("MAP_SERVICE_TOKEN", "test-map-token");
    const fetchMock = vi
      .spyOn(globalThis, "fetch")
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            updated_at: "2026-09-28T12:00:00Z",
            route_points: [
              { latitude: 45, longitude: -85, elapsed_seconds: 60 },
              { latitude: 45.01, longitude: -85.01 },
            ],
          }),
          { status: 200 },
        ),
      )
      .mockResolvedValueOnce(
        new Response(new Uint8Array([137, 80, 78, 71]), { status: 200 }),
      );

    const response = await GET(
      new Request(
        "http://localhost/activity-map-images/full/1?activityId=7&theme=dark&dpr=2",
        { headers: { authorization: "Bearer owner" } },
      ),
      context,
    );

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      "http://api:8080/api/activities/7",
    );
    expect(
      new Headers(fetchMock.mock.calls[0]?.[1]?.headers).get("authorization"),
    ).toBe("Bearer owner");
    const renderBody = JSON.parse(String(fetchMock.mock.calls[1]?.[1]?.body));
    expect(
      new Headers(fetchMock.mock.calls[1]?.[1]?.headers).get("authorization"),
    ).toBe("Bearer test-map-token");
    expect(renderBody).toMatchObject({
      profile: "rust",
      theme: "dark",
      dpr: 2,
      variant: "full",
      points: [
        { latitude: 45, longitude: -85 },
        { latitude: 45.01, longitude: -85.01 },
      ],
    });
    expect(renderBody).not.toHaveProperty("scope");
    expect(renderBody.points[0]).toEqual({ latitude: 45, longitude: -85 });
    expect(response.headers.get("Content-Type")).toBe("image/png");
    expect(response.headers.get("Cache-Control")).toBe("private, no-cache");
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
});
