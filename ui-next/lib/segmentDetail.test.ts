import { describe, expect, it } from "vitest";
import {
  buildLeaderGroupFollowViewport,
  buildLeaderPairFollowViewport,
  parseOptionalPositiveNumberParam,
  parsePlaybackPaceParam,
  parseRacePlaybackSpeedParam,
  parseSelectedEffortIdsParam,
  segmentEffortDayAttemptSummaries,
} from "./segmentDetail";

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

describe("buildLeaderGroupFollowViewport", () => {
  it("widens enough to include the top three markers", () => {
    const leaderPoint = {
      elapsed_seconds: 12,
      latitude: 45,
      longitude: -122,
    };
    const runnerUpPoint = {
      elapsed_seconds: 12,
      latitude: 45.002,
      longitude: -121.998,
    };
    const thirdPoint = {
      elapsed_seconds: 12,
      latitude: 45.006,
      longitude: -121.99,
    };

    const viewport = buildLeaderGroupFollowViewport(
      [leaderPoint, runnerUpPoint, thirdPoint],
      19,
    );

    expect(viewport?.point.latitude).toBeCloseTo(45.003, 6);
    expect(viewport?.point.longitude).toBeCloseTo(-121.995, 6);
    expect(viewport?.zoom).toBeLessThan(19);
    expect(viewport?.zoom).toBeGreaterThanOrEqual(12);
  });
});

describe("segment route param parsers", () => {
  it("parses selected effort IDs from a comma-delimited route param", () => {
    expect(parseSelectedEffortIdsParam("1, 2,bad,-3,4")).toEqual([1, 2, 4]);
    expect(parseSelectedEffortIdsParam(["7,8", "9"])).toEqual([7, 8]);
    expect(parseSelectedEffortIdsParam(undefined)).toEqual([]);
  });

  it("parses optional positive number params", () => {
    expect(parseOptionalPositiveNumberParam("12")).toBe(12);
    expect(parseOptionalPositiveNumberParam("0")).toBeNull();
    expect(parseOptionalPositiveNumberParam("bad")).toBeNull();
  });

  it("parses playback pace params", () => {
    expect(parsePlaybackPaceParam("detail")).toBe("detail");
    expect(parsePlaybackPaceParam("fast")).toBe("fast");
    expect(parsePlaybackPaceParam("slow")).toBeUndefined();
  });

  it("parses race playback speed params", () => {
    expect(parseRacePlaybackSpeedParam("0.10")).toBe(0.1);
    expect(parseRacePlaybackSpeedParam("0.25")).toBe(0.25);
    expect(parseRacePlaybackSpeedParam("4")).toBe(4);
    expect(parseRacePlaybackSpeedParam("fast")).toBeUndefined();
    expect(parseRacePlaybackSpeedParam("0.2")).toBeUndefined();
  });
});

describe("segmentEffortDayAttemptSummaries", () => {
  it("numbers same-rider same-day attempts by activity time and segment start", () => {
    const summaries = segmentEffortDayAttemptSummaries([
      {
        id: 3,
        rider_user_id: 7,
        activity_id: 20,
        activity_title: "Evening Ride",
        rider_name: "Eric",
        activity_started_at: "2026-05-08T23:30:00Z",
        effort_index: 1,
        duration_seconds: 95,
        start_elapsed_seconds: 120,
        end_elapsed_seconds: 215,
      },
      {
        id: 2,
        rider_user_id: 7,
        activity_id: 20,
        activity_title: "Evening Ride",
        rider_name: "Eric",
        activity_started_at: "2026-05-08T23:30:00Z",
        effort_index: 2,
        duration_seconds: 87,
        start_elapsed_seconds: 420,
        end_elapsed_seconds: 507,
      },
      {
        id: 1,
        rider_user_id: 7,
        activity_id: 19,
        activity_title: "Lunch Ride",
        rider_name: "Eric",
        activity_started_at: "2026-05-08T16:00:00Z",
        effort_index: 1,
        duration_seconds: 91,
        start_elapsed_seconds: 300,
        end_elapsed_seconds: 391,
      },
      {
        id: 4,
        rider_user_id: 7,
        activity_id: 21,
        activity_title: "Next Day",
        rider_name: "Eric",
        activity_started_at: "2026-05-09T16:00:00Z",
        effort_index: 1,
        duration_seconds: 82,
        start_elapsed_seconds: 300,
        end_elapsed_seconds: 382,
      },
    ]);

    expect(summaries.get(1)).toEqual({
      attemptNumber: 1,
      attemptCount: 3,
      isFastestOfDay: false,
    });
    expect(summaries.get(3)).toEqual({
      attemptNumber: 2,
      attemptCount: 3,
      isFastestOfDay: false,
    });
    expect(summaries.get(2)).toEqual({
      attemptNumber: 3,
      attemptCount: 3,
      isFastestOfDay: true,
    });
    expect(summaries.get(4)).toEqual({
      attemptNumber: 1,
      attemptCount: 1,
      isFastestOfDay: false,
    });
  });
});
