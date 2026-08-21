import { fireEvent, render, screen, within } from "@testing-library/react";
import { type ComponentProps } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import RequireAuth from "../RequireAuth";
import SegmentRaceViewer from "../segment-detail/SegmentRaceViewer";

const mocks = vi.hoisted(() => ({
  useCurrentUser: vi.fn(),
  useSegment: vi.fn(),
  useSegmentComparison: vi.fn(),
  renderMapLibreRouteMap: vi.fn(),
}));

vi.mock("@ericbutera/kaleido", () => ({
  auth: {
    useAuthApi: () => ({
      useCurrentUser: mocks.useCurrentUser,
    }),
  },
  LoadingCard: () => <div aria-label="Loading" />,
  LoadingSpinner: (props: any) => <span aria-hidden="true" {...props} />,
}));

vi.mock("../../lib/queries", () => ({
  useSegment: mocks.useSegment,
  useSegmentComparison: mocks.useSegmentComparison,
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

function makeRoutePoint(elapsed_seconds: number) {
  return {
    elapsed_seconds,
    latitude: 45 + elapsed_seconds / 10_000,
    longitude: -122 + elapsed_seconds / 10_000,
    distance_meters: elapsed_seconds * 8,
  };
}

function makeEffort({
  id,
  activity_title,
  activity_started_at,
  effort_index,
  duration_seconds,
  rider_name = "Eric",
  route_points,
}: {
  id: number;
  activity_title: string;
  activity_started_at: string;
  effort_index: number;
  duration_seconds: number;
  rider_name?: string;
  route_points?: ReturnType<typeof makeRoutePoint>[];
}) {
  return {
    id,
    rider_user_id: 1,
    activity_id: id + 1000,
    activity_title,
    rider_name,
    activity_started_at,
    effort_index,
    duration_seconds,
    start_elapsed_seconds: 0,
    end_elapsed_seconds: duration_seconds,
    distance_meters: 1000,
    route_points: route_points ?? [
      makeRoutePoint(0),
      makeRoutePoint(duration_seconds / 2),
      makeRoutePoint(duration_seconds),
    ],
  };
}

function makeSegment() {
  return {
    id: 1,
    title: "North Ridge Sprint",
    source: "manual",
    mode: "dh" as const,
    effort_count: 3,
    best_duration_seconds: 95,
    current_user_pr_duration_seconds: 95,
    created_at: "2026-05-01T12:00:00Z",
    route_points: [makeRoutePoint(0), makeRoutePoint(120)],
    efforts: [
      makeEffort({
        id: 3896,
        activity_title: "Lunch Laps",
        activity_started_at: "2026-05-06T12:00:00Z",
        effort_index: 1,
        duration_seconds: 95,
      }),
      makeEffort({
        id: 3954,
        activity_title: "Lunch Laps",
        activity_started_at: "2026-05-06T12:00:00Z",
        effort_index: 2,
        duration_seconds: 99,
      }),
      makeEffort({
        id: 3586,
        activity_title: "Evening Ride",
        activity_started_at: "2026-05-08T22:30:00Z",
        effort_index: 3,
        duration_seconds: 105,
      }),
    ],
  };
}

function renderRaceViewer({
  segment = makeSegment(),
  selectedEffortIds = [3896, 3954, 3586],
  initialPlaybackSpeed,
}: {
  segment?: ReturnType<typeof makeSegment>;
  selectedEffortIds?: number[];
  initialPlaybackSpeed?: ComponentProps<
    typeof SegmentRaceViewer
  >["initialPlaybackSpeed"];
} = {}) {
  mocks.useCurrentUser.mockReturnValue({
    user: { id: 1, name: "Eric" },
    isLoading: false,
  });
  mocks.useSegment.mockReturnValue({
    data: segment,
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

  return render(
    <RequireAuth>
      <SegmentRaceViewer
        segmentId={1}
        initialSelectedEffortIds={selectedEffortIds}
        initialPlaybackSpeed={initialPlaybackSpeed}
      />
    </RequireAuth>,
  );
}

function getPlaybackTimer(container: HTMLElement) {
  const timer = container.querySelector(".tabular-nums");

  expect(timer).toHaveTextContent("-- / 1m 45s");

  return timer as HTMLElement;
}

describe("SegmentRaceViewer", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("exposes ride date and attempt number on each effort card", () => {
    renderRaceViewer();

    const secondAttemptCard = screen.getByTitle(
      /Eric - .*May 6, 2026.* - Attempt 2/,
    );

    expect(secondAttemptCard).toHaveAttribute(
      "title",
      expect.stringMatching(/Eric - .*May 6, 2026.* - Attempt 2/),
    );
    expect(
      within(secondAttemptCard).getByText("Ride date"),
    ).toBeInTheDocument();
    expect(
      within(secondAttemptCard).getByText("Attempt 2"),
    ).toBeInTheDocument();
    expect(
      within(secondAttemptCard).getByText("Lunch Laps"),
    ).toBeInTheDocument();
  });

  it("links back to the segment detail with the selected efforts", () => {
    renderRaceViewer();

    expect(screen.getByRole("link", { name: "Back" })).toHaveAttribute(
      "href",
      "/segments/1?efforts=3896%2C3954%2C3586",
    );
  });

  it("removes a ride from the race viewer and back link selection", () => {
    renderRaceViewer();

    fireEvent.click(
      screen.getByRole("button", {
        name: /Remove Eric - .*May 6, 2026.* - Attempt 2 from race viewer/,
      }),
    );

    expect(
      screen.queryByTitle(/Eric - .*May 6, 2026.* - Attempt 2/),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Back" })).toHaveAttribute(
      "href",
      "/segments/1?efforts=3896%2C3586",
    );
  });

  it("renders exact speed values in a dropdown slider", () => {
    const { container } = renderRaceViewer();

    const timer = getPlaybackTimer(container);
    const speedControl = screen.getByRole("button", {
      name: "Race playback speed",
    });

    expect(speedControl).toHaveTextContent("1x");
    expect(timer.parentElement).toContainElement(speedControl);

    const speedSlider = screen.getByLabelText("Race playback speed slider");
    expect(speedSlider).toHaveAttribute("max", "9");

    fireEvent.change(speedSlider, { target: { value: "9" } });

    expect(speedControl).toHaveTextContent("4x");
    expect(screen.getByRole("button", { name: "0.10x" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "0.25x" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "4x" })).toBeInTheDocument();
  });

  it("honors an initially requested race playback speed", () => {
    renderRaceViewer({ initialPlaybackSpeed: 0.25 });

    expect(
      screen.getByRole("button", { name: "Race playback speed" }),
    ).toHaveTextContent("0.25x");
  });

  it("keeps effort cards in selected order while live lead labels update", () => {
    const segment = makeSegment();
    segment.efforts = [
      makeEffort({
        id: 1,
        activity_title: "Warmup Run",
        activity_started_at: "2026-05-06T12:00:00Z",
        effort_index: 1,
        duration_seconds: 140,
        rider_name: "Taylor",
      }),
      makeEffort({
        id: 2,
        activity_title: "Race Run",
        activity_started_at: "2026-05-06T13:00:00Z",
        effort_index: 1,
        duration_seconds: 95,
        rider_name: "Jordan",
      }),
      makeEffort({
        id: 3,
        activity_title: "Evening Run",
        activity_started_at: "2026-05-06T14:00:00Z",
        effort_index: 1,
        duration_seconds: 125,
        rider_name: "Morgan",
      }),
    ];

    renderRaceViewer({
      segment,
      selectedEffortIds: [1, 2, 3],
    });

    fireEvent.change(screen.getByLabelText("Race playback timeline"), {
      target: { value: "100" },
    });

    const cards = screen
      .getAllByTitle(/(Taylor|Jordan|Morgan) - .* - Attempt 1/)
      .filter((card) => card.getAttribute("aria-label")?.includes(" - "));

    expect(cards.map((card) => card.getAttribute("aria-label"))).toEqual([
      expect.stringMatching(/^Taylor/),
      expect.stringMatching(/^Jordan/),
      expect.stringMatching(/^Morgan/),
    ]);
    expect(within(cards[1]).getByText("Lead")).toBeInTheDocument();
  });

  it("delegates user zoom preservation to the route map", () => {
    renderRaceViewer();

    const mapProps = mocks.renderMapLibreRouteMap.mock.calls.at(-1)?.[0] ?? {};

    expect(mapProps).toEqual(
      expect.objectContaining({
        followViewportBehavior: "jump",
        followViewportPreserveUserZoom: true,
      }),
    );
  });

  it("keeps the timeline on its own full-width row for narrow viewports", () => {
    const { container } = renderRaceViewer();

    const timeline = screen.getByLabelText("Race playback timeline");

    expect(timeline).toHaveClass("w-full");
    expect(timeline.parentElement).not.toBe(
      getPlaybackTimer(container).parentElement,
    );
  });

  it("uses direct map following so race playback does not stack camera animations", () => {
    renderRaceViewer();

    expect(mocks.renderMapLibreRouteMap).toHaveBeenCalledWith(
      expect.objectContaining({
        followViewportBehavior: "jump",
      }),
    );
  });

  it("keeps race follow zoom fixed instead of auto zooming from marker spread", () => {
    const segment = makeSegment();
    segment.route_points = [
      { ...makeRoutePoint(0), latitude: 45, longitude: -122 },
      { ...makeRoutePoint(240), latitude: 45.02, longitude: -121.98 },
    ];
    segment.efforts = [
      makeEffort({
        id: 1,
        activity_title: "Leader",
        activity_started_at: "2026-05-06T12:00:00Z",
        effort_index: 1,
        duration_seconds: 240,
        route_points: [
          { ...makeRoutePoint(0), latitude: 45, longitude: -122 },
          { ...makeRoutePoint(240), latitude: 45.02, longitude: -121.98 },
        ],
      }),
      makeEffort({
        id: 2,
        activity_title: "Runner Up",
        activity_started_at: "2026-05-06T13:00:00Z",
        effort_index: 1,
        duration_seconds: 260,
        route_points: [
          { ...makeRoutePoint(0), latitude: 45, longitude: -122 },
          { ...makeRoutePoint(260), latitude: 45.02, longitude: -121.98 },
        ],
      }),
      makeEffort({
        id: 3,
        activity_title: "Third",
        activity_started_at: "2026-05-06T14:00:00Z",
        effort_index: 1,
        duration_seconds: 280,
        route_points: [
          { ...makeRoutePoint(0), latitude: 45, longitude: -122 },
          { ...makeRoutePoint(280), latitude: 45.02, longitude: -121.98 },
        ],
      }),
    ];

    renderRaceViewer({
      segment,
      selectedEffortIds: [1, 2, 3],
    });

    fireEvent.change(screen.getByLabelText("Race playback timeline"), {
      target: { value: "180" },
    });

    const mapProps = mocks.renderMapLibreRouteMap.mock.calls.at(-1)?.[0] ?? {};

    expect(mapProps.followViewport).toEqual(
      expect.objectContaining({
        zoom: 19,
      }),
    );
  });
});
