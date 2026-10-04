export type NumericDomain = [number, number];

export function maxFiniteValue(
  values: Iterable<number | null | undefined>,
  fallback = 0,
) {
  let maxValue = Number.NEGATIVE_INFINITY;

  for (const value of values) {
    if (typeof value === "number" && Number.isFinite(value)) {
      maxValue = Math.max(maxValue, value);
    }
  }

  return maxValue === Number.NEGATIVE_INFINITY ? fallback : maxValue;
}

export function zeroBasedDomain(
  values: Iterable<number | null | undefined>,
  fallbackMax = 1,
): NumericDomain {
  return [0, Math.max(maxFiniteValue(values, fallbackMax), fallbackMax)];
}
