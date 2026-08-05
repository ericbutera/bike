import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import DhGoalsProgressPanel from "../DhGoalsProgressPanel";
import RequireAuth from "../RequireAuth";

const mocks = vi.hoisted(() => ({
  useCurrentUser: vi.fn(),
  useDhGoalProgress: vi.fn(),
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
  useDhGoalProgress: mocks.useDhGoalProgress,
}));

vi.mock("next/link", () => ({
  default: ({ href, children, ...props }: any) => (
    <a href={href} {...props}>
      {children}
    </a>
  ),
}));

vi.mock("recharts", async () => {
  const actual = await vi.importActual<typeof import("recharts")>("recharts");

  return {
    ...actual,
    ResponsiveContainer: ({ children }: { children: React.ReactNode }) => (
      <div style={{ width: 960, height: 360 }}>{children}</div>
    ),
  };
});

describe("DhGoalsProgressPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();

    mocks.useCurrentUser.mockReturnValue({
      user: { id: 1, email: "rider@example.com" },
      isLoading: false,
    });
    mocks.useDhGoalProgress.mockReturnValue({
      data: {
        generated_at: "2026-05-21T12:00:00Z",
        summary: {
          segment_count: 2,
          session_count: 3,
          effort_count: 9,
          average_efforts_per_session: 3,
          average_repeat_fade_percent: 4.2,
          average_top_3_gap_percent: 2.4,
        },
        goals: [
          {
            key: "dh_laps_per_session",
            label: "DH laps per session",
            unit: "count",
            direction: "at_least",
            current_value: 3,
            target_value: 3,
            progress_percent: 100,
          },
          {
            key: "dh_repeat_fade",
            label: "DH repeat fade",
            unit: "percent",
            direction: "at_most",
            current_value: 4.2,
            target_value: 5,
            progress_percent: 100,
          },
          {
            key: "dh_rolling_top3_gap",
            label: "DH top-3 gap",
            unit: "percent",
            direction: "at_most",
            current_value: 2.4,
            target_value: 3,
            progress_percent: 100,
          },
        ],
        recommendations: [
          {
            key: "maintain_dh_momentum",
            priority: "low",
            title: "Keep the downhill rhythm going",
            detail:
              "You have enough recent DH depth to keep sharpening with one more repeat-lap day this week.",
          },
        ],
        segments: [
          {
            segment_id: 21,
            segment_title: "FMR Upper",
            effort_count: 5,
            personal_record_duration_seconds: 142,
            recent_best_duration_seconds: 145,
            rolling_top_3_average_duration_seconds: 147.3,
            top_3_pr_gap_percent: 3.7,
            repeat_fade_percent: 4.5,
            latest_activity_id: 301,
            latest_activity_title: "Post Canyon Laps",
            latest_activity_started_at: "2026-05-20T18:30:00Z",
          },
        ],
        recent_sessions: [
          {
            activity_id: 301,
            activity_title: "Post Canyon Laps",
            started_at: "2026-05-20T18:30:00Z",
            segment_count: 2,
            effort_count: 4,
            fastest_effort_duration_seconds: 145,
            average_repeat_fade_percent: 4.5,
          },
          {
            activity_id: 302,
            activity_title: "Mt Hood Shuttle Day",
            started_at: "2026-05-18T17:00:00Z",
            segment_count: 1,
            effort_count: 3,
            fastest_effort_duration_seconds: 148,
            average_repeat_fade_percent: 3.8,
          },
        ],
      },
      isLoading: false,
      isFetching: false,
      isError: false,
      error: null,
    });
  });

  it("renders DH goals, session trend, recommendations, and segment benchmarks", () => {
    render(<DhGoalsProgressPanel />);

    expect(screen.getByText("DH goals & progress")).toBeInTheDocument();
    expect(screen.getByText("Recent session shape")).toBeInTheDocument();
    expect(
      screen.getByRole("img", { name: "DH recent sessions chart" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Segment benchmarks")).toBeInTheDocument();
    expect(
      screen.getByText("Keep the downhill rhythm going"),
    ).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "FMR Upper" })).toHaveAttribute(
      "href",
      "/segments/21",
    );
    expect(
      screen
        .getAllByRole("link", { name: "Post Canyon Laps" })
        .every((link) => link.getAttribute("href") === "/activities/301"),
    ).toBe(true);
    expect(screen.getByText("DH laps per session")).toBeInTheDocument();
    expect(screen.getByText("Recent sessions")).toBeInTheDocument();
  });
});
