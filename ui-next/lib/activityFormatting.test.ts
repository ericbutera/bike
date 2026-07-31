import { describe, expect, it } from "vitest";
import { formatElevationRate } from "./activityFormatting";

describe("activity formatting", () => {
  it("formats elevation rate using the selected unit system", () => {
    expect(formatElevationRate(1000, "imperial")).toBe("3,281 ft/h");
    expect(formatElevationRate(1000, "metric")).toBe("1,000 m/h");
    expect(formatElevationRate(1000, "mixed")).toBe("1,000 m/h");
  });
});
