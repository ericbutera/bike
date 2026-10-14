import { activityTypeValues, type components } from "./openapi/react-query/api";

export type ActivityType = components["schemas"]["ActivityType"];

export const ACTIVITY_TYPES = Object.fromEntries(
  activityTypeValues.map((value) => [
    value.charAt(0).toUpperCase() + value.slice(1),
    value,
  ]),
) as { [Value in ActivityType as Capitalize<Value>]: Value };

const activityTypePresentation = {
  training: {
    label: "Training",
    description:
      "Cycling activities count toward training volume and progression.",
  },
  race: {
    label: "Race",
    description: "Cycling races are outcomes for XC race insights.",
  },
} satisfies Record<ActivityType, { label: string; description: string }>;

export const ACTIVITY_TYPE_OPTIONS = activityTypeValues.map((value) => ({
  value,
  ...activityTypePresentation[value],
}));

export function normalizeActivityType(
  value: ActivityType | string | null | undefined,
): ActivityType {
  return activityTypeValues.includes(value as ActivityType)
    ? (value as ActivityType)
    : ACTIVITY_TYPES.Training;
}

export function formatActivityTypeLabel(
  value: ActivityType | string | null | undefined,
) {
  return activityTypePresentation[normalizeActivityType(value)].label;
}
