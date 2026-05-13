import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import SegmentDetailPanel from "../SegmentDetailPanel";

const mocks = vi.hoisted(() => ({
  useCurrentUser: vi.fn(),
  useSegment: vi.fn(),
  renderLeafletRouteMap: vi.fn(),
}));

vi.mock("@ericbutera/kaleido", () => ({
  auth: {
    useAuthApi: () => ({
      useCurrentUser: mocks.useCurrentUser,
    }),
  },
}));

vi.mock("../../lib/queries", () => ({
  useSegment: mocks.useSegment,
}));

vi.mock("../LeafletRouteMap", () => ({
  default: (props: any) => {
    mocks.renderLeafletRouteMap(props);

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
    original_filename: "north-climb.gpx",
    format: "gpx",
    distance_meters: 1800,
    effort_count: 2,
    best_duration_seconds: 312,
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
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("renders sign-in actions when the user is signed out", () => {
    mocks.useCurrentUser.mockReturnValue({ user: null, isLoading: false });

    render(<SegmentDetailPanel segmentId={14} />);

    expect(
      screen.getByText("Sign in to compare segment efforts"),
    ).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Sign in" })).toHaveAttribute(
      "href",
      "/login",
    );
  });

  it("renders the comparison UI for a segment", () => {
    render(<SegmentDetailPanel segmentId={14} />);

    expect(
      screen.getByRole("heading", { name: "North Climb" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("searchbox", { name: "Search efforts" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Selected rides")).toBeInTheDocument();
    expect(screen.getByText("Overall KOM")).toBeInTheDocument();
    expect(screen.getByText("Your PR")).toBeInTheDocument();
    expect(screen.getAllByText("Lunch Ride").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Eric Butera").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Casey Fast").length).toBeGreaterThan(0);
    expect(
      screen.getByRole("img", { name: "Segment comparison map" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("img", { name: "Segment comparison chart" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "5m 12s" })).toHaveAttribute(
      "href",
      "/activities/7",
    );
    expect(screen.getByRole("link", { name: "5m 00s" })).toHaveAttribute(
      "href",
      "/activities/8",
    );
    expect(
      screen.getByRole("button", {
        name: "Remove Hill Attack from comparison",
      }),
    ).toBeInTheDocument();
    expect(screen.getByText("KOM")).toBeInTheDocument();
    expect(screen.getByText("Top 2")).toBeInTheDocument();
    expect(screen.queryByText("PR")).not.toBeInTheDocument();
    expect(screen.queryByText("Hover point")).not.toBeInTheDocument();
    expect(screen.queryByText("Back to home")).not.toBeInTheDocument();
  });

  it("lets the user change the chart metric", async () => {
    const user = userEvent.setup();

    render(<SegmentDetailPanel segmentId={14} />);

    await user.click(screen.getByRole("button", { name: "Heart rate" }));

    expect(screen.getByRole("button", { name: "Heart rate" })).toHaveClass(
      "btn-primary",
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

    render(<SegmentDetailPanel segmentId={14} />);

    expect(screen.getByText("KOM")).toBeInTheDocument();
    expect(screen.queryByText("PR")).not.toBeInTheDocument();
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

    render(<SegmentDetailPanel segmentId={14} />);

    fireEvent.change(
      screen.getByRole("slider", { name: "Playback timeline" }),
      {
        target: { value: "120" },
      },
    );

    const props = mocks.renderLeafletRouteMap.mock.lastCall?.[0];

    expect(props?.movingMarkers).toHaveLength(1);
    expect(props?.movingMarkers[0].point.latitude).toBeCloseTo(45.004, 3);
    expect(props?.movingMarkers[0].point.longitude).toBeCloseTo(-121.996, 3);
  });

  it("filters efforts by the selected time window", async () => {
    const user = userEvent.setup();

    render(<SegmentDetailPanel segmentId={14} />);

    await user.click(screen.getByRole("button", { name: "Day" }));

    expect(screen.getByText("1 of 2 efforts")).toBeInTheDocument();
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

    render(<SegmentDetailPanel segmentId={14} />);

    expect(screen.getByText("2 selected")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Day" }));

    expect(screen.getByText("1 of 2 efforts")).toBeInTheDocument();
    expect(screen.getByText("2 selected")).toBeInTheDocument();
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

    expect(screen.getByText("1 selected")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", {
        name: "Remove Lunch Ride from comparison",
      }),
    ).not.toBeInTheDocument();
  });

  it("filters the effort table by the search query", async () => {
    const user = userEvent.setup();

    render(<SegmentDetailPanel segmentId={14} />);

    await user.type(
      screen.getByRole("searchbox", { name: "Search efforts" }),
      "Casey",
    );

    expect(screen.getByText("1 of 2 efforts")).toBeInTheDocument();
    expect(
      screen.queryByRole("link", { name: "5m 12s" }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: "5m 00s" })).toHaveAttribute(
      "href",
      "/activities/8",
    );
  });

  it("shows the efforts list in a scrollable 10-row viewport", () => {
    const segment = makeSegment();

    segment.efforts = Array.from({ length: 27 }, (_, index) => ({
      ...segment.efforts[0],
      id: index + 1,
      activity_id: index + 101,
      activity_title: `Ride ${index + 1}`,
      rider_name: index % 2 === 0 ? "Eric Butera" : "Casey Fast",
      rider_user_id: index % 2 === 0 ? 1 : 2,
      activity_started_at: `2026-05-${String((index % 9) + 1).padStart(2, "0")}T12:00:00Z`,
      effort_index: index + 1,
      duration_seconds: 300 + index,
    }));
    segment.effort_count = segment.efforts.length;

    mocks.useSegment.mockReturnValue({
      data: segment,
      isLoading: false,
      isError: false,
      error: null,
    });

    render(<SegmentDetailPanel segmentId={14} />);

    const effortsTable = screen.getByLabelText("Segment efforts table");
    const firstEffortRow = screen
      .getByRole("link", { name: "5m 00s" })
      .closest("tr");
    const lastEffortRow = screen
      .getByRole("link", { name: "5m 26s" })
      .closest("tr");

    expect(effortsTable).toHaveClass("overflow-y-auto");
    expect(effortsTable).toHaveAttribute(
      "style",
      expect.stringContaining("max-height: 31rem"),
    );
    expect(
      screen.getByRole("columnheader", { name: "Place" }),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Scroll to see more than 10 efforts"),
    ).toBeInTheDocument();
    expect(firstEffortRow?.querySelector("td")).toHaveTextContent("1");
    expect(screen.getByRole("link", { name: "5m 00s" })).toHaveAttribute(
      "href",
      "/activities/101",
    );
    expect(screen.getByRole("link", { name: "5m 25s" })).toHaveAttribute(
      "href",
      "/activities/126",
    );
    expect(lastEffortRow?.querySelector("td")).toHaveTextContent("27");
    expect(screen.getByRole("link", { name: "5m 26s" })).toHaveAttribute(
      "href",
      "/activities/127",
    );
    expect(
      screen.queryByRole("button", { name: "Next" }),
    ).not.toBeInTheDocument();
    expect(screen.queryByText(/Page 1 of/i)).not.toBeInTheDocument();
  });
});
