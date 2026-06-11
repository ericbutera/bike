import { render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import SegmentRaceViewer from "../segment-detail/SegmentRaceViewer";

const mocks = vi.hoisted(() => ({
  useCurrentUser: vi.fn(),
  useSegment: vi.fn(),
  renderMapLibreRouteMap: vi.fn(),
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
}: {
  id: number;
  activity_title: string;
  activity_started_at: string;
  effort_index: number;
  duration_seconds: number;
  rider_name?: string;
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
    route_points: [
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

function renderRaceViewer() {
  mocks.useCurrentUser.mockReturnValue({
    user: { id: 1, name: "Eric" },
    isLoading: false,
  });
  mocks.useSegment.mockReturnValue({
    data: makeSegment(),
    isLoading: false,
    isError: false,
    error: null,
  });

  return render(
    <SegmentRaceViewer
      segmentId={1}
      initialSelectedEffortIds={[3896, 3954, 3586]}
      initialReferenceEffortId={3896}
    />,
  );
}

function getPlaybackTimer(container: HTMLElement) {
  const timer = container.querySelector(".tabular-nums");

  expect(timer).toHaveTextContent("-- / 1m 35s");

  return timer as HTMLElement;
}

describe("SegmentRaceViewer", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("exposes ride date and attempt number on each effort card", () => {
    renderRaceViewer();

    const secondAttemptCard = screen.getByLabelText(
      /Eric - .*May 6, 2026.* - Attempt 2/,
    );

    expect(secondAttemptCard).toHaveAttribute(
      "title",
      expect.stringMatching(/Eric - .*May 6, 2026.* - Attempt 2/),
    );
    expect(
      within(secondAttemptCard).getByText("Ride date"),
    ).toBeInTheDocument();
    expect(within(secondAttemptCard).getByText("Attempt 2")).toBeInTheDocument();
    expect(
      within(secondAttemptCard).getByText("Lunch Laps"),
    ).toBeInTheDocument();
  });

  it("renders the mobile pace controls beside the playback timer", () => {
    const { container } = renderRaceViewer();

    const timer = getPlaybackTimer(container);
    const mobileControls = timer.parentElement?.querySelector(".sm\\:hidden");

    expect(mobileControls).toBeTruthy();
    expect(
      within(mobileControls as HTMLElement).getByText("Slow"),
    ).toBeInTheDocument();
    expect(
      within(mobileControls as HTMLElement).getByText("Auto"),
    ).toBeInTheDocument();
    expect(
      within(mobileControls as HTMLElement).getByText("Fast"),
    ).toBeInTheDocument();
  });

  it("keeps the timeline on its own full-width row for narrow viewports", () => {
    const { container } = renderRaceViewer();

    const timeline = screen.getByLabelText("Race playback timeline");

    expect(timeline).toHaveClass("w-full");
    expect(timeline.parentElement).not.toBe(
      getPlaybackTimer(container).parentElement,
    );
  });

  it("uses eased map following so race playback does not jump through tight turns", () => {
    renderRaceViewer();

    expect(mocks.renderMapLibreRouteMap).toHaveBeenCalledWith(
      expect.objectContaining({
        followViewportBehavior: "ease",
      }),
    );
  });
});
