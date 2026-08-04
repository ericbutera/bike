import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { type ComponentProps } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import RequireAuth from "../RequireAuth";
import SegmentDetailPanel from "../SegmentDetailPanel";
import { ComparisonGapChartTooltip } from "../segment-detail/SegmentDetailComparisonSection";

vi.mock("recharts", async (importOriginal) => {
  const actual = await importOriginal<typeof import("recharts")>();
  const React = await import("react");

  return {
    ...actual,
    ResponsiveContainer: ({ children }: { children: React.ReactNode }) => {
      if (React.isValidElement<{ width?: number; height?: number }>(children)) {
        return React.cloneElement(children, {
          width: 640,
          height: 240,
        });
      }

      return React.createElement(
        "div",
        { style: { width: 640, height: 240 } },
        children,
      );
    },
  };
});

const mocks = vi.hoisted(() => ({
  useCurrentUser: vi.fn(),
  useSegment: vi.fn(),
  useSegmentComparison: vi.fn(),
  useUpdateSegment: vi.fn(),
  useDeleteSegment: vi.fn(),
  renderMapLibreRouteMap: vi.fn(),
  routerPush: vi.fn(),
}));

vi.mock("@ericbutera/kaleido", async () => {
  const React = await import("react");

  return {
    auth: {
      useAuthApi: () => ({
        useCurrentUser: mocks.useCurrentUser,
      }),
    },
    Pagination: ({
      page,
      perPage,
      total,
      onPageChange,
    }: {
      page: number;
      perPage: number;
      total: number;
      onPageChange: (page: number) => void;
    }) => {
      const totalPages = Math.max(1, Math.ceil(total / perPage));

      if (totalPages <= 1) {
        return null;
      }

      return (
        <div>
          {Array.from({ length: totalPages }, (_, index) => index + 1).map(
            (pageNumber) => (
              <button
                key={pageNumber}
                type="button"
                aria-current={pageNumber === page ? "page" : undefined}
                onClick={() => {
                  onPageChange(pageNumber);
                }}
              >
                {pageNumber}
              </button>
            ),
          )}
        </div>
      );
    },
  };
});

vi.mock("../../lib/queries", () => ({
  useSegment: mocks.useSegment,
  useSegmentComparison: mocks.useSegmentComparison,
  useUpdateSegment: mocks.useUpdateSegment,
  useDeleteSegment: mocks.useDeleteSegment,
}));

vi.mock("next/navigation", () => ({
  useRouter: () => ({
    push: mocks.routerPush,
  }),
}));

vi.mock("../MapLibreRouteMap", () => ({
  default: (props: any) => {
    mocks.renderMapLibreRouteMap(props);

    return <div role="img" aria-label={props.ariaLabel} />;
  },
}));

vi.mock("next/link", () => ({
  default: ({ href, children, ...props }: any) => (
    <a href={href} {...props}>
      {children}
    </a>
  ),
}));

function makeRoutePoint(
  elapsed_seconds: number,
  latitude: number,
  longitude: number,
  overrides: Partial<{
    distance_meters: number | null;
    elevation_meters: number | null;
    speed_mps: number | null;
    heart_rate_bpm: number | null;
    cadence_rpm: number | null;
  }> = {},
) {
  return {
    elapsed_seconds,
    latitude,
    longitude,
    distance_meters: elapsed_seconds * 10,
    elevation_meters: 120 + elapsed_seconds,
    speed_mps: 8 + elapsed_seconds / 20,
    heart_rate_bpm: 140 + elapsed_seconds,
    cadence_rpm: 82,
    ...overrides,
  };
}

