import { describe, expect, it } from "vitest";
import { chartBucketLabel } from "./Charts";

describe("report chart labels", () => {
  it("groups yearly report buckets by their UTC year", () => {
    const date = new Date("2026-01-01T00:30:00Z");
    for (const range of ["3year", "5year", "all"] as const) {
      expect(chartBucketLabel(date, range)).toBe("2026");
    }
  });
});
