import type { components } from "./openapi/react-query/api";

export type ActivitySport = components["schemas"]["ActivitySport"];

export const ACTIVITY_SPORT_OPTIONS = [
  { value: "run", label: "Run" },
  { value: "walk", label: "Walk" },
  { value: "hike", label: "Hike" },
  { value: "mountain_bike", label: "Mountain bike" },
  { value: "indoor_trainer_ride", label: "Indoor trainer ride" },
  { value: "road_ride", label: "Road ride" },
] as const satisfies ReadonlyArray<{ value: ActivitySport; label: string }>;

const activitySports = new Set<string>(
  ACTIVITY_SPORT_OPTIONS.map(({ value }) => value),
);

export function parseActivitySport(
  value: string | null,
): ActivitySport | undefined {
  return value && activitySports.has(value)
    ? (value as ActivitySport)
    : undefined;
}
