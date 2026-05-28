import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import ActivityDetailPanel from "../ActivityDetailPanel";

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
  useRegenerateActivity: vi.fn(),
  useDeleteActivity: vi.fn(),
  useSegments: vi.fn(),
  useUpdateSegment: vi.fn(),
  updateSegmentAsync: vi.fn(),
  renderMapLibreRouteMap: vi.fn(),
  routerPush: vi.fn(),
}));

vi.mock("@ericbutera/kaleido", () => ({
  auth: {
    useAuthApi: () => ({
      useCurrentUser: mocks.useCurrentUser,
    }),
  },
}));

vi.mock("../../lib/queries", () => ({
  useActivity: mocks.useActivity,
  useRegenerateActivity: mocks.useRegenerateActivity,
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
    id: number;
    title: string;
    sport: string;
    source: string;
    original_filename: string | null;
    format: string | null;
    started_at: string;
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
  }> = {},
) {
  return {
    id: 7,
    title: "Lunch Ride",
    sport: "ride",
    source: "manual_upload",
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
    laps: [
      {
        lap_index: 1,
        title: "Lap 1",
        duration_seconds: 1600,
        distance_meters: 14000,
        average_speed_mps: 8.7,
        average_heart_rate_bpm: 142,
        max_heart_rate_bpm: 162,
      },
    ],
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
    can_regenerate: true,
    ...overrides,
  };
}

describe("ActivityDetailPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
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
    mocks.useRegenerateActivity.mockReturnValue({
      regenerateAsync: vi.fn().mockResolvedValue(makeActivity()),
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
    vi.unstubAllGlobals();
  });

  it("renders a sign-in prompt when the user is signed out", () => {
    mocks.useCurrentUser.mockReturnValue({ user: null, isLoading: false });

    render(<ActivityDetailPanel activityId={7} />);

    expect(
      screen.getByText("Sign in to view activity details"),
    ).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Sign in" })).toHaveAttribute(
      "href",
      "/login",
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
    expect(screen.getByText("28.0 km")).toBeInTheDocument();
    expect(screen.getByText("53m 20s")).toBeInTheDocument();
    expect(screen.getAllByText("19.5 mph").length).toBeGreaterThan(0);
    expect(screen.getAllByText("168 bpm").length).toBeGreaterThan(0);
    expect(screen.getByText("lunch-ride.tcx")).toBeInTheDocument();
    expect(screen.getByText("Above 170 bpm")).toBeInTheDocument();
    expect(
      screen.getByRole("img", { name: "Activity route map" }),
    ).toBeInTheDocument();
    expect(mocks.renderMapLibreRouteMap).toHaveBeenCalledWith(
      expect.objectContaining({
        ariaLabel: "Activity route map",
        showZoomControls: true,
        showLayerPicker: true,
        defaultBasemap: "topo",
      }),
    );
    expect(screen.getByText("Matched segments")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Jump to North Climb matches" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Show time & runs" }),
    ).toHaveAttribute("aria-expanded", "false");
    expect(
      screen.queryByRole("img", { name: "North Climb attempts chart" }),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("Time")).not.toBeInTheDocument();
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
      screen.getByRole("button", { name: "Regenerate derived data" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Delete activity" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Lap splits")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Lap 1" })).toBeInTheDocument();
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
  });

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

    await user.click(heartRateButton);

    expect(heartRateButton).toHaveAttribute("aria-pressed", "false");

    await user.click(powerButton);

    expect(powerButton).toHaveAttribute("aria-pressed", "true");

    await user.click(speedButton);

    expect(speedButton).toHaveAttribute("aria-pressed", "true");
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

  it("keeps starred matched segments open", () => {
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
    expect(
      screen.getByRole("button", { name: "Starred stays open" }),
    ).toBeDisabled();
  });

  it("shows the regenerate action", async () => {
    const user = userEvent.setup();
    const regenerateAsync = vi.fn().mockResolvedValue(makeActivity());
    mocks.useRegenerateActivity.mockReturnValue({
      regenerateAsync,
      isPending: false,
      isError: false,
      error: null,
    });

    render(<ActivityDetailPanel activityId={7} />);

    await user.click(
      screen.getByRole("button", { name: "Regenerate derived data" }),
    );

    expect(regenerateAsync).toHaveBeenCalledWith(7);
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

  it("renders the route map empty state with a regenerate action when route points are missing", () => {
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
