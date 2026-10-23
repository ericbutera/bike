import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ACTIVITY_TYPES, type ActivityType } from "../../lib/activityTypes";
import ActivityDetailPanel from "../ActivityDetailPanel";
import MatchedSegmentsSection from "../MatchedSegmentsSection";
import ActivitySignalsCard from "../activity-detail/ActivitySignalsCard";
import RequireAuth from "../RequireAuth";
import type { RouteMapProps } from "../RouteMapTypes";
import { segmentEffort } from "../../tests/fixtures/activitySegments";
import {
  activityLap,
  climbOutput,
  savedClimb,
  climbingRoutePoints as makeClimbingRoutePoints,
} from "../../tests/fixtures/activities";

vi.mock("recharts", async (importOriginal) => {
  const actual = await importOriginal<typeof import("recharts")>();
  const React = await import("react");

  return {
    ...actual,
    ResponsiveContainer: ({ children }: { children: React.ReactNode }) => {
      if (React.isValidElement<{ width?: number; height?: number }>(children)) {
        return React.cloneElement(children, {
          width: 960,
          height: 240,
        });
      }

      return React.createElement(
        "div",
        { style: { width: 960, height: 240 } },
        children,
      );
    },
  };
});

const mocks = vi.hoisted(() => ({
  useCurrentUser: vi.fn(),
  useActivity: vi.fn(),
  useUpdateActivity: vi.fn(),
  useDeleteActivity: vi.fn(),
  useSegments: vi.fn(),
  useUpdateSegment: vi.fn(),
  updateSegmentAsync: vi.fn(),
  renderMapLibreRouteMap: vi.fn(),
  routerPush: vi.fn(),
}));

vi.mock("../../lib/auth", () => ({
  useAuth: () => ({
    user: mocks.useCurrentUser().user,
    isLoading: mocks.useCurrentUser().isLoading,
  }),
}));

vi.mock("../../lib/featureFlags", async () => {
  const actual = await vi.importActual<typeof import("../../lib/featureFlags")>(
    "../../lib/featureFlags",
  );
  return { ...actual, useFeatureFlag: () => true };
});

vi.mock("../../lib/activitySourceFiles", () => ({
  activitySourceFileUrl: (id: number | string) =>
    `http://localhost:3000/api/activities/${id}/source-file`,
}));

vi.mock("../../lib/queries", () => ({
  useActivity: mocks.useActivity,
  useUpdateActivity: mocks.useUpdateActivity,
  useDeleteActivity: mocks.useDeleteActivity,
  useSegments: mocks.useSegments,
  useUpdateSegment: mocks.useUpdateSegment,
}));

vi.mock("next/navigation", () => ({
  useRouter: () => ({
    push: mocks.routerPush,
  }),
}));

vi.mock("next/link", () => ({
  default: ({ href, children, ...props }: any) => (
    <a href={href} {...props}>
      {children}
    </a>
  ),
}));

vi.mock("../MapLibreRouteMap", () => ({
  default: (props: any) => {
    mocks.renderMapLibreRouteMap(props);
    return <div role="img" aria-label={props.ariaLabel} />;
  },
}));

function makeActivity(
  overrides: Partial<{
    climb_output: ReturnType<typeof climbOutput> | null;
    id: number;
    title: string;
    sport: string;
    source: string;
    activity_type: ActivityType;
    original_filename: string | null;
    format: string | null;
    started_at: string;
    location: string | null;
    ended_at: string | null;
    distance_meters: number | null;
    moving_time_seconds: number | null;
    total_time_seconds: number | null;
    elevation_gain_meters: number | null;
    elevation_loss_meters: number | null;
    average_speed_mps: number | null;
    max_speed_mps: number | null;
    average_heart_rate_bpm: number | null;
    max_heart_rate_bpm: number | null;
    average_cadence_rpm: number | null;
    max_cadence_rpm: number | null;
    calories: number | null;
    relative_effort: number | null;
    estimated_ftp_watts: number | null;
    heart_rate_zones: Array<{
      zone: number;
      label: string;
      min_bpm: number | null;
      max_bpm: number | null;
      duration_seconds: number;
      share_percent: number;
    }>;
    laps: Array<{
      lap_index: number;
      title: string;
      duration_seconds: number | null;
      distance_meters: number | null;
      average_speed_mps: number | null;
      average_heart_rate_bpm: number | null;
      max_heart_rate_bpm: number | null;
    }>;
    chart_points: Array<{
      elapsed_seconds: number;
      distance_meters: number | null;
      elevation_meters: number | null;
      speed_mps: number | null;
      heart_rate_bpm: number | null;
      cadence_rpm: number | null;
      power_watts: number | null;
    }>;
    route_points: Array<{
      elapsed_seconds: number;
      latitude: number;
      longitude: number;
      distance_meters: number | null;
      elevation_meters: number | null;
      speed_mps: number | null;
      heart_rate_bpm: number | null;
      cadence_rpm: number | null;
      power_watts: number | null;
    }>;
    segment_efforts: Array<{
      segment_id: number;
      segment_title: string;
      effort_index: number;
      duration_seconds: number;
      start_route_point_index: number;
      end_route_point_index: number;
      overall_rank?: number | null;
      personal_rank?: number | null;
      personal_best_duration_seconds?: number | null;
    }>;
    can_regenerate: boolean;
    can_download_source_file: boolean;
  }> = {},
) {
  return {
    id: 7,
    title: "Lunch Ride",
    sport: "ride",
    source: "manual_upload",
    activity_type: ACTIVITY_TYPES.Training,
    original_filename: "lunch-ride.tcx",
    format: "tcx",
    started_at: "2026-05-06T12:00:00Z",
    ended_at: "2026-05-06T13:00:00Z",
    distance_meters: 28000,
    moving_time_seconds: 3200,
    total_time_seconds: 3600,
    elevation_gain_meters: 310,
    elevation_loss_meters: 305,
    average_speed_mps: 8.7,
    max_speed_mps: 14.8,
    average_heart_rate_bpm: 144,
    max_heart_rate_bpm: 168,
    average_cadence_rpm: 84,
    max_cadence_rpm: 102,
    calories: 640,
    relative_effort: 106,
    estimated_ftp_watts: 265,
    heart_rate_zones: [
      {
        zone: 1,
        label: "Z1",
        min_bpm: null,
        max_bpm: 120,
        duration_seconds: 600,
        share_percent: 18.8,
      },
      {
        zone: 2,
        label: "Z2",
        min_bpm: 121,
        max_bpm: 140,
        duration_seconds: 1200,
        share_percent: 37.5,
      },
      {
        zone: 3,
        label: "Z3",
        min_bpm: 141,
        max_bpm: 155,
        duration_seconds: 800,
        share_percent: 25,
      },
      {
        zone: 4,
        label: "Z4",
        min_bpm: 156,
        max_bpm: 170,
        duration_seconds: 500,
        share_percent: 15.6,
      },
      {
        zone: 5,
        label: "Z5",
        min_bpm: 171,
        max_bpm: null,
        duration_seconds: 100,
        share_percent: 3.1,
      },
    ],
    laps: [activityLap({ title: "Lap 1" })],
    chart_points: [
      {
        elapsed_seconds: 0,
        distance_meters: 0,
        elevation_meters: 100,
        speed_mps: 0,
        heart_rate_bpm: 128,
        cadence_rpm: 80,
        power_watts: 142,
      },
      {
        elapsed_seconds: 1600,
        distance_meters: 14000,
        elevation_meters: 180,
        speed_mps: 8.7,
        heart_rate_bpm: 150,
        cadence_rpm: 88,
        power_watts: 248,
      },
      {
        elapsed_seconds: 3200,
        distance_meters: 28000,
        elevation_meters: 140,
        speed_mps: 14.8,
        heart_rate_bpm: 168,
        cadence_rpm: 102,
        power_watts: 314,
      },
    ],
    route_points: [
      {
        elapsed_seconds: 0,
        latitude: 45.0,
        longitude: -122.0,
        distance_meters: 0,
        elevation_meters: 100,
        speed_mps: 0,
        heart_rate_bpm: 128,
        cadence_rpm: 80,
        power_watts: 142,
      },
      {
        elapsed_seconds: 1600,
        latitude: 45.02,
        longitude: -121.98,
        distance_meters: 14000,
        elevation_meters: 180,
        speed_mps: 8.7,
        heart_rate_bpm: 150,
        cadence_rpm: 88,
        power_watts: 248,
      },
      {
        elapsed_seconds: 3200,
        latitude: 45.04,
        longitude: -121.96,
        distance_meters: 28000,
        elevation_meters: 140,
        speed_mps: 14.8,
        heart_rate_bpm: 168,
        cadence_rpm: 102,
        power_watts: 314,
      },
    ],
    segment_efforts: [
      {
        segment_id: 11,
        segment_title: "North Climb",
        effort_index: 1,
        duration_seconds: 312,
        start_route_point_index: 1,
        end_route_point_index: 2,
        overall_rank: 1,
        personal_rank: 1,
        personal_best_duration_seconds: 312,
      },
      {
        segment_id: 11,
        segment_title: "North Climb",
        effort_index: 2,
        duration_seconds: 330,
        start_route_point_index: 0,
        end_route_point_index: 1,
        overall_rank: 3,
        personal_rank: 2,
        personal_best_duration_seconds: 312,
      },
    ],
    climb_output: climbOutput([]),
    can_regenerate: true,
    can_download_source_file: true,
    ...overrides,
  };
}

