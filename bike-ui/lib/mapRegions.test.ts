import { readFileSync } from "node:fs";
import { gunzipSync } from "node:zlib";
import { describe, expect, it } from "vitest";
import { findMapRegion, type MapRegions } from "./mapRegions";

const regions: MapRegions = JSON.parse(
  gunzipSync(readFileSync("public/map-regions.geojson.gz")).toString("utf8"),
);

describe("bundled map region boundaries", () => {
  it("preserves the complete set of state and province features", () => {
    expect(regions.features).toHaveLength(4596);
    expect(
      regions.features.every(
        (feature) => feature.geometry.coordinates.length > 0,
      ),
    ).toBe(true);
  });
  it("resolves both Michigan peninsulas to bounds containing the entire state", () => {
    const lower = findMapRegion(regions, [-85.62, 44.76]);
    const upper = findMapRegion(regions, [-87.4, 46.55]);
    expect(lower?.name).toBe("Michigan");
    expect(upper).toEqual(lower);
    expect(lower!.bounds[0]).toBeLessThan(-90);
    expect(lower!.bounds[1]).toBeLessThan(42);
    expect(lower!.bounds[2]).toBeGreaterThan(-83);
    expect(lower!.bounds[3]).toBeGreaterThan(48);
  });
  it("uses the containing state instead of a neighboring state's overlapping bounds", () => {
    expect(findMapRegion(regions, [-87.63, 41.88])?.name).toBe("Illinois");
    expect(findMapRegion(regions, [-123.12, 49.28])?.name).toBe(
      "British Columbia",
    );
    expect(findMapRegion(regions, [151.21, -33.87])?.name).toBe(
      "New South Wales",
    );
    expect(findMapRegion(regions, [0, 0])).toBeUndefined();
  });
  it("excludes polygon holes and includes separate islands", () => {
    const islands: MapRegions = {
      type: "FeatureCollection",
      features: [
        {
          type: "Feature",
          properties: { name: "Islands" },
          geometry: {
            type: "MultiPolygon",
            coordinates: [
              [
                [
                  [0, 0],
                  [4, 0],
                  [4, 4],
                  [0, 4],
                  [0, 0],
                ],
                [
                  [1, 1],
                  [2, 1],
                  [2, 2],
                  [1, 2],
                  [1, 1],
                ],
              ],
              [
                [
                  [10, 10],
                  [12, 10],
                  [12, 12],
                  [10, 12],
                  [10, 10],
                ],
              ],
            ],
          },
        },
      ],
    };
    expect(findMapRegion(islands, [1.5, 1.5])).toBeUndefined();
    expect(findMapRegion(islands, [11, 11])).toEqual({
      name: "Islands",
      bounds: [0, 0, 12, 12],
    });
  });
  it("fits states crossing the date line without zooming out to the whole world", () => {
    const state = findMapRegion(regions, [-149.9, 61.2]);
    expect(state?.name).toBe("Alaska");
    expect(state!.bounds[2] - state!.bounds[0]).toBeLessThan(70);
    expect(state!.bounds[3]).toBeGreaterThan(70);
  });
});
