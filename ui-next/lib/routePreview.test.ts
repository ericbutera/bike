import { describe, expect, it } from "vitest";
import { buildRoutePreviewGeometry } from "./routePreview";

describe("buildRoutePreviewGeometry", () => {
  it("keeps every point for a sharp-turn feed preview", () => {
    const routePoints = [
      { latitude: 45.0, longitude: -122.0 },
      { latitude: 45.005, longitude: -121.99 },
      { latitude: 44.996, longitude: -121.985 },
      { latitude: 45.012, longitude: -121.97 },
      { latitude: 44.994, longitude: -121.955 },
      { latitude: 45.008, longitude: -121.94 },
    ];
    const geometry = buildRoutePreviewGeometry(routePoints, "full");

    expect(geometry).not.toBeNull();
    expect(geometry?.width).toBe(1000);
    expect(geometry?.height).toBe(300);
    expect(geometry?.pathData).toContain(" L ");
    expect(geometry?.pathData).not.toContain("C ");
    expect(geometry?.pathData.match(/ L /g) ?? []).toHaveLength(
      routePoints.length - 1,
    );
  });
});
