import { describe, expect, it } from "vitest";
import { buildActivityClimbs } from "./activityClimbs";
import { type ActivityRoutePoint } from "./queries";

function point(
  elapsedSeconds: number,
  distanceMeters: number,
  elevationMeters: number,
): ActivityRoutePoint {
  return {
    elapsed_seconds: elapsedSeconds,
    latitude: 45 + elapsedSeconds * 0.00001,
    longitude: -122 + elapsedSeconds * 0.00001,
    distance_meters: distanceMeters,
    elevation_meters: elevationMeters,
  };
}

describe("buildActivityClimbs", () => {
  it("finds sustained climbs and keeps short rolling gaps inside the climb", () => {
    const climbs = buildActivityClimbs([
      point(0, 0, 100),
      point(120, 600, 125),
      point(240, 1200, 160),
      point(300, 1300, 158),
      point(420, 1900, 205),
      point(520, 2300, 215),
      point(620, 2600, 214),
      point(760, 3400, 218),
      point(900, 4300, 265),
      point(1040, 5200, 318),
    ]);

    expect(climbs).toHaveLength(2);
    expect(climbs[0]).toMatchObject({
      id: "climb-1",
      sequence: 1,
      startRoutePointIndex: 0,
      endRoutePointIndex: 4,
      startDistanceMeters: 0,
      endDistanceMeters: 1900,
      distanceMeters: 1900,
      elevationGainMeters: 107,
      elevationLossMeters: 2,
      category: 4,
      durationSeconds: 420,
    });
    expect(climbs[0]?.avgGradePercent).toBeCloseTo(5.632, 3);
    expect(climbs[0]?.maxGradePercent).toBeCloseTo(7.833, 3);
    expect(climbs[0]?.routePoints).toHaveLength(5);

    expect(climbs[1]).toMatchObject({
      id: "climb-2",
      sequence: 2,
      startRoutePointIndex: 7,
      endRoutePointIndex: 9,
      distanceMeters: 1800,
      elevationGainMeters: 100,
      elevationLossMeters: 0,
      category: 4,
      durationSeconds: 280,
    });
  });

  it("ignores short bumps that do not meet sustained climb thresholds", () => {
    const climbs = buildActivityClimbs([
      point(0, 0, 100),
      point(30, 120, 108),
      point(50, 220, 116),
      point(80, 350, 118),
    ]);

    expect(climbs).toEqual([]);
  });

  it("returns an empty list when distance or elevation samples are missing", () => {
    const climbs = buildActivityClimbs([
      point(0, 0, 100),
      {
        ...point(120, 700, 150),
        elevation_meters: null,
      },
      point(240, 1400, 200),
    ]);

    expect(climbs).toEqual([]);
  });
});
