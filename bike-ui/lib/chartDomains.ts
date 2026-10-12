export type NumericDomain = [number, number];

export function maxFiniteValue(
  values: Iterable<number | null | undefined>,
  fallback = 0,
) {
  let maximum = Number.NEGATIVE_INFINITY;

  for (const value of values) {
    if (typeof value === "number" && Number.isFinite(value)) {
      maximum = Math.max(maximum, value);
    }
  }

  return maximum === Number.NEGATIVE_INFINITY ? fallback : maximum;
}

export function zeroBasedDomain(
  values: Iterable<number | null | undefined>,
  fallbackMax = 1,
): NumericDomain {
  return [0, Math.max(maxFiniteValue(values, fallbackMax), fallbackMax)];
}
