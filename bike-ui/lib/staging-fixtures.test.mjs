import { describe, expect, it } from "vitest";
import { baselineData, stagingData } from "../tests/e2e/helpers/seeds.mjs";

describe("staging fixture export", () => {
  it("reuses rides and segments, assigns their owner to SSO, and replaces test keys", () => {
    let nextKey = 0;
    const data = stagingData(
      "reviewer@example.test",
      () => `staged-key-${++nextKey}`,
    );
    const baseline = baselineData();
    const users = data.records.filter((record) => record.entity === "users");
    expect(users[0].values).toMatchObject({
      id: 1,
      email: "reviewer@example.test",
      is_admin: true,
    });
    expect(
      users.every((record) => record.values.api_key.startsWith("staged-key-")),
    ).toBe(true);
    expect(data.records.filter((record) => record.entity !== "users")).toEqual(
      baseline.records.filter((record) => record.entity !== "users"),
    );
    expect(data.prepare_heatmaps).toBe(true);
    expect(baseline.records[0].values.email).toBe("developer@bike.local");
  });

  it("rejects a missing or malformed owner", () => {
    for (const email of ["", "invalid", "user name@example.test"]) {
      expect(() => stagingData(email, () => "unused")).toThrow("login email");
    }
  });
});
