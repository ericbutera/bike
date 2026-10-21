import { describe, expect, it } from "vitest";
import { competitionData } from "../tests/e2e/helpers/seeds.mjs";
import {
  competitionRiders,
  competitionCases,
} from "../tests/e2e/helpers/segment-competition.mjs";

describe("segment competition fixtures", () => {
  it("gives fifteen active competitors both real routes and unique ride IDs", () => {
    const { users, rides } = competitionData();
    expect(competitionRiders).toHaveLength(15);
    expect(new Set(competitionRiders).size).toBe(15);
    expect(users).toHaveLength(14); // The existing SSO reviewer is competitor fifteen.
    expect(users.every(({ values }) => !values.disabled)).toBe(true);
    expect(new Set(rides.map(({ id }) => id)).size).toBe(rides.length);
    for (const userId of competitionRiders) {
      expect(
        new Set(
          rides
            .filter((ride) => ride.user_id === userId)
            .map(({ route }) => route),
        ),
      ).toEqual(new Set(["city", "unmarked"]));
    }
  });

  it("includes same-day displacement, KOM turnover and a later slower yearly best", () => {
    const { rides } = competitionData();
    const find = (id) => rides.find((ride) => ride.id === id);
    const earned = find(competitionCases.earnedTopTen);
    const displaced = find(competitionCases.displacedTopTen);
    expect(earned.started_at.slice(0, 10)).toBe(
      displaced.started_at.slice(0, 10),
    );
    expect(Date.parse(displaced.started_at)).toBeGreaterThan(
      Date.parse(earned.started_at),
    );
    expect(displaced.pace).toBeLessThan(earned.pace);
    expect(find(competitionCases.newKom).pace).toBeLessThan(0.55);
    expect(find(competitionCases.yearlyBest).pace).toBeGreaterThan(earned.pace);
    expect(find(competitionCases.repeatedAttempts).repeats).toBe(3);
    expect(rides.filter((ride) => ride.user_id === 1).length).toBeGreaterThan(
      10,
    );
    expect(new Set(rides.map((ride) => ride.started_at.slice(0, 4)))).toEqual(
      new Set(["2024", "2025", "2026"]),
    );
  });
});
