import { describe, expect, it } from "vitest";
import { baselineData, scenarioData } from "../e2e/helpers/seeds.mjs";

describe("review fixtures", () => {
  it("removes only the selected ride's ranks, preserving times and comparison history", () => {
    const original = baselineData();
    const efforts = original.records.filter(
      (record) => record.entity === "segment_efforts",
    );
    const scenario = scenarioData("missing-ranks");
    const changed = efforts.map((effort) => ({
      ...effort.values,
      ...scenario.records.find((patch) => patch.values.id === effort.values.id)
        ?.values,
    }));

    expect(changed.find((effort) => effort.activity_id === 1109)).toEqual({
      ...efforts.find((effort) => effort.values.activity_id === 1109).values,
      overall_rank: null,
      user_rank: null,
    });
    expect(changed.find((effort) => effort.activity_id === 1110)).toEqual(
      efforts.find((effort) => effort.values.activity_id === 1110).values,
    );
    expect(baselineData()).toEqual(original);
    expect(scenario.records).toHaveLength(1);
    expect(scenario.records[0].entity).toBe("segment_efforts");
  });

  it("rejects an unknown scenario before launching services", () => {
    expect(() => scenarioData("typo")).toThrow("Unknown E2E scenario");
  });
});
