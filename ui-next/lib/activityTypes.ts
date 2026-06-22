export const ACTIVITY_TYPES = {
  Training: "training",
  Race: "race",
} as const;

export type ActivityType = (typeof ACTIVITY_TYPES)[keyof typeof ACTIVITY_TYPES];

export const ACTIVITY_TYPE_OPTIONS = [
  {
    value: ACTIVITY_TYPES.Training,
    label: "Training",
    description: "Counts as training volume and progression.",
  },
  {
    value: ACTIVITY_TYPES.Race,
    label: "Race",
    description: "Treated as an outcome for XC race insights.",
  },
] satisfies Array<{
  value: ActivityType;
  label: string;
  description: string;
}>;

export function normalizeActivityType(
  value: ActivityType | string | null | undefined,
): ActivityType {
  return value === ACTIVITY_TYPES.Race
    ? ACTIVITY_TYPES.Race
    : ACTIVITY_TYPES.Training;
}

export function formatActivityTypeLabel(
  value: ActivityType | string | null | undefined,
) {
  return normalizeActivityType(value) === ACTIVITY_TYPES.Race
    ? "Race"
    : "Training";
}
