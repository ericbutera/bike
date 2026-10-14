import {
  activitySportValues,
  type components,
} from "./openapi/react-query/api";

export type ActivitySport = components["schemas"]["ActivitySport"];

export const ACTIVITY_SPORT_OPTIONS = activitySportValues.map((value) => ({
  value,
  label: value.charAt(0).toUpperCase() + value.slice(1).replaceAll("_", " "),
}));

const activitySports = new Set<string>(activitySportValues);

export function parseActivitySport(
  value: string | null,
): ActivitySport | undefined {
  return value && activitySports.has(value)
    ? (value as ActivitySport)
    : undefined;
}
