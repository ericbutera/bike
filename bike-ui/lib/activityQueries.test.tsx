import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  activityCard,
  activityEarnedAward,
  climbingRoutePoints,
} from "@/tests/fixtures/activities";
import { useActivities } from "./queries";

const api = vi.hoisted(() => ({ query: vi.fn() }));

vi.mock("./api", () => ({
  $api: { useQuery: api.query },
  $typedApi: { useQuery: api.query },
}));

beforeEach(() => api.query.mockReset());

describe("saved activity card query", () => {
  it.each([7, 8])(
    "returns rider %s compact cards and pagination in their saved order",
    (userId) => {
      const activityId = userId * 10;
      const cards = [
        activityCard({
          id: activityId,
          route_points: climbingRoutePoints().map(
            ({ elapsed_seconds, latitude, longitude }) => ({
              elapsed_seconds,
              latitude,
              longitude,
            }),
          ),
          earned_awards: [
            activityEarnedAward({ activity_id: activityId, position: 2 }),
            activityEarnedAward({ activity_id: activityId, position: 1 }),
          ],
        }),
        activityCard({ id: activityId - 1, sport: "hike" }),
      ];
      const metadata = {
        page: 2,
        per_page: 2,
        total: 5,
        total_pages: 3,
      };
      api.query.mockReturnValue({
        data: { data: cards, metadata },
        isFetching: false,
        isError: false,
        error: null,
      });

      const { result } = renderHook(() =>
        useActivities({ page: 2, perPage: 2, sport: "hike" }),
      );

      expect(api.query).toHaveBeenCalledWith(
        "get",
        "/activities",
        { params: { query: { page: 2, per_page: 2, sport: "hike" } } },
        { enabled: true },
      );
      expect(result.current.data).toBe(cards);
      expect(result.current.metadata).toBe(metadata);
      expect(
        result.current.data?.[0].earned_awards.map((award) => award.position),
      ).toEqual([2, 1]);
      expect(result.current.isFetching).toBe(false);
      expect(result.current.isError).toBe(false);
      expect(result.current.error).toBeNull();
    },
  );

  it("preserves loading, completed empty pages and query failures", () => {
    api.query.mockReturnValue({ data: undefined, isLoading: true });
    const { result, rerender } = renderHook(useActivities);
    expect(result.current.data).toBeUndefined();
    expect(result.current.metadata).toBeUndefined();
    expect(result.current.isLoading).toBe(true);

    const metadata = { page: 1, per_page: 10, total: 0, total_pages: 0 };
    api.query.mockReturnValue({
      data: { data: [], metadata },
      isLoading: false,
    });
    rerender();
    expect(result.current.data).toEqual([]);
    expect(result.current.metadata).toBe(metadata);
    expect(result.current.isLoading).toBe(false);

    const error = new Error("Activity cards unavailable");
    api.query.mockReturnValue({ data: undefined, isError: true, error });
    rerender();
    expect(result.current.data).toBeUndefined();
    expect(result.current.metadata).toBeUndefined();
    expect(result.current.isError).toBe(true);
    expect(result.current.error).toBe(error);
  });

  it("uses feed defaults and bounds page inputs while honoring disabled queries", () => {
    api.query.mockReturnValue({ data: undefined });
    renderHook(useActivities);
    expect(api.query).toHaveBeenLastCalledWith(
      "get",
      "/activities",
      { params: { query: { page: 1, per_page: 10, sport: undefined } } },
      { enabled: true },
    );

    renderHook(() =>
      useActivities({ page: -2, perPage: 0, sport: "hike", enabled: false }),
    );
    expect(api.query).toHaveBeenLastCalledWith(
      "get",
      "/activities",
      { params: { query: { page: 1, per_page: 1, sport: "hike" } } },
      { enabled: false },
    );
  });
});
