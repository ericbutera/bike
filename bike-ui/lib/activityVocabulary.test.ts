import { describe, expect, it } from "vitest";
import { ACTIVITY_SPORT_OPTIONS, parseActivitySport } from "./activitySports";
import {
  ACTIVITY_TYPES,
  ACTIVITY_TYPE_OPTIONS,
  formatActivityTypeLabel,
  normalizeActivityType,
} from "./activityTypes";
import {
  activitySportValues,
  activityTypeValues,
} from "./openapi/react-query/api";

describe("backend activity vocabulary", () => {
  it("uses every OpenAPI sport in filters and URL parsing", () => {
    expect(ACTIVITY_SPORT_OPTIONS.map(({ value }) => value)).toEqual(
      activitySportValues,
    );
    for (const value of activitySportValues) {
      expect(parseActivitySport(value)).toBe(value);
    }
    expect(ACTIVITY_SPORT_OPTIONS).toContainEqual({
      value: "swim",
      label: "Swim",
    });
    expect(parseActivitySport("unrecognized")).toBeUndefined();
    expect(parseActivitySport(null)).toBeUndefined();
  });

  it("uses OpenAPI classifications with local display text", () => {
    expect(ACTIVITY_TYPE_OPTIONS.map(({ value }) => value)).toEqual(
      activityTypeValues,
    );
    for (const option of ACTIVITY_TYPE_OPTIONS) {
      expect(normalizeActivityType(option.value)).toBe(option.value);
      expect(formatActivityTypeLabel(option.value)).toBe(option.label);
    }
    expect(normalizeActivityType("unrecognized")).toBe(ACTIVITY_TYPES.Training);
    expect(normalizeActivityType(null)).toBe(ACTIVITY_TYPES.Training);
    expect(formatActivityTypeLabel(ACTIVITY_TYPES.Race)).toBe("Race");
  });
});
