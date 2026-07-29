import { describe, expect, it } from "vitest";
import {
  buildLeaderPairFollowViewport,
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
