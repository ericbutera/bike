import type {
  ActivitySegmentEffort,
  OverallAchievement,
  PersonalAchievement,
} from "../../lib/queries";

export function segmentEffort(
  overrides: Partial<ActivitySegmentEffort> = {},
): ActivitySegmentEffort {
  return {
    segment_id: 11,
    segment_title: "North Climb",
    effort_index: 1,
    duration_seconds: 312,
    start_route_point_index: 0,
    end_route_point_index: 2,
    overall_rank: 1,
    personal_rank: 1,
    personal_best_duration_seconds: 312,
    ...overrides,
  };
}

export function personalAchievement(
  overrides: Partial<PersonalAchievement> = {},
): PersonalAchievement {
  return { position: 1, effort_id: 1, duration_seconds: 312, ...overrides };
}

export function overallAchievement(
  overrides: Partial<OverallAchievement> = {},
): OverallAchievement {
  return {
    position: 1,
    user_id: 7,
    rider_name: "Fixture Rider",
    effort_id: 90,
    duration_seconds: 312,
    ...overrides,
  };
}
