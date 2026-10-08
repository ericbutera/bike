import { describe, expect, it } from "vitest";
import { localRidingCenter, parseHeatmapCamera } from "./heatmapCamera";

describe("heatmap camera", () => {
  it("chooses the main riding area rather than the midpoint of distant trips", () => {
    const center = localRidingCenter([
      { longitude: -85.62, latitude: 44.76 },
      { longitude: -85.63, latitude: 44.77 },
      { longitude: -122.33, latitude: 47.61 },
    ]);
    expect(center![0]).toBeCloseTo(-85.625);
    expect(center![1]).toBeCloseTo(44.765);
    expect(localRidingCenter([])).toBeUndefined();
  });
  it("restores a valid saved camera and rejects partial or out-of-range cameras", () => {
    expect(
      parseHeatmapCamera(new URLSearchParams("lng=-85.62&lat=44.76&zoom=13")),
    ).toEqual({ longitude: -85.62, latitude: 44.76, zoom: 13 });
    for (const query of [
      "",
      "lng=0&lat=0",
      "lng=&lat=0&zoom=13",
      "lng=NaN&lat=0&zoom=13",
      "lng=181&lat=0&zoom=13",
      "lng=0&lat=86&zoom=13",
      "lng=0&lat=0&zoom=19",
    ]) {
      expect(parseHeatmapCamera(new URLSearchParams(query))).toBeUndefined();
    }
  });
  it("recovers the previously saved loading placeholder without rejecting other ocean views", () => {
    expect(
      parseHeatmapCamera(new URLSearchParams("lng=0&lat=0&zoom=2")),
    ).toBeUndefined();
    expect(
      parseHeatmapCamera(new URLSearchParams("lng=0&lat=0&zoom=13")),
    ).toEqual({ longitude: 0, latitude: 0, zoom: 13 });
  });
});
