import { type Activity, type ActivityRoutePoint } from "./queries";

export type ActivityClimbCategory = string;

export type ActivityClimb = {
  id: string;
  sequence: number;
  startRoutePointIndex: number | null;
  endRoutePointIndex: number | null;
  startDistanceMeters: number | null;
  endDistanceMeters: number | null;
  distanceMeters: number;
  elevationGainMeters: number;
  elevationLossMeters: number | null;
  avgGradePercent: number | null;
  maxGradePercent: number | null;
  category: ActivityClimbCategory | null;
  durationSeconds: number | null;
  routePoints: ActivityRoutePoint[];
};

// Map saved values to the UI shape; processing owns all climb calculations.
export function buildActivityClimbs(
  output: Activity["climb_output"],
): ActivityClimb[] {
  return (output?.climbs ?? []).map((climb) => ({
    id: `climb-${climb.climb_number}`,
    sequence: climb.climb_number,
    startRoutePointIndex: climb.start_route_point_index ?? null,
    endRoutePointIndex: climb.end_route_point_index ?? null,
    startDistanceMeters: climb.route_points[0]?.distance_meters ?? null,
    endDistanceMeters: climb.route_points.at(-1)?.distance_meters ?? null,
    distanceMeters: climb.distance_meters,
    elevationGainMeters: climb.elevation_gain_meters,
    elevationLossMeters: climb.elevation_loss_meters ?? null,
    avgGradePercent: climb.average_grade_percent ?? null,
    maxGradePercent: climb.max_grade_percent ?? null,
    category: climb.category ?? null,
    durationSeconds: climb.duration_seconds,
    routePoints: climb.route_points,
  }));
}