function makeManyClimbRoutePoints(count: number) {
  const points: ReturnType<typeof makeClimbingRoutePoints> = [];
  let elapsedSeconds = 0;
  let distanceMeters = 0;
  let elevationMeters = 100;

  const pushPoint = () => {
    points.push({
      elapsed_seconds: elapsedSeconds,
      latitude: 45 + elapsedSeconds * 0.00001,
      longitude: -122 + elapsedSeconds * 0.00001,
      distance_meters: distanceMeters,
      elevation_meters: elevationMeters,
      speed_mps: 5,
      heart_rate_bpm: 145,
      cadence_rpm: 84,
      power_watts: 220,
    });
  };

  pushPoint();

  for (let climbIndex = 0; climbIndex < count; climbIndex += 1) {
    for (let step = 0; step < 4; step += 1) {
      elapsedSeconds += 120;
      distanceMeters += 600;
      elevationMeters += 30;
      pushPoint();
    }

    if (climbIndex < count - 1) {
      elapsedSeconds += 90;
      distanceMeters += 300;
      elevationMeters -= 40;
      pushPoint();
    }
  }

  return points;
}

function makeMatchedActivity(segmentCount: number) {
  return makeActivity({
    segment_efforts: Array.from({ length: segmentCount }, (_, index) =>
      segmentEffort({
        segment_id: index + 1,
        segment_title: `Segment ${String(index + 1).padStart(2, "0")}`,
      }),
    ),
  });
}

function currentMap() {
  return mocks.renderMapLibreRouteMap.mock.calls
    .map(([props]) => props as RouteMapProps)
    .filter((props) => props.ariaLabel === "Activity route map")
    .at(-1)!;
}

function currentSegmentOverlays() {
  return (currentMap().overlays ?? []).filter((overlay) =>
    /^\d+-/.test(overlay.id),
  );
}