function makeSegment() {
  return {
    id: 14,
    title: "North Climb",
    source: "manual_upload",
    mode: "xc",
    original_filename: "north-climb.gpx",
    format: "gpx",
    distance_meters: 1800,
    effort_count: 2,
    best_duration_seconds: 312,
    current_user_pr_duration_seconds: 312,
    created_at: "2026-05-07T07:00:00Z",
    route_points: [
      makeRoutePoint(0, 45.0, -122.0),
      makeRoutePoint(120, 45.004, -121.996),
      makeRoutePoint(240, 45.008, -121.992),
      makeRoutePoint(312, 45.012, -121.988),
    ],
    efforts: [
      {
        id: 1,
        rider_user_id: 1,
        activity_id: 7,
        activity_title: "Lunch Ride",
        rider_name: "Eric Butera",
        activity_started_at: "2026-05-06T12:00:00Z",
        effort_index: 1,
        duration_seconds: 312,
        start_elapsed_seconds: 400,
        end_elapsed_seconds: 712,
        distance_meters: 1800,
        route_points: [
          makeRoutePoint(0, 45.0, -122.0),
          makeRoutePoint(120, 45.004, -121.996),
          makeRoutePoint(240, 45.008, -121.992),
          makeRoutePoint(312, 45.012, -121.988),
        ],
      },
      {
        id: 2,
        rider_user_id: 2,
        activity_id: 8,
        activity_title: "Hill Attack",
        rider_name: "Casey Fast",
        activity_started_at: "2026-05-08T08:00:00Z",
        effort_index: 2,
        duration_seconds: 300,
        start_elapsed_seconds: 500,
        end_elapsed_seconds: 800,
        distance_meters: 1800,
        route_points: [
          makeRoutePoint(0, 45.0, -122.0, { speed_mps: 7.6 }),
          makeRoutePoint(120, 45.004, -121.996, { speed_mps: 8.1 }),
          makeRoutePoint(240, 45.008, -121.992, { speed_mps: 8.7 }),
          makeRoutePoint(300, 45.012, -121.988, { speed_mps: 9.0 }),
        ],
      },
    ],
  };
}

function makeManyEfforts(count: number) {
  return Array.from({ length: count }, (_, index) => ({
    id: index + 1,
    rider_user_id: (index % 4) + 1,
    activity_id: 100 + index,
    activity_title: `Ride ${index + 1}`,
    rider_name: `Rider ${index + 1}`,
    activity_started_at: `2026-05-${String((index % 28) + 1).padStart(2, "0")}T08:00:00Z`,
    effort_index: index + 1,
    duration_seconds: 280 + index,
    start_elapsed_seconds: 300 + index,
    end_elapsed_seconds: 580 + index,
    distance_meters: 1800,
    route_points: [
      makeRoutePoint(0, 45.0, -122.0),
      makeRoutePoint(120, 45.004, -121.996),
      makeRoutePoint(240, 45.008, -121.992),
      makeRoutePoint(280 + index, 45.012, -121.988),
    ],
  }));
}

