import { describe, expect, it } from "vitest";
import {
  calculateHeartRateZoneBoundsFromMaxHeartRate,
  hasConfiguredHeartRateZoneBounds,
} from "../trainingProfile";

describe("trainingProfile helpers", () => {
  it("calculates four ascending heart rate zone ceilings from max heart rate", () => {
    expect(calculateHeartRateZoneBoundsFromMaxHeartRate(190)).toEqual([
      114, 133, 152, 171,
    ]);
  });

  it("rejects invalid max heart rate values", () => {
    expect(() => calculateHeartRateZoneBoundsFromMaxHeartRate(60)).toThrow(
      /Max heart rate must be between/,
    );
  });

  it("detects whether four heart rate zone ceilings are configured", () => {
    expect(hasConfiguredHeartRateZoneBounds([110, 128, 145, 162])).toBe(true);
    expect(hasConfiguredHeartRateZoneBounds([110, 128, 145])).toBe(false);
    expect(hasConfiguredHeartRateZoneBounds(null)).toBe(false);
  });
});
