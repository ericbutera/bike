import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import XcGoalsProgressPanel from "../XcGoalsProgressPanel";

const mocks = vi.hoisted(() => ({
  useCurrentUser: vi.fn(),
  useUpdateUserPreferences: vi.fn(),
  useUserPreferences: vi.fn(),
  useXcGoalProgress: vi.fn(),
  updateAsync: vi.fn(),
}));

vi.mock("@ericbutera/kaleido", () => ({
  auth: {
    useAuthApi: () => ({
      useCurrentUser: mocks.useCurrentUser,
    }),
  },
}));

vi.mock("../../lib/queries", () => ({
  useUpdateUserPreferences: mocks.useUpdateUserPreferences,
  useUserPreferences: mocks.useUserPreferences,
  useXcGoalProgress: mocks.useXcGoalProgress,
}));

vi.mock("react-hot-toast", () => ({
  default: {
    error: vi.fn(),
    success: vi.fn(),
  },
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

describe("XcGoalsProgressPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();

    mocks.useCurrentUser.mockReturnValue({
      user: { id: 1, email: "rider@example.com" },
      isLoading: false,
    });
    mocks.useUpdateUserPreferences.mockReturnValue({
      updateAsync: mocks.updateAsync,
      isPending: false,
    });
    mocks.useUserPreferences.mockReturnValue({
      data: {
        unit_system: "mixed",
        xc_goal_target_date: "2026-09-20",
        xc_goal_target_distance_meters: 160934.4,
        xc_goal_target_elevation_gain_meters: 3962.4,
      },
      isLoading: false,
      isError: false,
      error: null,
    });
    mocks.useXcGoalProgress.mockReturnValue({
      data: {
        generated_at: "2026-05-21T12:00:00Z",
        event_goal: {
          target_date: "2026-09-20",
          days_remaining: 122,
          target_distance_meters: 160934.4,
          target_elevation_gain_meters: 3962.4,
          best_distance_meters: 120000,
          best_distance_progress_percent: 74.6,
          best_distance_activity: {
            activity_id: 101,
            activity_title: "Post Canyon Endurance",
            started_at: "2026-05-20T12:00:00Z",
          },
          best_elevation_gain_meters: 2700,
          best_elevation_gain_progress_percent: 68.1,
          best_elevation_activity: {
            activity_id: 101,
            activity_title: "Post Canyon Endurance",
            started_at: "2026-05-20T12:00:00Z",
          },
        },
        summary: {
          recent_window_days: 28,
          recent_ride_count: 3,
          comparable_ride_count: 2,
          total_z2_time_seconds: 16200,
          total_climbing_time_seconds: 4200,
          total_climbing_elevation_gain_meters: 1800,
          average_aerobic_decoupling_percent: 4.6,
        },
        goals: [
          {
            key: "weekly_z2_average",
            label: "Weekly Z2 average",
            unit: "seconds",
            direction: "at_least",
            current_value: 4050,
            target_value: 14400,
            progress_percent: 28.1,
          },
          {
            key: "weekly_climbing_average",
            label: "Weekly climbing average",
            unit: "meters",
            direction: "at_least",
            current_value: 450,
            target_value: 1500,
            progress_percent: 30,
          },
          {
            key: "aerobic_decoupling",
            label: "Aerobic decoupling",
            unit: "percent",
            direction: "at_most",
            current_value: 4.6,
            target_value: 5,
            progress_percent: 100,
          },
        ],
        recommendations: [
          {
            key: "add_climbing_endurance",
            priority: "medium",
            title: "Add more climbing durability",
            detail:
              "A longer climbing-focused endurance ride would improve the climbing side of the XC progression model.",
          },
        ],
        weekly_progress: [
          {
            week_start: "2026-05-05",
            ride_count: 1,
            comparable_ride_count: 1,
            z2_time_seconds: 7200,
            climbing_elevation_gain_meters: 800,
            average_aerobic_decoupling_percent: 4.2,
          },
          {
            week_start: "2026-05-12",
            ride_count: 2,
            comparable_ride_count: 1,
            z2_time_seconds: 9000,
            climbing_elevation_gain_meters: 1000,
            average_aerobic_decoupling_percent: 5.0,
          },
        ],
        recent_rides: [
          {
            activity_id: 101,
            activity_title: "Post Canyon Endurance",
            started_at: "2026-05-20T12:00:00Z",
            ride_focus: "xc_endurance",
            route_family_key: "post-canyon",
            distance_meters: 34000,
            elevation_gain_meters: 850,
            z2_time_seconds: 7200,
            climbing_time_seconds: 1800,
            climbing_elevation_gain_meters: 850,
            aerobic_decoupling_percent: 4.2,
          },
          {
            activity_id: 102,
            activity_title: "Mixed Trail Spin",
            started_at: "2026-05-15T12:00:00Z",
            ride_focus: "mixed_xc",
            route_family_key: null,
            distance_meters: 22000,
            elevation_gain_meters: 500,
            z2_time_seconds: 3600,
            climbing_time_seconds: 1200,
            climbing_elevation_gain_meters: 500,
            aerobic_decoupling_percent: null,
          },
        ],
      },
      isLoading: false,
      isFetching: false,
      isError: false,
      error: null,
    });
  });

  it("renders a sign-in prompt when the user is signed out", () => {
    mocks.useCurrentUser.mockReturnValue({ user: null, isLoading: false });

    render(<XcGoalsProgressPanel />);

    expect(screen.getByText("XC goals & progress")).toBeInTheDocument();
    expect(
      screen.getByRole("link", { name: "Sign in to view XC progress" }),
    ).toHaveAttribute("href", "/login");
  });

  it("renders XC goals, charts, recommendations, and recent rides", () => {
    render(<XcGoalsProgressPanel />);

    expect(screen.getByText("Event target")).toBeInTheDocument();
    expect(screen.getByDisplayValue("2026-09-20")).toBeInTheDocument();
    expect(screen.getByText("Target Sep 20, 2026")).toBeInTheDocument();
    expect(screen.getByText("Weekly endurance load")).toBeInTheDocument();
    expect(
      screen.getByRole("img", { name: "XC weekly progression chart" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("img", { name: "XC decoupling trend chart" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Next ride guidance")).toBeInTheDocument();
    expect(
      screen.getByText("Add more climbing durability"),
    ).toBeInTheDocument();
    expect(
      screen
        .getAllByRole("link", { name: "Post Canyon Endurance" })
        .every((link) => link.getAttribute("href") === "/activities/101"),
    ).toBe(true);
    expect(screen.getByText("XC endurance")).toBeInTheDocument();
    expect(screen.getByText("Mixed XC")).toBeInTheDocument();
  });
});
