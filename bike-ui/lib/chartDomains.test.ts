import { describe, expect, it } from "vitest";
import { maxFiniteValue, zeroBasedDomain } from "./chartDomains";

describe("chartDomains", () => {
  it("finds the maximum finite value while ignoring empty values", () => {
    expect(maxFiniteValue([undefined, null, Number.NaN, 12, 8])).toBe(12);
  });

  it("builds a stable zero-based domain from all supplied values", () => {
    expect(zeroBasedDomain([12, 48, 36])).toEqual([0, 48]);
  });

  it("keeps an empty chart domain non-degenerate", () => {
    expect(zeroBasedDomain([])).toEqual([0, 1]);
  });
});
