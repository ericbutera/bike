import { describe, expect, it } from "vitest";
import { buildActivityClimbs } from "./activityClimbs";
import {
  climbOutput,
  savedClimb,
  climbingRoutePoints,
} from "../tests/fixtures/activities";

describe("buildActivityClimbs", () => {
  it("preserves saved numbering, bounds and metrics even below detection thresholds", () => {
    const [climb] = buildActivityClimbs(
      climbOutput([
        savedClimb({
          climb_number: 7,
          start_route_point_index: 10,
          end_route_point_index: 14,
          distance_meters: 100,
          elevation_gain_meters: 2,
          duration_seconds: 12,
          average_grade_percent: 2,
          elevation_loss_meters: 17,
          max_grade_percent: 42,
          category: "HC",
        }),
      ]),
    );
    expect(climb).toMatchObject({
      id: "climb-7",
      sequence: 7,
      startRoutePointIndex: 10,
      endRoutePointIndex: 14,
      distanceMeters: 100,
      elevationGainMeters: 2,
      avgGradePercent: 2,
      elevationLossMeters: 17,
      maxGradePercent: 42,
      category: "HC",
      durationSeconds: 12,
      routePoints: climbingRoutePoints(),
    });
  });

  it("uses saved metrics without deriving them from the prepared geometry", () => {
    const points = climbingRoutePoints();
    points[2].elevation_meters = 130;
    const [climb] = buildActivityClimbs(
      climbOutput([
        savedClimb({
          route_points: points,
          elevation_loss_meters: 9,
          max_grade_percent: 7,
          category: "2",
        }),
      ]),
    );
    expect(climb).toMatchObject({
      elevationLossMeters: 9,
      maxGradePercent: 7,
      category: "2",
    });
  });

  it("retains non-geographic climbs with missing presentation values", () => {
    const [climb] = buildActivityClimbs(
      climbOutput([
        savedClimb({
          route_points: [],
          start_route_point_index: null,
          end_route_point_index: null,
          average_grade_percent: null,
          elevation_loss_meters: null,
          max_grade_percent: null,
          category: null,
        }),
      ]),
    );
    expect(climb).toMatchObject({
      elevationGainMeters: 125,
      startDistanceMeters: null,
      endDistanceMeters: null,
      elevationLossMeters: null,
      maxGradePercent: null,
      category: null,
      avgGradePercent: null,
      routePoints: [],
    });
  });

  it.each([null, undefined, climbOutput([])])(
    "does not synthesize climbs for missing or empty results",
    (output) => {
      expect(buildActivityClimbs(output)).toEqual([]);
    },
  );
});