describe("SegmentDetailPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(Date, "now").mockReturnValue(
      new Date("2026-05-08T12:00:00Z").getTime(),
    );
    mocks.useCurrentUser.mockReturnValue({
      user: { id: 1, name: "Eric Butera", email: "rider@example.com" },
      isLoading: false,
    });
    mocks.useSegment.mockReturnValue({
      data: makeSegment(),
      isLoading: false,
      isError: false,
      error: null,
    });
    mocks.useSegmentComparison.mockReturnValue({
      data: null,
      isLoading: false,
      isError: false,
      error: null,
    });
    mocks.useUpdateSegment.mockReturnValue({
      updateAsync: vi.fn(),
      isPending: false,
      isError: false,
      error: null,
    });
    mocks.useDeleteSegment.mockReturnValue({
      deleteAsync: vi.fn(),
      isPending: false,
      isError: false,
      error: null,
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  function renderSegmentDetailPanel(
    props: Partial<ComponentProps<typeof SegmentDetailPanel>> = {},
  ) {
    return render(
      <RequireAuth>
        <SegmentDetailPanel segmentId={14} {...props} />
      </RequireAuth>,
    );
  }

  it("renders sign-in actions when the user is signed out", () => {
    mocks.useCurrentUser.mockReturnValue({ user: null, isLoading: false });

    renderSegmentDetailPanel();

    expect(screen.getByText("Sign in required")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Sign in" })).toHaveAttribute(
      "href",
      "/login",
    );
  });

  it("renders the comparison UI for a segment", () => {
    renderSegmentDetailPanel();

    const hillAttackRow = screen
      .getByRole("link", { name: "5m 00s" })
      .closest("tr");
    const lunchRideRow = screen
      .getByRole("link", { name: "5m 12s" })
      .closest("tr");

    if (!hillAttackRow) {
      throw new Error("Hill Attack row not found");
    }

    if (!lunchRideRow) {
      throw new Error("Lunch Ride row not found");
    }

    const topTwoBadge = within(lunchRideRow).getByText("Top 2");

    expect(
      screen.getByRole("heading", { name: "North Climb" }),
    ).toBeInTheDocument();
    expect(screen.getByText("FMR / Effort Comparison")).toBeInTheDocument();
    expect(
      screen.queryByRole("searchbox", { name: "Search efforts" }),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("2 rides selected")).not.toBeInTheDocument();
    expect(screen.getByText("Comparison workspace")).toBeInTheDocument();
    expect(screen.getByText("Athletes")).toBeInTheDocument();
    expect(screen.getAllByText("Speed").length).toBeGreaterThan(0);
    expect(screen.getByText("HR")).toBeInTheDocument();
    expect(screen.getByText("KOM 5m 00s")).toBeInTheDocument();
    expect(screen.getByText("Your PR 5m 12s")).toBeInTheDocument();
    expect(screen.getAllByText("Lunch Ride").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Eric Butera").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Casey Fast").length).toBeGreaterThan(0);
    expect(
      screen.queryByText(
        /The selected rides drive both the route playback and the shared chart/i,
      ),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText(
        /Play the selected attempts against the same route to see where each ride is gaining or losing time/i,
      ),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText(
        /Compare the selected attempts across elapsed time while the map dots advance/i,
      ),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("Chart comparison")).not.toBeInTheDocument();
    expect(
      screen.getByRole("img", { name: "Segment comparison map" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("img", { name: "Segment comparison chart" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Slow" })).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Detail" }),
    ).not.toBeInTheDocument();
    expect(screen.queryByText(/\btarget\b/i)).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: "5m 12s" })).toHaveAttribute(
      "href",
      "/activities/7",
    );
    expect(screen.getByRole("link", { name: "5m 00s" })).toHaveAttribute(
      "href",
      "/activities/8",
    );
    expect(
      within(hillAttackRow).getByRole("button", {
        name: "Remove Hill Attack from comparison",
      }),
    ).toBeInTheDocument();
    expect(screen.getByText("KOM")).toBeInTheDocument();
    expect(screen.getByText("Top 2")).toBeInTheDocument();
    expect(topTwoBadge).toHaveClass("badge-warning");
    expect(lunchRideRow).not.toHaveClass("bg-info/10");
    expect(screen.queryByText("PR")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Heart rate" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText(
        "Click a ride to make it the reference line while playback runs.",
      ),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("Hover point")).not.toBeInTheDocument();
    expect(screen.queryByText("Back to home")).not.toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Segment mode" })).toHaveValue(
      "xc",
    );
    expect(
      screen.getByRole("button", { name: "Open segment actions" }),
    ).toBeInTheDocument();
  });

  it("shows same-day run numbers in the efforts table", () => {
    const segment = makeSegment();
    const fasterHistoricalEfforts = Array.from({ length: 10 }, (_, index) => ({
      id: index + 1,
      rider_user_id: 3,
      activity_id: 100 + index,
      activity_title: `Earlier Fast Ride ${index + 1}`,
      rider_name: "Riley Quick",
      activity_started_at: `2026-05-${String(index + 1).padStart(2, "0")}T08:00:00Z`,
      effort_index: 1,
      duration_seconds: 300 + index,
      start_elapsed_seconds: 100,
      end_elapsed_seconds: 400 + index,
      distance_meters: 1800,
      route_points: [
        makeRoutePoint(0, 45.0, -122.0),
        makeRoutePoint(300 + index, 45.012, -121.988),
      ],
    }));

    segment.efforts = [
      ...fasterHistoricalEfforts,
      {
        id: 101,
        rider_user_id: 2,
        activity_id: 210,
        activity_title: "Evening Repeats",
        rider_name: "Casey Fast",
        activity_started_at: "2026-05-08T22:00:00Z",
        effort_index: 1,
        duration_seconds: 420,
        start_elapsed_seconds: 300,
        end_elapsed_seconds: 720,
        distance_meters: 1800,
        route_points: [
          makeRoutePoint(0, 45.0, -122.0),
          makeRoutePoint(420, 45.012, -121.988),
        ],
      },
      {
        id: 102,
        rider_user_id: 2,
        activity_id: 210,
        activity_title: "Evening Repeats",
        rider_name: "Casey Fast",
        activity_started_at: "2026-05-08T22:00:00Z",
        effort_index: 2,
        duration_seconds: 410,
        start_elapsed_seconds: 900,
        end_elapsed_seconds: 1310,
        distance_meters: 1800,
        route_points: [
          makeRoutePoint(0, 45.0, -122.0),
          makeRoutePoint(410, 45.012, -121.988),
        ],
      },
      {
        id: 103,
        rider_user_id: 2,
        activity_id: 210,
        activity_title: "Evening Repeats",
        rider_name: "Casey Fast",
        activity_started_at: "2026-05-08T22:00:00Z",
        effort_index: 3,
        duration_seconds: 430,
        start_elapsed_seconds: 1500,
        end_elapsed_seconds: 1930,
        distance_meters: 1800,
        route_points: [
          makeRoutePoint(0, 45.0, -122.0),
          makeRoutePoint(430, 45.012, -121.988),
        ],
      },
    ];
    segment.effort_count = segment.efforts.length;
    segment.best_duration_seconds = 300;

    mocks.useSegment.mockReturnValue({
      data: segment,
      isLoading: false,
      isError: false,
      error: null,
    });

    renderSegmentDetailPanel();

    const table = screen.getByLabelText("Segment efforts table");
    const runOneRow = within(table)
      .getByRole("link", { name: "7m 00s" })
      .closest("tr");
    const fastestRunRow = within(table)
      .getByRole("link", { name: "6m 50s" })
      .closest("tr");
    const runThreeRow = within(table)
      .getByRole("link", { name: "7m 10s" })
      .closest("tr");

    expect(
      within(table).getByRole("columnheader", { name: "Run" }),
    ).toBeInTheDocument();
    expect(runOneRow).not.toBeNull();
    expect(fastestRunRow).not.toBeNull();
    expect(runThreeRow).not.toBeNull();
    expect(
      within(runOneRow as HTMLElement).getByText("Run 1"),
    ).toBeInTheDocument();
    expect(
      within(fastestRunRow as HTMLElement).getByText("Run 2"),
    ).toBeInTheDocument();
    expect(
      within(fastestRunRow as HTMLElement).queryByText("Fastest"),
    ).not.toBeInTheDocument();
    expect(fastestRunRow).not.toHaveClass("bg-success/10");
    expect(
      within(runThreeRow as HTMLElement).getByText("Run 3"),
    ).toBeInTheDocument();
  });

  it("paginates efforts 25 per page", async () => {
    const user = userEvent.setup();
    const segment = makeSegment();

    segment.efforts = makeManyEfforts(30);
    segment.effort_count = 30;
    segment.best_duration_seconds = 280;
    segment.current_user_pr_duration_seconds = 280;

    mocks.useSegment.mockReturnValue({
      data: segment,
      isLoading: false,
      isError: false,
      error: null,
    });

    renderSegmentDetailPanel();

    const table = screen.getByLabelText("Segment efforts table");

    expect(within(table).getAllByRole("row")).toHaveLength(26);
    expect(screen.getByText("Showing 1-25 of 30 efforts")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "2" }));

    expect(within(table).getAllByRole("row")).toHaveLength(6);
    expect(screen.getByText("Showing 26-30 of 30 efforts")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /5m 09s/i })).toBeInTheDocument();
  });

  it("updates the segment mode", async () => {
    const user = userEvent.setup();
    const updateAsync = vi.fn().mockResolvedValue({
      ...makeSegment(),
      mode: "dh",
    });

    mocks.useUpdateSegment.mockReturnValue({
      updateAsync,
      isPending: false,
      isError: false,
      error: null,
    });

    renderSegmentDetailPanel();

    await user.selectOptions(
      screen.getByRole("combobox", { name: "Segment mode" }),
      "dh",
    );

    expect(updateAsync).toHaveBeenCalledWith({
      id: 14,
      mode: "dh",
    });
  });

  it("updates live comparison metrics when playback moves", async () => {
    renderSegmentDetailPanel();

    fireEvent.change(
      screen.getByRole("slider", { name: "Playback timeline" }),
      {
        target: { value: "120" },
      },
    );

    expect(screen.getByText("2m 00s")).toBeInTheDocument();
    expect(screen.queryByText("elapsed")).not.toBeInTheDocument();
    expect(screen.queryByText("vs ref")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Slow" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Auto" })).toHaveClass(
      "btn-neutral",
    );
    expect(screen.queryByText(/\btarget\b/i)).not.toBeInTheDocument();
  });

  it("hides the playback target helper badge for long reference rides", () => {
    const segment = makeSegment();

    segment.efforts = [
      {
        ...segment.efforts[0],
        duration_seconds: 3600,
        route_points: [
          makeRoutePoint(0, 45.0, -122.0),
          makeRoutePoint(1800, 45.006, -121.994),
          makeRoutePoint(3600, 45.012, -121.988),
        ],
      },
      segment.efforts[1],
    ];

    mocks.useSegment.mockReturnValue({
      data: segment,
      isLoading: false,
      isError: false,
      error: null,
    });

    renderSegmentDetailPanel();

    expect(screen.queryByText(/\btarget\b/i)).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Auto" })).toHaveClass(
      "btn-neutral",
    );
  });

  it("keeps non-reference rides from showing live metrics after they finish", () => {
    const segment = makeSegment();

    segment.efforts = [
      {
        ...segment.efforts[0],
        duration_seconds: 110,
        route_points: [
          makeRoutePoint(0, 45.0, -122.0),
          makeRoutePoint(55, 45.006, -121.994),
          makeRoutePoint(110, 45.012, -121.988),
        ],
      },
      {
        ...segment.efforts[1],
        duration_seconds: 25,
        route_points: [
          makeRoutePoint(0, 45.0, -122.0, { speed_mps: 10.2 }),
          makeRoutePoint(12.5, 45.006, -121.994, { speed_mps: 10.8 }),
          makeRoutePoint(25, 45.012, -121.988, { speed_mps: 11.1 }),
        ],
      },
    ];

    mocks.useSegment.mockReturnValue({
      data: segment,
      isLoading: false,
      isError: false,
      error: null,
    });

    renderSegmentDetailPanel();

    fireEvent.change(
      screen.getByRole("slider", { name: "Playback timeline" }),
      {
        target: { value: "50" },
      },
    );

    const hillAttackReferenceButton = screen.getByRole("button", {
      name: "Make Hill Attack the reference ride",
    });
    const hillAttackRow = hillAttackReferenceButton.closest("div.grid");

    expect(hillAttackRow).not.toBeNull();
    expect(
      within(hillAttackRow as HTMLElement).getAllByText("--"),
    ).toHaveLength(2);
  });

  it("sorts the athlete pane by the current leader as playback advances", () => {
    renderSegmentDetailPanel();

    fireEvent.change(
      screen.getByRole("slider", { name: "Playback timeline" }),
      {
        target: { value: "120" },
      },
    );

    const referenceButtons = screen.getAllByRole("button", {
      name: /Make .* the reference ride/,
    });

    expect(referenceButtons[0]).toHaveAttribute(
      "aria-label",
      "Make Hill Attack the reference ride",
    );
    expect(referenceButtons[1]).toHaveAttribute(
      "aria-label",
      "Make Lunch Ride the reference ride",
    );
  });

  it("shows only KOM when the same effort is also the current user PR", () => {
    const segment = makeSegment();

    segment.efforts = [
      {
        ...segment.efforts[0],
        duration_seconds: 300,
        end_elapsed_seconds: 700,
        route_points: [
          makeRoutePoint(0, 45.0, -122.0),
          makeRoutePoint(120, 45.004, -121.996),
          makeRoutePoint(240, 45.008, -121.992),
          makeRoutePoint(300, 45.012, -121.988),
        ],
      },
      {
        ...segment.efforts[1],
        duration_seconds: 330,
        end_elapsed_seconds: 830,
        route_points: [
          makeRoutePoint(0, 45.0, -122.0),
          makeRoutePoint(120, 45.004, -121.996),
          makeRoutePoint(240, 45.008, -121.992),
          makeRoutePoint(330, 45.012, -121.988),
        ],
      },
    ];

    mocks.useSegment.mockReturnValue({
      data: segment,
      isLoading: false,
      isError: false,
      error: null,
    });

    renderSegmentDetailPanel();

    expect(screen.getByText("KOM")).toBeInTheDocument();
    expect(screen.queryByText("PR")).not.toBeInTheDocument();
  });

  it("uses the segment PR duration when the current user effort is not matched locally", () => {
    const segment = makeSegment();

    segment.efforts = segment.efforts.map((effort) => ({
      ...effort,
      rider_user_id: effort.rider_user_id + 50,
    }));

    mocks.useSegment.mockReturnValue({
      data: segment,
      isLoading: false,
      isError: false,
      error: null,
    });

    renderSegmentDetailPanel();

    expect(screen.getByText("Your PR 5m 12s")).toBeInTheDocument();
    expect(
      screen.getByText("Personal best across matched efforts"),
    ).toBeInTheDocument();
  });

  it("projects playback markers onto the segment route geometry", () => {
    const segment = makeSegment();

    segment.efforts = [
      {
        ...segment.efforts[0],
        route_points: [
          makeRoutePoint(0, 46.0, -123.0, { distance_meters: 0 }),
          makeRoutePoint(120, 46.5, -123.5, { distance_meters: 1200 }),
          makeRoutePoint(240, 47.0, -124.0, { distance_meters: 2400 }),
          makeRoutePoint(312, 47.5, -124.5, { distance_meters: 3120 }),
        ],
      },
    ];

    mocks.useSegment.mockReturnValue({
      data: segment,
      isLoading: false,
      isError: false,
      error: null,
    });

    renderSegmentDetailPanel();

    fireEvent.change(
      screen.getByRole("slider", { name: "Playback timeline" }),
      {
        target: { value: "120" },
      },
    );

    const props = mocks.renderMapLibreRouteMap.mock.lastCall?.[0];

    expect(props?.movingMarkers).toHaveLength(1);
    expect(props?.movingMarkers[0].label).toBe("1");
    expect(props?.movingMarkers[0].point.latitude).toBeCloseTo(45.004, 3);
    expect(props?.movingMarkers[0].point.longitude).toBeCloseTo(-121.996, 3);
    expect(props?.fitBoundsPadding).toBe(40);
    expect(props?.fitBoundsMaxZoom).toBe(18);
  });

  it("keeps the embedded map in overview mode and links to the race viewer", () => {
    renderSegmentDetailPanel();

    expect(
      screen.queryByRole("button", { name: "Overview" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Leader follow" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("link", { name: "Open race viewer" }),
    ).toHaveAttribute("href", "/segments/14/race?efforts=1%2C2&ref=1");
    expect(
      mocks.renderMapLibreRouteMap.mock.lastCall?.[0]?.followViewport,
    ).toBeUndefined();
  });

  it("uses initially requested efforts when returning from the race viewer", () => {
    renderSegmentDetailPanel({
      initialSelectedEffortIds: [2],
      initialReferenceEffortId: 2,
    });

    expect(
      screen.getAllByRole("button", {
        name: "Remove Hill Attack from comparison",
      }),
    ).toHaveLength(2);
    expect(
      screen.getByRole("button", {
        name: "Add Lunch Ride to comparison",
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("link", { name: "Open race viewer" }),
    ).toHaveAttribute("href", "/segments/14/race?efforts=2&ref=2");
  });

  it("hides timer and pace controls on very narrow screens", () => {
    renderSegmentDetailPanel();

    const timeline = screen.getByRole("slider", { name: "Playback timeline" });
    const timerChip =
      timeline.parentElement?.querySelector("span.rounded-full");
    const paceControls = screen.getByRole("button", {
      name: "Slow",
    }).parentElement;

    expect(timerChip).toHaveClass("max-[420px]:hidden");
    expect(paceControls).toHaveClass("max-[420px]:hidden");
  });

  it("does not dim other comparison rows when a reference ride is selected", async () => {
    const user = userEvent.setup();

    renderSegmentDetailPanel();

    await user.click(
      screen.getByRole("button", {
        name: "Make Hill Attack the reference ride",
      }),
    );

    expect(document.querySelector(".opacity-40")).toBeNull();
  });

  it("renders the athlete pane without drag and drop affordances", () => {
    renderSegmentDetailPanel();

    expect(document.querySelector("[draggable='true']")).toBeNull();
    expect(
      screen.queryByTitle("Drag to reorder selected efforts"),
    ).not.toBeInTheDocument();
  });

  it("filters efforts by the selected time window", async () => {
    const user = userEvent.setup();

    renderSegmentDetailPanel();

    await user.click(screen.getByRole("button", { name: "Day" }));

    expect(screen.getByText("Showing 1-1 of 1 efforts")).toBeInTheDocument();
    expect(
      screen.queryByRole("link", { name: "5m 12s" }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: "5m 00s" })).toHaveAttribute(
      "href",
      "/activities/8",
    );
  });

  it("keeps compared efforts selected across time filters until removed", async () => {
    const user = userEvent.setup();

    renderSegmentDetailPanel();

    expect(screen.queryByText(/^\d+ selected$/)).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Day" }));

    expect(screen.getByText("Showing 1-1 of 1 efforts")).toBeInTheDocument();
    expect(
      screen.getByRole("button", {
        name: "Remove Lunch Ride from comparison",
      }),
    ).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", {
        name: "Remove Lunch Ride from comparison",
      }),
    );

    expect(
      screen.queryAllByRole("button", {
        name: "Remove Lunch Ride from comparison",
      }),
    ).toHaveLength(0);
    expect(
      screen.getAllByRole("button", {
        name: "Remove Hill Attack from comparison",
      }).length,
    ).toBeGreaterThan(0);
  });

  it("renders the comparison tooltip in a condensed single-line-per-ride format", () => {
    const selectedRows = [
      {
        color: "#2563eb",
        markerLabel: "1",
        effort: {
          id: 101,
          rider_user_id: 1,
          activity_id: 71,
          activity_title: "Lunch Ride",
          rider_name: "Eric Butera",
          activity_started_at: "2026-05-06T12:00:00Z",
          effort_index: 1,
          duration_seconds: 120,
          start_elapsed_seconds: 0,
          end_elapsed_seconds: 120,
          distance_meters: 1000,
          route_points: [
            makeRoutePoint(0, 45.0, -122.0, {
              distance_meters: 0,
              elevation_meters: 10,
              speed_mps: 5,
              heart_rate_bpm: 100,
            }),
            makeRoutePoint(120, 45.01, -121.99, {
              distance_meters: 1000,
              elevation_meters: 30,
              speed_mps: 6,
              heart_rate_bpm: 120,
            }),
          ],
        },
      },
      {
        color: "#dc2626",
        markerLabel: "2",
        effort: {
          id: 102,
          rider_user_id: 2,
          activity_id: 72,
          activity_title: "Hill Attack",
          rider_name: "Casey Fast",
          activity_started_at: "2026-05-08T08:00:00Z",
          effort_index: 1,
          duration_seconds: 110,
          start_elapsed_seconds: 0,
          end_elapsed_seconds: 110,
          distance_meters: 1000,
          route_points: [
            makeRoutePoint(0, 45.0, -122.0, {
              distance_meters: 0,
              elevation_meters: 12,
              speed_mps: 5.4,
              heart_rate_bpm: 104,
            }),
            makeRoutePoint(110, 45.01, -121.99, {
              distance_meters: 1000,
              elevation_meters: 28,
              speed_mps: 6.2,
              heart_rate_bpm: 132,
            }),
          ],
        },
      },
    ];
    const tooltipRow = {
      progress: 0.5,
      distanceMeters: 500,
      elevation: 20,
    };

    render(
      <ComparisonGapChartTooltip
        active
        label={500}
        payload={[
          {
            dataKey: "elevation",
            value: 20,
            payload: tooltipRow,
          },
          {
            dataKey: "effort_101",
            value: 0,
            payload: tooltipRow,
          },
          {
            dataKey: "effort_102",
            value: 5,
            payload: tooltipRow,
          },
        ]}
        referenceEffort={selectedRows[0].effort}
        selectedRows={selectedRows}
        unitSystem="metric"
      />,
    );

    expect(screen.getByText("Time 1m 00s")).toBeInTheDocument();
    expect(screen.getByText("Elev 20 m")).toBeInTheDocument();
    expect(screen.getByText("Dist 0.5 km")).toBeInTheDocument();

    const rideOneRow = screen.getByLabelText("Ride 1 tooltip row");
    expect(within(rideOneRow).getByText("#1")).toBeInTheDocument();
    expect(within(rideOneRow).getByText("1m 00s")).toBeInTheDocument();
    expect(within(rideOneRow).getByText("19.8 km/h")).toBeInTheDocument();
    expect(within(rideOneRow).getByText("110 bpm")).toBeInTheDocument();

    const rideTwoRow = screen.getByLabelText("Ride 2 tooltip row");
    expect(within(rideTwoRow).getByText("#2")).toBeInTheDocument();
    expect(within(rideTwoRow).getByText("55s")).toBeInTheDocument();
    expect(within(rideTwoRow).getByText("20.9 km/h")).toBeInTheDocument();
    expect(within(rideTwoRow).getByText("118 bpm")).toBeInTheDocument();
  });

  it("truncates tooltip times to three decimal places", () => {
    const selectedRows = [
      {
        color: "#2563eb",
        markerLabel: "1",
        effort: {
          id: 201,
          rider_user_id: 1,
          activity_id: 81,
          activity_title: "Reference Ride",
          rider_name: "Eric Butera",
          activity_started_at: "2026-05-10T12:00:00Z",
          effort_index: 1,
          duration_seconds: 42,
          start_elapsed_seconds: 0,
          end_elapsed_seconds: 42,
          distance_meters: 1000,
          route_points: [
            makeRoutePoint(0, 45.0, -122.0, {
              distance_meters: 0,
              elevation_meters: 10,
              speed_mps: 9.476,
              heart_rate_bpm: 138,
            }),
            makeRoutePoint(41.688658415029664, 45.01, -121.99, {
              distance_meters: 1000,
              elevation_meters: 30,
              speed_mps: 9.476,
              heart_rate_bpm: 144,
            }),
          ],
        },
      },
      {
        color: "#dc2626",
        markerLabel: "2",
        effort: {
          id: 202,
          rider_user_id: 2,
          activity_id: 82,
          activity_title: "Hill Attack",
          rider_name: "Casey Fast",
          activity_started_at: "2026-05-12T08:00:00Z",
          effort_index: 1,
          duration_seconds: 39,
          start_elapsed_seconds: 0,
          end_elapsed_seconds: 39,
          distance_meters: 1000,
          route_points: [
            makeRoutePoint(0, 45.0, -122.0, {
              distance_meters: 0,
              elevation_meters: 12,
              speed_mps: 9.476,
              heart_rate_bpm: 140,
            }),
            makeRoutePoint(39.000408945136748, 45.01, -121.99, {
              distance_meters: 1000,
              elevation_meters: 28,
              speed_mps: 9.476,
              heart_rate_bpm: 142,
            }),
          ],
        },
      },
    ];
    const tooltipRow = {
      progress: 0.5,
      distanceMeters: 500,
      elevation: 20,
    };

    render(
      <ComparisonGapChartTooltip
        active
        label={500}
        payload={[
          {
            dataKey: "elevation",
            value: 20,
            payload: tooltipRow,
          },
          {
            dataKey: "effort_201",
            value: 0,
            payload: tooltipRow,
          },
          {
            dataKey: "effort_202",
            value: 5,
            payload: tooltipRow,
          },
        ]}
        referenceEffort={selectedRows[0].effort}
        selectedRows={selectedRows}
        unitSystem="imperial"
      />,
    );

    expect(screen.getByText("Time 20.844s")).toBeInTheDocument();

    const rideTwoRow = screen.getByLabelText("Ride 2 tooltip row");
    expect(within(rideTwoRow).getByText("19.500s")).toBeInTheDocument();
    expect(within(rideTwoRow).getByText("21.2 mph")).toBeInTheDocument();
    expect(within(rideTwoRow).getByText("141 bpm")).toBeInTheDocument();
  });
});