describe("ActivityDetailPanel", () => {
  it.each([null, climbOutput([])])(
    "uses saved output instead of detecting climbs from the route: %s",
    async (output) => {
      const user = userEvent.setup();
      mocks.useActivity.mockReturnValue({
        data: makeActivity({
          route_points: makeClimbingRoutePoints(),
          climb_output: output,
          segment_efforts: [],
        }),
      });
      render(<ActivityDetailPanel activityId={7} />);
      expect(
        screen.queryByRole("button", { name: "Show climb 1 details" }),
      ).not.toBeInTheDocument();
      expect(
        screen.getByText("No sustained climbs found."),
      ).toBeInTheDocument();
      expect(
        screen.getByRole("tab", {
          name: "Climbs (0)",
        }),
      ).toBeInTheDocument();
      act(() => currentMap().overlayLayers?.onChange("climbs", true));
      expect(currentMap().climbOverlays).toEqual([]);
      await user.click(screen.getByRole("tab", { name: /Climbs/ }));
    },
  );

  it("retains saved metrics when a climb has no route geometry", async () => {
    const user = userEvent.setup();
    mocks.useActivity.mockReturnValue({
      data: makeActivity({
        segment_efforts: [],
        climb_output: climbOutput([
          savedClimb({
            route_points: [],
            start_route_point_index: null,
            end_route_point_index: null,
            average_grade_percent: null,
          }),
        ]),
      }),
    });
    render(<ActivityDetailPanel activityId={7} />);
    expect(screen.getByText("410 ft")).toBeInTheDocument();
    await user.click(
      screen.getByRole("button", { name: "Show climb 1 details" }),
    );
    expect(
      screen.getByRole("button", { name: "View Climb 1 on map" }),
    ).toBeDisabled();
    expect(
      screen.queryByRole("img", { name: "Climb 1 elevation profile" }),
    ).not.toBeInTheDocument();
    expect(screen.getByText("Activity distance: --")).toBeInTheDocument();
  });

  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(HTMLDialogElement.prototype, "showModal").mockImplementation(
      function (this: HTMLDialogElement) {
        this.setAttribute("open", "");
      },
    );
    vi.stubGlobal(
      "confirm",
      vi.fn(() => true),
    );
    mocks.useCurrentUser.mockReturnValue({
      user: { id: 1, email: "rider@example.com" },
      isLoading: false,
    });
    mocks.useActivity.mockReturnValue({
      data: makeActivity(),
      isLoading: false,
      isError: false,
      error: null,
    });
    mocks.useUpdateActivity.mockReturnValue({
      updateAsync: vi.fn().mockResolvedValue(makeActivity()),
      isPending: false,
      isError: false,
      error: null,
    });
    mocks.useDeleteActivity.mockReturnValue({
      deleteAsync: vi.fn().mockResolvedValue(undefined),
      isPending: false,
      isError: false,
      error: null,
    });
    mocks.useSegments.mockReturnValue({
      data: [],
      isLoading: false,
      isError: false,
      error: null,
    });
    mocks.useUpdateSegment.mockReturnValue({
      updateAsync: mocks.updateSegmentAsync,
      isPending: false,
      isError: false,
      error: null,
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("hides the matched-segments card when an activity has no matches", () => {
    mocks.useActivity.mockReturnValue({
      data: makeActivity({ sport: "hike", segment_efforts: [] }),
    });
    const { container } = render(
      <MatchedSegmentsSection
        activityId={7}
        selectedSegmentIds={[]}
        onToggleSegmentMatch={vi.fn()}
      />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("shows decorative location and clock icons beside the header metadata", () => {
    mocks.useActivity.mockReturnValue({
      data: makeActivity({ location: "Traverse City, Michigan" }),
    });
    render(<ActivityDetailPanel activityId={7} />);
    const location = screen.getByText("Traverse City, Michigan");
    expect(location.querySelector("svg")).toHaveAttribute(
      "data-icon",
      "location-dot",
    );
    expect(location.querySelector("svg")).toHaveAttribute(
      "aria-hidden",
      "true",
    );
    expect(location.nextElementSibling?.tagName).toBe("TIME");
    const clock = location.nextElementSibling?.querySelector("svg");
    expect(clock).toHaveAttribute("data-icon", "clock");
    expect(clock).toHaveAttribute("aria-hidden", "true");
    expect(location.nextElementSibling).toHaveAttribute(
      "datetime",
      "2026-05-06T12:00:00Z",
    );
    expect(
      screen.getByRole("heading", { name: "Lunch Ride" }).closest(".card"),
    ).toBeNull();
  });

  it("omits the location group when no location is available", () => {
    mocks.useActivity.mockReturnValue({
      data: makeActivity({ location: null }),
    });
    const { container } = render(<ActivityDetailPanel activityId={7} />);
    expect(
      container.querySelector('[data-icon="location-dot"]'),
    ).not.toBeInTheDocument();
    expect(container.querySelector("time")).toHaveAttribute(
      "datetime",
      "2026-05-06T12:00:00Z",
    );
  });

  it("renders the summary metrics, laps, and charts", () => {
    render(<ActivityDetailPanel activityId={7} />);

    expect(
      screen.getByRole("heading", { name: "Lunch Ride" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Relative effort")).toBeInTheDocument();
    expect(screen.getByText("106")).toBeInTheDocument();
    expect(screen.getByText("Activity data")).toBeInTheDocument();
    expect(screen.getByText("Avg")).toBeInTheDocument();
    expect(screen.getByText("Max")).toBeInTheDocument();
    expect(screen.getByText("17.4 mi")).toBeInTheDocument();
    expect(screen.getByText("53m 20s")).toBeInTheDocument();
    expect(screen.getAllByText("19.5 mph").length).toBeGreaterThan(0);
    expect(screen.getAllByText("168 bpm").length).toBeGreaterThan(0);
    expect(screen.queryByText("lunch-ride.tcx")).not.toBeInTheDocument();
    expect(screen.getByText("Above 170 bpm")).toBeInTheDocument();
    expect(
      screen.getByRole("img", { name: "Activity route map" }),
    ).toBeInTheDocument();
    expect(mocks.renderMapLibreRouteMap).toHaveBeenCalledWith(
      expect.objectContaining({
        ariaLabel: "Activity route map",
        showZoomControls: true,
        defaultBasemap: "default",
      }),
    );
    expect(screen.getByText("Matched segments")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Highlight North Climb" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Show time & runs" }),
    ).toHaveAttribute("aria-expanded", "false");
    expect(
      screen.queryByRole("img", { name: "North Climb attempts chart" }),
    ).not.toBeInTheDocument();
    expect(
      within(
        screen.getByRole("region", { name: "Matched segments" }),
      ).queryByText("Time"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText(
        "Hover or tap a point to see leaderboard position and max heart rate.",
      ),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("link", { name: /5m 12s/i }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("link", { name: /5m 30s/i }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: "North Climb" })).toHaveAttribute(
      "href",
      "/segments/11",
    );
    expect(screen.queryByText("Attempt trend")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("link", { name: "Open segment detail" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("link", { name: "Compare efforts" }),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("Summary overview")).not.toBeInTheDocument();
    expect(screen.queryByText("KOM")).not.toBeInTheDocument();
    expect(screen.queryByText("PR")).not.toBeInTheDocument();
    expect(screen.queryByText("Best 5m 12s")).not.toBeInTheDocument();
    expect(
      screen.queryByText("Leaderboard #1 overall"),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("Peak HR 168 bpm")).not.toBeInTheDocument();
    expect(screen.queryByText("Trending faster")).not.toBeInTheDocument();
    expect(screen.queryByText("High heart rate")).not.toBeInTheDocument();
    expect(screen.getByText(/^2 runs?$/i)).toBeInTheDocument();
    expect(screen.queryByText(/High heart rate at/i)).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Regenerate derived data" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("link", { name: "Download original source" }),
    ).toHaveAttribute(
      "href",
      "http://localhost:3000/api/activities/7/source-file",
    );
    expect(
      screen.getByRole("button", { name: "Delete activity" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Lap splits")).toBeInTheDocument();
    expect(screen.getByLabelText("Lap splits details")).toBeInTheDocument();
    expect(
      screen.queryByText(/These lap rollups come from/i),
    ).not.toBeInTheDocument();
    expect(
      within(screen.getByRole("table", { name: "Lap splits" })).getByRole(
        "cell",
        { name: "Lap 1" },
      ),
    ).toBeInTheDocument();
    expect(screen.getByText("1 lap").closest(".badge")).toBeNull();
    expect(
      screen.getByRole("region", { name: "Activity summary" }),
    ).not.toContainElement(
      screen.getByRole("region", { name: "Activity data" }),
    );
    expect(
      screen.queryByLabelText("Activity data details"),
    ).not.toBeInTheDocument();
    expect(screen.getByText("Ride signals")).toBeInTheDocument();
    expect(
      screen.getByRole("img", { name: "Activity signals chart" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Heart rate" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByRole("button", { name: "Power" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
    expect(screen.getByRole("button", { name: "Speed" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
    expect(screen.getByRole("button", { name: "Elevation" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(
      screen.queryByRole("link", { name: "Back to activities" }),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("Import trace")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "View import trace" }),
    ).not.toBeInTheDocument();
  });

  it("keeps detected climbs off the overall route map by default", () => {
    mocks.useActivity.mockReturnValue({
      data: makeActivity({
        route_points: makeClimbingRoutePoints(),
        climb_output: climbOutput(),
        segment_efforts: [segmentEffort()],
      }),
    });
    render(<ActivityDetailPanel activityId={7} />);

    const overlays = currentMap().overlays ?? [];
    expect(overlays.map((overlay) => overlay.id)).toEqual(["11-1"]);
    expect(currentMap().climbOverlays).toEqual([]);
    expect(currentMap().overlayLayers).toMatchObject({
      segments: true,
      climbs: false,
    });
  });

  it("shares a card between segments and climbs while retaining detail selections", async () => {
    const user = userEvent.setup();
    mocks.useActivity.mockReturnValue({
      data: makeActivity({
        route_points: makeClimbingRoutePoints(),
        climb_output: climbOutput(),
      }),
    });
    render(<ActivityDetailPanel activityId={7} />);
    const card = screen.getByRole("region", { name: "Segments and climbs" });
    const segments = screen.getByRole("tab", { name: "Segments (1)" });
    const climbs = screen.getByRole("tab", { name: "Climbs (1)" });
    expect(segments).toHaveAttribute("aria-selected", "true");
    expect(within(card).getAllByRole("tabpanel")).toHaveLength(1);
    expect(card.querySelector(".card")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Show time & runs" }));
    const overview = currentMap();
    await user.click(climbs);
    expect(
      screen.queryByRole("link", { name: "North Climb" }),
    ).not.toBeInTheDocument();
    await user.click(
      screen.getByRole("button", { name: "Show climb 1 details" }),
    );
    expect(
      screen.getByRole("img", { name: "Climb 1 elevation profile" }),
    ).toBeInTheDocument();
    await user.click(segments);
    expect(
      screen.getByRole("img", { name: "North Climb attempts chart" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("img", { name: "Climb 1 elevation profile" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Highlight North Climb" }),
    ).toHaveAttribute("aria-pressed", "true");
    await user.click(climbs);
    expect(
      screen.getByRole("button", { name: "Show climb 1 details" }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(
      screen.getByRole("img", { name: "Climb 1 elevation profile" }),
    ).toBeInTheDocument();
    expect(currentMap()).toBe(overview);
    expect(currentMap().overlayLayers).toMatchObject({
      segments: true,
      climbs: false,
    });
  });

  it("supports keyboard tab focus, selection, and wrapping", async () => {
    const user = userEvent.setup();
    mocks.useActivity.mockReturnValue({
      data: makeActivity({
        route_points: makeClimbingRoutePoints(),
        climb_output: climbOutput(),
      }),
    });
    render(<ActivityDetailPanel activityId={7} />);
    const segments = screen.getByRole("tab", { name: "Segments (1)" });
    const climbs = screen.getByRole("tab", { name: "Climbs (1)" });
    act(() => segments.focus());
    for (const [key, target] of [
      ["{ArrowRight}", climbs],
      ["{ArrowRight}", segments],
      ["{ArrowLeft}", climbs],
      ["{Home}", segments],
      ["{End}", climbs],
    ] as const) {
      await user.keyboard(key);
      expect(target).toHaveFocus();
      expect(target).toHaveAttribute("aria-selected", "true");
      expect(target).toHaveAttribute("tabindex", "0");
      expect(screen.getByRole("tabpanel")).toHaveAttribute(
        "aria-labelledby",
        target.id,
      );
      expect(target).toHaveAttribute(
        "aria-controls",
        screen.getByRole("tabpanel").id,
      );
    }
  });

  it("resets detail tabs and climb selection when navigating to another activity", async () => {
    const user = userEvent.setup();
    const activity = makeActivity({
      route_points: makeClimbingRoutePoints(),
      climb_output: climbOutput(),
    });
    mocks.useActivity.mockReturnValue({ data: activity });
    const { rerender } = render(<ActivityDetailPanel activityId={7} />);
    await user.click(screen.getByRole("tab", { name: "Climbs (1)" }));
    await user.click(
      screen.getByRole("button", { name: "Show climb 1 details" }),
    );
    mocks.useActivity.mockReturnValue({ data: { ...activity, id: 8 } });
    rerender(<ActivityDetailPanel activityId={8} />);
    expect(screen.getByRole("tab", { name: "Segments (1)" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await user.click(screen.getByRole("tab", { name: "Climbs (1)" }));
    expect(
      screen.getByRole("button", { name: "Show climb 1 details" }),
    ).toHaveAttribute("aria-pressed", "false");
  });

  it.each([
    { segments: true, climbs: false, tabs: ["Segments (1)", "Climbs (0)"] },
    { segments: false, climbs: true, tabs: ["Climbs (1)"] },
    { segments: false, climbs: false, tabs: ["Climbs (0)"] },
  ])(
    "offers saved climb results alongside populated segments: $tabs",
    ({ segments, climbs, tabs }) => {
      mocks.useActivity.mockReturnValue({
        data: makeActivity({
          route_points: climbs ? makeClimbingRoutePoints() : [],
          climb_output: climbOutput(climbs ? [savedClimb()] : []),
          segment_efforts: segments ? [segmentEffort()] : [],
        }),
      });
      render(<ActivityDetailPanel activityId={7} />);
      expect(
        screen
          .queryAllByRole("tab")
          .map((tab) => tab.getAttribute("aria-label")),
      ).toEqual(tabs);
      if (tabs.length === 0) {
        expect(
          screen.queryByRole("region", { name: "Segments and climbs" }),
        ).not.toBeInTheDocument();
      } else {
        expect(screen.getByRole("tab", { name: tabs[0] })).toHaveAttribute(
          "aria-selected",
          "true",
        );
      }
    },
  );

  it("toggles climb and segment layers independently and restores the selected segments", async () => {
    const user = userEvent.setup();
    const activity = {
      ...makeMatchedActivity(2),
      route_points: makeClimbingRoutePoints(),
      climb_output: climbOutput(),
    };
    mocks.useActivity.mockReturnValue({ data: activity });
    const { rerender } = render(<ActivityDetailPanel activityId={7} />);
    const first = screen.getByRole("button", { name: "Highlight Segment 01" });
    await user.click(first);
    await user.unhover(first);
    const points = currentMap().routePoints;
    act(() => currentMap().overlayLayers?.onChange("climbs", true));
    expect(currentMap().climbOverlays).toEqual([
      expect.objectContaining({
        id: "climb-1",
        points: makeClimbingRoutePoints(),
      }),
    ]);
    expect(currentMap().overlays).toHaveLength(2);
    expect(screen.getByText("Climb start → finish")).toBeInTheDocument();
    act(() => currentMap().overlayLayers?.onChange("segments", false));
    expect(currentMap().overlays).toEqual([]);
    expect(currentMap().climbOverlays).toHaveLength(1);
    expect(
      screen.queryByRole("group", { name: "Map segments" }),
    ).not.toBeInTheDocument();
    act(() => currentMap().overlayLayers?.onChange("segments", true));
    expect(
      screen.getByRole("button", { name: "Highlight Segment 01" }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(
      currentMap().overlays?.find((overlay) => overlay.id === "2-1")?.color,
    ).toBe("#0060df");
    act(() => currentMap().overlayLayers?.onChange("climbs", false));
    expect(currentMap().climbOverlays).toEqual([]);
    expect(currentMap().routePoints).toBe(points);
    expect(currentMap().fitBoundsPoints).toBeUndefined();
    act(() => currentMap().overlayLayers?.onChange("segments", false));
    expect(currentMap().overlays).toEqual([]);
    act(() => currentMap().overlayLayers?.onChange("climbs", true));
    mocks.useActivity.mockReturnValue({ data: { ...activity, id: 8 } });
    rerender(<ActivityDetailPanel activityId={8} />);
    expect(currentMap().overlayLayers).toMatchObject({
      segments: true,
      climbs: false,
    });
  });

  it("matches ten distinct segment colors to status dots and cycles for larger activities", () => {
    const activity = makeMatchedActivity(22);
    activity.segment_efforts.push(
      segmentEffort({
        segment_id: 1,
        segment_title: "Segment 01",
        effort_index: 2,
      }),
    );
    mocks.useActivity.mockReturnValue({ data: activity });
    render(<ActivityDetailPanel activityId={7} />);

    const overlays = currentSegmentOverlays();
    const colors = overlays
      .filter((overlay) => overlay.id.endsWith("-1"))
      .map((overlay) => overlay.color);
    expect(new Set(colors.slice(0, 10)).size).toBe(10);
    expect(colors.slice(10, 20)).toEqual(colors.slice(0, 10));
    expect(colors).not.toContain("#0060df");
    expect(colors).not.toContain("#f59e0b");
    expect(overlays.find((overlay) => overlay.id === "1-2")?.color).toBe(
      colors[0],
    );
    for (const overlay of overlays) {
      expect(overlay).toMatchObject({
        weight: 6,
        casingColor: "#ffffff",
        casingWeight: 9,
      });
    }
    const buttons = within(
      screen.getByRole("group", { name: "Map segments" }),
    ).getAllByRole("button");
    buttons.forEach((button, index) => {
      expect(button.querySelector(".status")).toHaveStyle({
        backgroundColor: colors[index],
      });
      const card = document.getElementById(`activity-segment-${index + 1}`);
      expect(card?.querySelector(".status")).toHaveStyle({
        backgroundColor: colors[index],
      });
    });
    expect(
      screen.queryByRole("button", { name: /Clear selection/ }),
    ).not.toBeInTheDocument();
  });

  it("previews segments from the map and legend, preserves multiple selections, and clears without jumping", async () => {
    const user = userEvent.setup();
    mocks.useActivity.mockReturnValue({ data: makeMatchedActivity(3) });
    render(<ActivityDetailPanel activityId={7} />);
    const colors = currentSegmentOverlays().map((overlay) => overlay.color);
    const first = screen.getByRole("button", { name: "Highlight Segment 01" });
    const second = screen.getByRole("button", { name: "Highlight Segment 02" });
    const third = screen.getByRole("button", { name: "Highlight Segment 03" });
    const card = document.getElementById("activity-segment-1")!;
    const scrollIntoView = vi.fn();
    card.scrollIntoView = scrollIntoView;
    const detailFocus = vi.spyOn(
      within(card).getByRole("link", { name: "Segment 01" }),
      "focus",
    );
    const colorFor = (id: string) =>
      currentSegmentOverlays().find((overlay) => overlay.id === id)?.color;

    await user.hover(second);
    expect(colorFor("1-1")).toBe("#0060df");
    expect(colorFor("2-1")).toBe(colors[1]);
    expect(colorFor("3-1")).toBe("#0060df");
    await user.unhover(second);
    expect(currentSegmentOverlays().map((overlay) => overlay.color)).toEqual(
      colors,
    );

    await user.click(first);
    await user.unhover(first);
    await user.click(second);
    await user.unhover(second);
    expect(first).toHaveAttribute("aria-pressed", "true");
    expect(second).toHaveAttribute("aria-pressed", "true");
    expect(colorFor("1-1")).toBe(colors[0]);
    expect(colorFor("2-1")).toBe(colors[1]);
    expect(colorFor("3-1")).toBe("#0060df");
    expect(scrollIntoView).not.toHaveBeenCalled();
    expect(detailFocus).not.toHaveBeenCalled();

    await user.hover(third);
    expect(colorFor("1-1")).toBe("#0060df");
    expect(colorFor("2-1")).toBe("#0060df");
    expect(colorFor("3-1")).toBe(colors[2]);
    await user.unhover(third);
    expect(colorFor("1-1")).toBe(colors[0]);
    expect(colorFor("2-1")).toBe(colors[1]);

    act(() =>
      currentSegmentOverlays()
        .find((overlay) => overlay.id === "3-1")
        ?.onMouseEnter?.(),
    );
    expect(colorFor("1-1")).toBe("#0060df");
    expect(currentSegmentOverlays().at(-1)?.id).toBe("3-1");
    act(() =>
      currentSegmentOverlays()
        .find((overlay) => overlay.id === "3-1")
        ?.onClick?.(),
    );
    act(() =>
      currentSegmentOverlays()
        .find((overlay) => overlay.id === "3-1")
        ?.onMouseLeave?.(),
    );
    expect(third).toHaveAttribute("aria-pressed", "true");
    await user.click(first);
    await user.unhover(first);
    expect(first).toHaveAttribute("aria-pressed", "false");
    expect(colorFor("1-1")).toBe("#0060df");
    await user.click(
      screen.getByRole("button", { name: "Clear selection (2)" }),
    );
    expect(currentSegmentOverlays().map((overlay) => overlay.color)).toEqual(
      colors,
    );
    expect(first).toHaveAttribute("aria-pressed", "false");
    expect(second).toHaveAttribute("aria-pressed", "false");
    expect(third).toHaveAttribute("aria-pressed", "false");
  });

  it("keeps the current legend preview when earlier hover targets leave", async () => {
    const user = userEvent.setup();
    mocks.useActivity.mockReturnValue({ data: makeMatchedActivity(3) });
    render(<ActivityDetailPanel activityId={7} />);
    const initial = currentSegmentOverlays();
    const mapSegment = initial.find((overlay) => overlay.id === "1-1")!;
    const second = screen.getByRole("button", { name: "Highlight Segment 02" });
    const third = screen.getByRole("button", { name: "Highlight Segment 03" });
    const colorFor = (id: string) =>
      currentSegmentOverlays().find((overlay) => overlay.id === id)?.color;

    act(() => mapSegment.onMouseEnter?.());
    await user.hover(second);
    act(() => mapSegment.onMouseEnter?.());
    expect(colorFor("1-1")).toBe("#0060df");
    expect(colorFor("2-1")).toBe(initial[1].color);
    act(() => mapSegment.onMouseLeave?.());
    expect(colorFor("1-1")).toBe("#0060df");
    expect(colorFor("2-1")).toBe(initial[1].color);
    expect(colorFor("3-1")).toBe("#0060df");
    await user.hover(third);
    fireEvent.mouseLeave(second);
    expect(colorFor("1-1")).toBe("#0060df");
    expect(colorFor("2-1")).toBe("#0060df");
    expect(colorFor("3-1")).toBe(initial[2].color);
    await user.unhover(third);
    expect(currentSegmentOverlays().map((overlay) => overlay.color)).toEqual(
      initial.map((overlay) => overlay.color),
    );
  });

  it("tracks the latest segment during fast forward and reverse hover sweeps", () => {
    mocks.useActivity.mockReturnValue({ data: makeMatchedActivity(22) });
    render(<ActivityDetailPanel activityId={7} />);
    const initial = currentSegmentOverlays();
    const buttons = within(
      screen.getByRole("group", { name: "Map segments" }),
    ).getAllByRole("button");
    fireEvent.click(buttons[0]);
    fireEvent.click(buttons[1]);

    for (const sweep of [buttons, [...buttons].reverse()]) {
      act(() => {
        sweep.forEach((button, index) => {
          fireEvent.mouseEnter(button);
          if (index > 0) fireEvent.mouseLeave(sweep[index - 1]);
          initial[0].onMouseLeave?.();
        });
      });
      const latestIndex = buttons.indexOf(sweep.at(-1)!);
      expect(
        currentSegmentOverlays().filter(
          (overlay) => overlay.color !== "#0060df",
        ),
      ).toEqual([
        expect.objectContaining({
          id: `${latestIndex + 1}-1`,
          color: initial[latestIndex].color,
        }),
      ]);
      expect(
        currentSegmentOverlays()
          .filter((overlay) => overlay.id !== `${latestIndex + 1}-1`)
          .every((overlay) => overlay.color === "#0060df"),
      ).toBe(true);
    }
    fireEvent.mouseLeave(buttons[0]);
    expect(
      currentSegmentOverlays()
        .filter((overlay) => overlay.color !== "#0060df")
        .map((overlay) => overlay.id),
    ).toEqual(["1-1", "2-1"]);
    expect(buttons[0]).toHaveAttribute("aria-pressed", "true");
    expect(buttons[1]).toHaveAttribute("aria-pressed", "true");
  });

  it("offers a labeled inline clear button only for selected segments and clears an active preview", async () => {
    const user = userEvent.setup();
    mocks.useActivity.mockReturnValue({ data: makeMatchedActivity(3) });
    render(<ActivityDetailPanel activityId={7} />);
    const initial = currentSegmentOverlays();
    const legend = screen.getByRole("group", { name: "Map segments" });
    const buttons = within(legend).getAllByRole("button");
    expect(
      screen.queryByRole("button", { name: /Clear selection/ }),
    ).not.toBeInTheDocument();
    await user.click(buttons[0]);
    await user.click(buttons[1]);
    await user.hover(buttons[2]);
    const clear = within(legend).getByRole("button", {
      name: "Clear selection (2)",
    });
    expect(legend.lastElementChild).toBe(clear);
    expect(
      clear.querySelector('svg[data-icon="rectangle-xmark"]'),
    ).toBeInTheDocument();
    expect(clear).toHaveAttribute("title", "Clear selection (2)");
    expect(clear).toHaveTextContent("Clear selection");
    fireEvent.click(clear);
    expect(currentSegmentOverlays().map((overlay) => overlay.color)).toEqual(
      initial.map((overlay) => overlay.color),
    );
    buttons.forEach((button) =>
      expect(button).toHaveAttribute("aria-pressed", "false"),
    );
    expect(
      screen.queryByRole("button", { name: /Clear selection/ }),
    ).not.toBeInTheDocument();
  });

  it("supports keyboard segment selection and resets selection for a new activity", async () => {
    const user = userEvent.setup();
    mocks.useActivity.mockReturnValue({ data: makeMatchedActivity(2) });
    const { rerender } = render(<ActivityDetailPanel activityId={7} />);
    const button = screen.getByRole("button", { name: "Highlight Segment 01" });
    act(() => button.focus());
    expect(
      currentSegmentOverlays().find((overlay) => overlay.id === "2-1")?.color,
    ).toBe("#0060df");
    await user.keyboard("{Enter}");
    expect(button).toHaveAttribute("aria-pressed", "true");
    await user.keyboard(" ");
    expect(button).toHaveAttribute("aria-pressed", "false");
    act(() => button.blur());
    expect(
      currentSegmentOverlays().every((overlay) => overlay.color !== "#0060df"),
    ).toBe(true);
    act(() => button.focus());
    await user.keyboard("{Enter}");
    const nextActivity = { ...makeMatchedActivity(2), id: 8 };
    mocks.useActivity.mockReturnValue({ data: nextActivity });
    rerender(<ActivityDetailPanel activityId={8} />);
    expect(
      screen.getByRole("button", { name: "Highlight Segment 01" }),
    ).toHaveAttribute("aria-pressed", "false");
    expect(
      currentSegmentOverlays().every((overlay) => overlay.color !== "#0060df"),
    ).toBe(true);
  });

  it("shows climb details independently and opens its route in a separate map", async () => {
    const user = userEvent.setup();

    mocks.useActivity.mockReturnValue({
      data: makeActivity({
        route_points: makeClimbingRoutePoints(),
        climb_output: climbOutput(),
        segment_efforts: [],
      }),
      isLoading: false,
      isError: false,
      error: null,
    });

    render(<ActivityDetailPanel activityId={7} />);

    expect(screen.getByRole("tab", { name: "Climbs (1)" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByText("1.5 mi")).toBeInTheDocument();
    expect(screen.getByText("410 ft")).toBeInTheDocument();
    expect(screen.getByText("5.2%")).toBeInTheDocument();

    const climbRow = screen.getByRole("button", {
      name: "Show climb 1 details",
    });

    expect(climbRow).toHaveAttribute("aria-pressed", "false");
    const overview = currentMap();
    expect(overview.overlays).toEqual([]);

    await user.click(climbRow);

    expect(
      screen.getByRole("button", { name: "Show climb 1 details" }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(
      screen.getByRole("heading", { name: "Climb 1" }),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Activity distance: 0 - 1.5 mi"),
    ).toBeInTheDocument();
    expect(screen.getByText("elevation gain")).toBeInTheDocument();
    expect(screen.getByText("elevation loss")).toBeInTheDocument();
    expect(screen.getByText("category")).toBeInTheDocument();
    expect(screen.getByText("max grade")).toBeInTheDocument();
    expect(screen.getByText("avg grade")).toBeInTheDocument();
    expect(screen.getByText("estimated")).toBeInTheDocument();
    expect(screen.getByText("8m 00s")).toBeInTheDocument();
    expect(
      screen.getByRole("img", { name: "Climb 1 elevation profile" }),
    ).toBeInTheDocument();
    expect(currentMap()).toBe(overview);
    expect(
      screen.queryByRole("button", { name: "Zoom out map" }),
    ).not.toBeInTheDocument();
    const viewMap = screen.getByRole("button", { name: "View Climb 1 on map" });
    expect(viewMap.querySelector('svg[data-icon="route"]')).toBeInTheDocument();
    expect(viewMap).toHaveAttribute("title", "View Climb 1 on map");
    expect(viewMap).not.toHaveTextContent("View on map");
    await user.click(viewMap);
    const dialog = screen.getByRole("dialog", { name: "Climb 1" });
    expect(
      within(dialog).getByRole("img", { name: "Climb 1 map" }),
    ).toBeInTheDocument();
    expect(mocks.renderMapLibreRouteMap.mock.lastCall?.[0]).toMatchObject({
      routePoints: makeClimbingRoutePoints(),
      routeColor: "#0060df",
      showZoomControls: true,
      fitBoundsMaxZoom: 16,
    });
    await user.click(within(dialog).getByRole("button", { name: "Close" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(
      screen.getByRole("img", { name: "Climb 1 elevation profile" }),
    ).toBeInTheDocument();
    expect(currentMap()).toBe(overview);
  });

  it("views a matched segment without changing its selection or expanding its details", async () => {
    const user = userEvent.setup();
    const activity = makeActivity();
    const scrollIntoView = vi.fn();
    render(<ActivityDetailPanel activityId={7} />);
    document.getElementById("activity-segment-11")!.scrollIntoView =
      scrollIntoView;
    const overview = currentMap();
    const selection = screen.getByRole("button", {
      name: "Highlight North Climb",
    });
    const viewMap = screen.getByRole("button", {
      name: "View North Climb on map",
    });
    expect(viewMap.querySelector('svg[data-icon="route"]')).toBeInTheDocument();
    expect(viewMap).toHaveAttribute("title", "View North Climb on map");
    expect(viewMap).not.toHaveTextContent("View on map");
    act(() => viewMap.focus());
    await user.keyboard("{Enter}");
    const dialog = screen.getByRole("dialog", { name: "North Climb" });
    const effort = activity.segment_efforts[0];
    expect(mocks.renderMapLibreRouteMap.mock.lastCall?.[0]).toMatchObject({
      ariaLabel: "North Climb map",
      routePoints: activity.route_points.slice(
        effort.start_route_point_index,
        effort.end_route_point_index + 1,
      ),
      routeColor: overview.overlays?.[0].color,
    });
    expect(
      mocks.renderMapLibreRouteMap.mock.lastCall?.[0].overlays,
    ).toBeUndefined();
    expect(selection).toHaveAttribute("aria-pressed", "false");
    expect(
      screen.queryByRole("img", { name: "North Climb attempts chart" }),
    ).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Close" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(selection).toHaveAttribute("aria-pressed", "false");
    await user.click(viewMap);
    fireEvent(
      screen.getByRole("dialog"),
      new Event("cancel", { cancelable: true }),
    );
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(currentMap()).toBe(overview);
    expect(scrollIntoView).not.toHaveBeenCalled();
  });

  it("disables a focused segment map when its route geometry is missing", () => {
    mocks.useActivity.mockReturnValue({
      data: makeActivity({ route_points: [] }),
    });
    render(<ActivityDetailPanel activityId={7} />);
    expect(
      screen.getByRole("button", { name: "View North Climb on map" }),
    ).toBeDisabled();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it.each([20, 21])(
    "keeps twenty climb rows visible with %i records",
    (count) => {
      mocks.useActivity.mockReturnValue({
        data: makeActivity({
          route_points: makeManyClimbRoutePoints(count),
          climb_output: climbOutput(
            Array.from({ length: count }, (_, index) =>
              savedClimb({ climb_number: index + 1 }),
            ),
          ),
          segment_efforts: [],
        }),
        isLoading: false,
        isError: false,
        error: null,
      });

      render(<ActivityDetailPanel activityId={7} />);

      expect(
        screen.getByRole("tab", { name: `Climbs (${count})` }),
      ).toBeInTheDocument();
      expect(
        screen.getByRole("button", { name: `Show climb ${count} details` }),
      ).toBeInTheDocument();

      const climbScroll = screen.getByTestId("activity-climbs-table-scroll");

      if (count > 20) {
        expect(climbScroll).toHaveClass("overflow-y-auto");
        expect(climbScroll.style.maxHeight).toBe("calc(52.5rem + 2px)");
      } else {
        expect(climbScroll).not.toHaveClass("overflow-y-auto");
        expect(climbScroll.style.maxHeight).toBe("");
      }
    },
  );

  it("shows the correct run details when a chart point is hovered", () => {
    render(<ActivityDetailPanel activityId={7} />);

    fireEvent.click(screen.getByRole("button", { name: "Show time & runs" }));

    fireEvent.mouseEnter(screen.getByLabelText("North Climb run 1 point"));

    expect(screen.getByText("Run 1 · 5m 12s")).toBeInTheDocument();
    expect(
      screen.getByText("Leaderboard #1 overall · Max heart rate 168 bpm"),
    ).toBeInTheDocument();
    expect(screen.getByText("Personal rank #1 all-time")).toBeInTheDocument();
    expect(screen.getByText("At PR")).toBeInTheDocument();
    expect(screen.getAllByText("KOM").length).toBeGreaterThan(0);
    expect(screen.queryByText("PR")).not.toBeInTheDocument();
    expect(screen.queryByText("Fastest run today")).not.toBeInTheDocument();

    fireEvent.mouseLeave(screen.getByLabelText("North Climb run 1 point"));
    fireEvent.mouseEnter(screen.getByLabelText("North Climb run 2 point"));

    expect(screen.getByText("Run 2 · 5m 30s")).toBeInTheDocument();
    expect(
      screen.getByText("Leaderboard #3 overall · Max heart rate 150 bpm"),
    ).toBeInTheDocument();
    expect(screen.getByText("Personal rank #2 all-time")).toBeInTheDocument();
    expect(screen.getByText("18s off PR")).toBeInTheDocument();
    expect(screen.getByText("Top 3")).toBeInTheDocument();
  });

  it("prefers a top-10 finish over PR and fastest for the same attempt", () => {
    mocks.useActivity.mockReturnValue({
      data: makeActivity({
        segment_efforts: [
          {
            segment_id: 11,
            segment_title: "North Climb",
            effort_index: 1,
            duration_seconds: 312,
            start_route_point_index: 1,
            end_route_point_index: 2,
            overall_rank: 3,
            personal_rank: 1,
            personal_best_duration_seconds: 312,
          },
        ],
      }),
      isLoading: false,
      isError: false,
      error: null,
    });

    render(<ActivityDetailPanel activityId={7} />);

    fireEvent.click(screen.getByRole("button", { name: "Show time & runs" }));

    expect(screen.getByText("Top 3")).toBeInTheDocument();
    expect(screen.queryByText("PR")).not.toBeInTheDocument();

    fireEvent.mouseEnter(screen.getByLabelText("North Climb run 1 point"));

    expect(screen.getAllByText("Top 3").length).toBeGreaterThan(0);
    expect(screen.queryByText("PR")).not.toBeInTheDocument();
    expect(screen.queryByText("Fastest run today")).not.toBeInTheDocument();
  });

  it("lets the user toggle the merged signal layers", async () => {
    const user = userEvent.setup();

    render(<ActivityDetailPanel activityId={7} />);

    const heartRateButton = screen.getByRole("button", { name: "Heart rate" });
    const powerButton = screen.getByRole("button", { name: "Power" });
    const speedButton = screen.getByRole("button", { name: "Speed" });
    const elevationButton = screen.getByRole("button", { name: "Elevation" });

    expect(heartRateButton).toHaveTextContent("Peak 168 bpm");
    expect(powerButton).toHaveTextContent("Peak 314 W");
    expect(speedButton).toHaveTextContent("Top speed");

    await user.click(heartRateButton);

    expect(heartRateButton).toHaveAttribute("aria-pressed", "false");
    expect(heartRateButton).toHaveTextContent("Peak 168 bpm");

    await user.click(elevationButton);
    expect(
      screen.queryByRole("img", { name: "Activity signals chart" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByText(
        "Turn on at least one signal layer to render the chart.",
      ),
    ).toBeInTheDocument();

    await user.click(powerButton);

    expect(powerButton).toHaveAttribute("aria-pressed", "true");
    expect(
      screen.getByRole("img", { name: "Activity signals chart" }),
    ).toBeInTheDocument();

    await user.click(speedButton);

    expect(speedButton).toHaveAttribute("aria-pressed", "true");
  });

  it("hides the signal card when there are no usable signal series", () => {
    const { container, rerender } = render(
      <ActivitySignalsCard chartPoints={[]} unitSystem="metric" />,
    );
    expect(container).toBeEmptyDOMElement();
    rerender(
      <ActivitySignalsCard
        chartPoints={makeActivity().chart_points.slice(0, 1)}
        unitSystem="metric"
      />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("orders matched segments alphabetically and expands a card on demand", async () => {
    const user = userEvent.setup();

    mocks.useActivity.mockReturnValue({
      data: makeActivity({
        segment_efforts: [
          {
            segment_id: 22,
            segment_title: "Zulu Ridge",
            effort_index: 1,
            duration_seconds: 410,
            start_route_point_index: 2,
            end_route_point_index: 3,
            overall_rank: 7,
            personal_rank: 2,
            personal_best_duration_seconds: 390,
          },
          {
            segment_id: 11,
            segment_title: "Alpha Climb",
            effort_index: 1,
            duration_seconds: 312,
            start_route_point_index: 1,
            end_route_point_index: 2,
            overall_rank: 1,
            personal_rank: 1,
            personal_best_duration_seconds: 312,
          },
        ],
      }),
      isLoading: false,
      isError: false,
      error: null,
    });

    render(<ActivityDetailPanel activityId={7} />);

    const segmentLinks = screen
      .getAllByRole("link")
      .filter((link) =>
        ["Alpha Climb", "Zulu Ridge"].includes(link.textContent ?? ""),
      );

    expect(segmentLinks.map((link) => link.textContent)).toEqual([
      "Alpha Climb",
      "Zulu Ridge",
    ]);

    await user.click(
      screen.getAllByRole("button", { name: "Show time & runs" })[0],
    );

    expect(
      screen.getByRole("img", { name: "Alpha Climb attempts chart" }),
    ).toBeInTheDocument();
  });

  it("starts starred segments expanded and allows collapsing without removing the star", async () => {
    const user = userEvent.setup();
    mocks.useSegments.mockReturnValue({
      data: [{ id: 11, starred: true }],
      isLoading: false,
      isError: false,
      error: null,
    });

    render(<ActivityDetailPanel activityId={7} />);

    expect(
      screen.getByRole("img", { name: "North Climb attempts chart" }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Hide time & runs" }));
    expect(
      screen.queryByRole("img", { name: "North Climb attempts chart" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Unstar North Climb" }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(
      screen.getByRole("button", { name: "Highlight North Climb" }),
    ).toHaveAttribute("aria-pressed", "false");
    await user.click(screen.getByRole("button", { name: "Show time & runs" }));
    expect(
      screen.getByRole("img", { name: "North Climb attempts chart" }),
    ).toBeInTheDocument();
  });

  it("opens and dismisses the activity editor from the detail view", async () => {
    const user = userEvent.setup();
    render(<ActivityDetailPanel activityId={7} />);

    await user.click(screen.getByRole("button", { name: "Edit activity" }));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("deletes the activity after confirmation", async () => {
    const user = userEvent.setup();
    const confirmSpy = vi.fn(() => true);
    const deleteAsync = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal("confirm", confirmSpy);
    mocks.useDeleteActivity.mockReturnValue({
      deleteAsync,
      isPending: false,
      isError: false,
      error: null,
    });

    render(<ActivityDetailPanel activityId={7} />);

    await user.click(screen.getByRole("button", { name: "Delete activity" }));

    expect(confirmSpy).toHaveBeenCalledWith(
      "Delete this activity? This removes the activity and clears any derived segment matches.",
    );
    expect(deleteAsync).toHaveBeenCalledWith(7);
    expect(mocks.routerPush).toHaveBeenCalledWith("/");
  });

  it("renders the route map empty state when route points are missing", () => {
    mocks.useActivity.mockReturnValue({
      data: makeActivity({ route_points: [], segment_efforts: [] }),
      isLoading: false,
      isError: false,
      error: null,
    });

    render(<ActivityDetailPanel activityId={7} />);

    expect(
      screen.getByText(
        /does not have enough stored route points for the map yet/i,
      ),
    ).toBeInTheDocument();
  });
});
