const HEART_RATE_ZONE_FIELD_COUNT = 4;
const HEART_RATE_ZONE_MAX_HEART_RATE_CEILING_PERCENTAGES = [0.6, 0.7, 0.8, 0.9];

export const MIN_MAX_HEART_RATE_BPM = 80;
export const MAX_MAX_HEART_RATE_BPM = 240;

export function hasConfiguredHeartRateZoneBounds(
  bounds?: number[] | null,
): boolean {
  return (bounds?.length ?? 0) === HEART_RATE_ZONE_FIELD_COUNT;
}

export function calculateHeartRateZoneBoundsFromMaxHeartRate(
  maxHeartRateBpm: number,
): number[] {
  if (
    !Number.isFinite(maxHeartRateBpm) ||
    maxHeartRateBpm < MIN_MAX_HEART_RATE_BPM ||
    maxHeartRateBpm > MAX_MAX_HEART_RATE_BPM
  ) {
    throw new Error(
      `Max heart rate must be between ${MIN_MAX_HEART_RATE_BPM} and ${MAX_MAX_HEART_RATE_BPM} bpm.`,
    );
  }

  let previousCeiling = 39;

  return HEART_RATE_ZONE_MAX_HEART_RATE_CEILING_PERCENTAGES.map((percent) => {
    const nextCeiling = Math.max(
      previousCeiling + 1,
      Math.round(maxHeartRateBpm * percent),
    );

    previousCeiling = nextCeiling;
    return nextCeiling;
  });
}
