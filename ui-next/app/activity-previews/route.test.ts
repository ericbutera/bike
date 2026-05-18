import { afterEach, describe, expect, it, vi } from "vitest";
import { handlePreviewRequest } from "./route";

describe("handlePreviewRequest", () => {
  afterEach(() => {
    vi.unstubAllEnvs();
    vi.restoreAllMocks();
  });

  it("loads full activity geometry through the internal API candidate", async () => {
    vi.stubEnv("API_URL", "http://localhost:3000/api");

    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(
        JSON.stringify({
          route_points: [
            { latitude: 45.0, longitude: -122.0 },
            { latitude: 45.015, longitude: -121.985 },
            { latitude: 45.03, longitude: -121.97 },
          ],
        }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      ),
    );

    const response = await handlePreviewRequest(
      new Request("http://localhost:3001/activity-previews/full?activityId=7"),
      "full",
    );

    expect(fetchMock).toHaveBeenCalledWith(
      "http://api:3000/api/activities/7",
      expect.objectContaining({
        cache: "no-store",
      }),
    );
    expect(response.headers.get("Content-Type")).toBe("image/svg+xml");
    expect(response.headers.get("Vary")).toBe("Cookie");

    const body = await response.text();

    expect(body).toContain('stroke="#16b8a5"');
    expect(body).not.toContain('fill-opacity="0.75"');
    expect(body).toContain('viewBox="0 0 1000 300"');
  });
});
