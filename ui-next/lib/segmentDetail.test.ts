import { describe, expect, it } from "vitest";
import { buildLeaderPairFollowViewport } from "./segmentDetail";

describe("buildLeaderPairFollowViewport", () => {
  it("keeps the current tight zoom when there is no runner-up marker", () => {
    const leaderPoint = {
      elapsed_seconds: 12,
      latitude: 45,
      longitude: -122,
    };

    expect(buildLeaderPairFollowViewport(leaderPoint, null, 19)).toEqual({
      point: leaderPoint,
      zoom: 19,
    });
  });

  it("stays tightly zoomed when first and second are nearly overlapping", () => {
    const leaderPoint = {
      elapsed_seconds: 12,
      latitude: 45,
      longitude: -122,
    };
    const runnerUpPoint = {
      elapsed_seconds: 12,
      latitude: 45.00005,
      longitude: -121.99995,
    };

    const viewport = buildLeaderPairFollowViewport(
      leaderPoint,
      runnerUpPoint,
      19,
    );

    expect(viewport?.zoom).toBe(19);
  });

  it("zooms out and centers between first and second as the gap grows", () => {
    const leaderPoint = {
      elapsed_seconds: 12,
      latitude: 45,
      longitude: -122,
    };
    const runnerUpPoint = {
      elapsed_seconds: 12,
      latitude: 45.004,
      longitude: -121.992,
    };

    const viewport = buildLeaderPairFollowViewport(
      leaderPoint,
      runnerUpPoint,
      19,
    );

    expect(viewport?.point.latitude).toBeCloseTo(45.002, 6);
    expect(viewport?.point.longitude).toBeCloseTo(-121.996, 6);
    expect(viewport?.zoom).toBeLessThan(19);
    expect(viewport?.zoom).toBeGreaterThanOrEqual(12);
  });
});
