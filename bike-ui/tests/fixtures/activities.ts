import type {
  Activity,
  ActivityEarnedAward,
  ActivityLap,
  ActivityRoutePoint,
} from "../../lib/queries";

export function activity(overrides: Partial<Activity> = {}): Activity {
  return {
    id: 42,
    earned_awards: [],
    title: "Fixture ride",
    sport: "road_ride",
    source: "manual_upload",
    started_at: "2026-10-14T12:00:00Z",
    ...overrides,
  };
}

export function activityEarnedAward(
  overrides: Partial<ActivityEarnedAward> = {},
): ActivityEarnedAward {
  return {
    activity_id: 42,
    position: 1,
    display_position: 1,
    segment_id: 11,
    segment_title: "Riverfront climb",
    effort_index: 1,
    overall_rank: 1,
    personal_rank: 1,
    personal_best_duration_seconds: 60,
    ...overrides,
  };
}

export function activityLap(overrides: Partial<ActivityLap> = {}): ActivityLap {
  return {
    lap_index: 1,
    title: "Warmup",
    duration_seconds: 1600,
    distance_meters: 14000,
    average_speed_mps: 8.7,
    average_heart_rate_bpm: 142,
    max_heart_rate_bpm: 162,
    ...overrides,
  };
}

export function climbingRoutePoints() {
  return [
    [0, 100],
    [600, 132],
    [1200, 168],
    [1800, 202],
    [2400, 225],
  ].map(([distance, elevation], index) => ({
    elapsed_seconds: index * 120,
    latitude: 45 + index * 0.01,
    longitude: -122 + index * 0.01,
    distance_meters: distance,
    elevation_meters: elevation,
    speed_mps: index === 0 ? 0 : 5,
    heart_rate_bpm: [128, 142, 148, 154, 160][index],
    cadence_rpm: [80, 82, 84, 86, 88][index],
    power_watts: [142, 210, 225, 238, 250][index],
  })) satisfies ActivityRoutePoint[];
}

export function savedClimb(
  overrides: Partial<
    NonNullable<Activity["climb_output"]>["climbs"][number]
  > = {},
) {
  return {
    climb_number: 1,
    start_route_point_index: 0,
    end_route_point_index: 4,
    start_elapsed_seconds: 0,
    end_elapsed_seconds: 480,
    duration_seconds: 480,
    distance_meters: 2400,
    elevation_gain_meters: 125,
    average_grade_percent: (125 / 2400) * 100,
    elevation_loss_meters: 0,
    max_grade_percent: 6,
    category: "4",
    route_points: climbingRoutePoints(),
    ...overrides,
  };
}

export function climbOutput(
  climbs = [savedClimb()],
): NonNullable<Activity["climb_output"]> {
  return { detector_version: 1, recording_sha256: "fixture-recording", climbs };
}
