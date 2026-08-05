import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ACTIVITY_TYPES } from "../../lib/activityTypes";
import RequireAuth from "../RequireAuth";
import XcGoalsProgressPanel from "../XcGoalsProgressPanel";

const mocks = vi.hoisted(() => ({
  useActivityProcessingState: vi.fn(),
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
  LoadingCard: () => <div aria-label="Loading" />,
  LoadingSpinner: (props: any) => <span aria-hidden="true" {...props} />,
}));

vi.mock("../../lib/queries", () => ({
  useActivityProcessingState: mocks.useActivityProcessingState,
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

function renderPanel() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <XcGoalsProgressPanel />
    </QueryClientProvider>,
  );
}

describe("XcGoalsProgressPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();

    mocks.useActivityProcessingState.mockReturnValue({
      data: null,
      isLoading: false,
      isError: false,
      error: null,
    });
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
        xc_goal_event_name: "Lumberjack 100",
        xc_goal_start_date: "2026-03-01",
        xc_goal_target_date: "2026-09-20",
        xc_goal_target_distance_meters: 160934.4,
        xc_goal_target_elevation_gain_meters: 3962.4,
        xc_goal_target_finish_time_seconds: 32400,
        xc_goal_event_profile: "endurance_mtb",
      },
      isLoading: false,
      isError: false,
      error: null,
    });
    mocks.useXcGoalProgress.mockReturnValue({
      data: {
        generated_at: "2026-05-21T12:00:00Z",
        event_goal: {
          event_name: "Lumberjack 100",
          event_profile: "endurance_mtb",
          start_date: "2026-03-01",
          target_date: "2026-09-20",
          days_remaining: 122,
          target_distance_meters: 160934.4,
          target_elevation_gain_meters: 3962.4,
          target_finish_time_seconds: 32400,
          target_finish_speed_mps: 4.97,
          target_climb_density_meters_per_kilometer: 24.6,
          training_window_days: 204,
          counted_ride_count: 12,
          counted_distance_meters: 76800,
          counted_elevation_gain_meters: 1860,
        },
        readiness: {
          status: "falling_behind",
          title: "Falling behind",
          reason:
            "Event specificity is the biggest limiter against the saved event target.",
          missing_most: "Event specificity",
          gates: [
            {
              key: "long_ride_distance",
              label: "Recent long ride",
              status: "watch",
              unit: "meters",
              direction: "at_least",
              current_value: 120000,
              target_value: 104607,
              gap_value: 0,
              progress_percent: 100,
              detail: "Best single ride in the last 90 days.",
            },
            {
              key: "big_climb_day",
              label: "Recent climb day",
              status: "on_track",
              unit: "meters",
              direction: "at_least",
              current_value: 2700,
              target_value: 1783.1,
              gap_value: 0,
              progress_percent: 100,
              detail: "Best single climbing ride in the last 90 days.",
            },
            {
              key: "climb_density",
              label: "Climb density",
              status: "falling_behind",
              unit: "meters_per_kilometer",
              direction: "at_least",
              current_value: 12,
              target_value: 19.7,
              gap_value: 7.7,
              progress_percent: 30,
              detail:
                "Training-block climbing per distance compared with the event's climb density.",
            },
            {
              key: "aerobic_decoupling",
              label: "Aerobic decoupling",
              status: "watch",
              unit: "percent",
              direction: "at_most",
              current_value: 5.8,
              target_value: 5,
              gap_value: 0.8,
              progress_percent: 86.2,
              detail: "Recent comparable endurance drift. Lower is better.",
            },
          ],
        },
        deficits: [
          {
            key: "event_specificity",
            priority: "high",
            title: "Event specificity",
            detail:
              "Current rides are not matching the event's climbing per mile closely enough.",
            gap_value: 7.7,
            gap_unit: "meters_per_kilometer",
            suggested_ride: {
              purpose: "climb_durability",
              duration_seconds_min: 7200,
              duration_seconds_max: 10800,
              distance_meters_min: 32000,
              distance_meters_max: 52000,
              climbing_elevation_gain_meters: 1050,
              intensity: "Z2 endurance",
              terrain: "Rolling singletrack climbs",
              detail:
                "Keep the effort mostly aerobic and collect steady climbing.",
            },
          },
        ],
        summary: {
          recent_window_days: 28,
          recent_ride_count: 3,
          comparable_ride_count: 2,
          total_z2_time_seconds: 16200,
          total_climbing_time_seconds: 4200,
          total_climbing_elevation_gain_meters: 1800,
          average_aerobic_decoupling_percent: 4.6,
        },
        race_results: [
          {
            activity_id: 201,
            activity_title: "2026 Lumberjack 100 Race Result",
            started_at: "2026-06-20T12:00:00Z",
            distance_meters: 160934.4,
            elevation_gain_meters: 3048,
            moving_time_seconds: 32400,
            average_speed_mps: 4.97,
            climb_density_meters_per_kilometer: 18.9,
            z2_time_seconds: 14400,
            climbing_time_seconds: 7200,
            climbing_elevation_gain_meters: 3048,
            aerobic_decoupling_percent: 6.2,
            prior_training_ride_count: 3,
            prior_training_z2_time_seconds: 16200,
            prior_training_climbing_elevation_gain_meters: 1800,
            prior_training_average_z2_speed_mps: 3.2,
            prior_training_average_aerobic_decoupling_percent: 4.6,
            race_vs_best_training_distance_percent: 473.3,
            race_vs_best_training_elevation_percent: 358.6,
            insight_title: "Race distance outpaced the recent endurance build",
            insight_detail:
              "The result was much longer than your biggest prior training ride while recent Z2 volume was below the v1 build target.",
          },
        ],
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
            priority: "high",
            title: "Choose a more event-like route",
            detail:
              "Current rides are not matching the event's climbing per mile closely enough.",
            purpose: "climb_durability",
            limiter: "Event specificity",
            gap_value: 7.7,
            gap_unit: "meters_per_kilometer",
            suggested_ride: {
              purpose: "climb_durability",
              duration_seconds_min: 7200,
              duration_seconds_max: 10800,
              distance_meters_min: 32000,
              distance_meters_max: 52000,
              climbing_elevation_gain_meters: 1050,
              intensity: "Z2 endurance",
              terrain: "Rolling singletrack climbs",
              detail:
                "Keep the effort mostly aerobic and collect steady climbing.",
            },
          },
        ],
        weekly_progress: [
          {
            week_start: "2026-05-05",
            ride_count: 1,
            comparable_ride_count: 1,
            distance_meters: 32000,
            z2_time_seconds: 7200,
            z2_distance_meters: 21960,
            average_z2_speed_mps: 3.05,
            climbing_time_seconds: 1800,
            climbing_elevation_gain_meters: 800,
            climbing_vertical_rate_meters_per_hour: 1600,
            average_aerobic_decoupling_percent: 4.2,
            z1_seconds: 1200,
            z2_zone_seconds: 7200,
            z3_seconds: 900,
            z4_seconds: 0,
            z5_seconds: 0,
          },
          {
            week_start: "2026-05-12",
            ride_count: 2,
            comparable_ride_count: 1,
            distance_meters: 56000,
            z2_time_seconds: 9000,
            z2_distance_meters: 29700,
            average_z2_speed_mps: 3.3,
            climbing_time_seconds: 2400,
            climbing_elevation_gain_meters: 1000,
            climbing_vertical_rate_meters_per_hour: 1500,
            average_aerobic_decoupling_percent: 5.0,
            z1_seconds: 1500,
            z2_zone_seconds: 9000,
            z3_seconds: 1200,
            z4_seconds: 300,
            z5_seconds: 0,
          },
        ],
        recent_rides: [
          {
            activity_id: 101,
            activity_title: "Post Canyon Endurance",
            started_at: "2026-05-20T12:00:00Z",
            activity_type: ACTIVITY_TYPES.Training,
            ride_focus: "xc_endurance",
            route_family_key: "post-canyon",
            distance_meters: 34000,
            elevation_gain_meters: 850,
            z2_time_seconds: 7200,
            z1_seconds: 1200,
            z2_zone_seconds: 7200,
            z3_seconds: 900,
            z4_seconds: 0,
            z5_seconds: 0,
            climbing_time_seconds: 1800,
            climbing_elevation_gain_meters: 850,
            aerobic_decoupling_percent: 4.2,
            training_purpose: "climb_durability",
            training_purpose_detail:
              "Useful for accumulating sustained climbing in the event build.",
          },
          {
            activity_id: 102,
            activity_title: "Mixed Trail Spin",
            started_at: "2026-05-15T12:00:00Z",
            activity_type: ACTIVITY_TYPES.Training,
            ride_focus: "mixed_xc",
            route_family_key: null,
            distance_meters: 22000,
            elevation_gain_meters: 500,
            z2_time_seconds: 3600,
            z1_seconds: 900,
            z2_zone_seconds: 3600,
            z3_seconds: 600,
            z4_seconds: 0,
            z5_seconds: 0,
            climbing_time_seconds: 1200,
            climbing_elevation_gain_meters: 500,
            aerobic_decoupling_percent: null,
            training_purpose: "base_endurance",
            training_purpose_detail:
              "Useful for aerobic volume but not yet a full event-specific benchmark.",
          },
          {
            activity_id: 103,
            activity_title: "Ride 103",
            started_at: "2026-05-13T12:00:00Z",
            activity_type: ACTIVITY_TYPES.Training,
            ride_focus: "xc_endurance",
            route_family_key: "post-canyon",
            distance_meters: 24000,
            elevation_gain_meters: 450,
            z2_time_seconds: 3600,
            z1_seconds: 800,
            z2_zone_seconds: 3600,
            z3_seconds: 600,
            z4_seconds: 0,
            z5_seconds: 0,
            climbing_time_seconds: 900,
            climbing_elevation_gain_meters: 450,
            aerobic_decoupling_percent: 4.8,
            training_purpose: "base_endurance",
            training_purpose_detail:
              "Useful for keeping aerobic volume consistent.",
          },
          {
            activity_id: 104,
            activity_title: "Ride 104",
            started_at: "2026-05-11T12:00:00Z",
            activity_type: ACTIVITY_TYPES.Training,
            ride_focus: "mixed_xc",
            route_family_key: null,
            distance_meters: 18000,
            elevation_gain_meters: 380,
            z2_time_seconds: 2700,
            z1_seconds: 600,
            z2_zone_seconds: 2700,
            z3_seconds: 300,
            z4_seconds: 0,
            z5_seconds: 0,
            climbing_time_seconds: 720,
            climbing_elevation_gain_meters: 380,
            aerobic_decoupling_percent: null,
            training_purpose: "base_endurance",
            training_purpose_detail:
              "Useful for keeping aerobic volume consistent.",
          },
          {
            activity_id: 105,
            activity_title: "Ride 105",
            started_at: "2026-05-09T12:00:00Z",
            activity_type: ACTIVITY_TYPES.Training,
            ride_focus: "xc_endurance",
            route_family_key: "post-canyon",
            distance_meters: 26000,
            elevation_gain_meters: 520,
            z2_time_seconds: 3900,
            z1_seconds: 700,
            z2_zone_seconds: 3900,
            z3_seconds: 450,
            z4_seconds: 0,
            z5_seconds: 0,
            climbing_time_seconds: 960,
            climbing_elevation_gain_meters: 520,
            aerobic_decoupling_percent: 5.1,
            training_purpose: "climb_durability",
            training_purpose_detail:
              "Useful for accumulating sustained climbing in the event build.",
          },
          {
            activity_id: 106,
            activity_title: "Ride 106",
            started_at: "2026-05-07T12:00:00Z",
            activity_type: ACTIVITY_TYPES.Training,
            ride_focus: "xc_endurance",
            route_family_key: "post-canyon",
            distance_meters: 28000,
            elevation_gain_meters: 540,
            z2_time_seconds: 4200,
            z1_seconds: 800,
            z2_zone_seconds: 4200,
            z3_seconds: 500,
            z4_seconds: 0,
            z5_seconds: 0,
            climbing_time_seconds: 1020,
            climbing_elevation_gain_meters: 540,
            aerobic_decoupling_percent: 4.9,
            training_purpose: "base_endurance",
            training_purpose_detail:
              "Useful for keeping aerobic volume consistent.",
          },
        ],
      },
      isLoading: false,
      isFetching: false,
      isError: false,
      error: null,
    });
  });

  it("renders XC goals, charts, recommendations, and recent rides", () => {
    renderPanel();

    expect(screen.getByText("Event target")).toBeInTheDocument();
    expect(screen.getAllByText("Lumberjack 100").length).toBeGreaterThan(0);
    expect(
      screen.getByRole("button", { name: "Edit target" }),
    ).toBeInTheDocument();
    expect(screen.queryByDisplayValue("2026-09-20")).not.toBeInTheDocument();
    expect(
      screen.queryByText("Current read on the target"),
    ).not.toBeInTheDocument();
    expect(screen.getByText("Qualifying rides")).toBeInTheDocument();
    expect(screen.getByText("Target density")).toBeInTheDocument();
    expect(screen.getByText("Current density")).toBeInTheDocument();
    expect(
      screen.getByText(/Training block: Mar 1, 2026 to Sep 20, 2026/),
    ).toBeInTheDocument();
    expect(
      screen.queryByText(/This target is the course demand model/),
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Edit target" }));
    expect(screen.getByDisplayValue("2026-09-20")).toBeInTheDocument();
    expect(screen.getByDisplayValue("9")).toBeInTheDocument();
    expect(screen.getByText("Target Sep 20, 2026")).toBeInTheDocument();
    expect(screen.getByText("Quick status")).toBeInTheDocument();
    expect(screen.getAllByText("Falling behind").length).toBeGreaterThan(0);
    expect(screen.getByText("What am I missing?")).toBeInTheDocument();
    expect(screen.getAllByText("Event specificity").length).toBeGreaterThan(0);
    expect(screen.getByText("Recent long ride")).toBeInTheDocument();
    expect(screen.getByText("Recent climb day")).toBeInTheDocument();
    expect(screen.getByText("Climb density")).toBeInTheDocument();
    expect(
      screen.getByTitle("Best single ride in the last 90 days."),
    ).toBeInTheDocument();
    expect(
      screen.queryByText("Best single ride in the last 90 days."),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("Distance covered")).not.toBeInTheDocument();
    expect(screen.queryByText("Climbing covered")).not.toBeInTheDocument();
    expect(screen.queryByText("Distance pace")).not.toBeInTheDocument();
    expect(screen.queryByText("Climbing pace")).not.toBeInTheDocument();
    expect(screen.getByText("Trends over time")).toBeInTheDocument();
    expect(screen.getByText("Weekly volume trend")).toBeInTheDocument();
    expect(screen.getByText("Durability trend")).toBeInTheDocument();
    expect(screen.getByText("Time in zones")).toBeInTheDocument();
    expect(screen.getByText("Weekly distance")).toBeInTheDocument();
    expect(screen.getByText("Weekly climbing")).toBeInTheDocument();
    expect(screen.getAllByText("Aerobic decoupling").length).toBeGreaterThan(0);
    expect(screen.getByText("Zone 2 share")).toBeInTheDocument();
    expect(
      screen.getByRole("img", {
        name: "XC weekly distance and climbing trend chart",
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("img", {
        name: "XC Z2 speed climbing rate and decoupling trend chart",
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("img", { name: "XC weekly time in zones chart" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Next ride guidance")).toBeInTheDocument();
    expect(screen.getByText("Race result insights")).toBeInTheDocument();
    expect(
      screen.getByRole("link", {
        name: "2026 Lumberjack 100 Race Result",
      }),
    ).toHaveAttribute("href", "/activities/201");
    expect(
      screen.getByText("Race distance outpaced the recent endurance build"),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Choose a more event-like route"),
    ).toBeInTheDocument();
    expect(screen.getAllByText("Climb durability").length).toBeGreaterThan(0);
    expect(
      screen.getByLabelText("Choose a more event-like route details"),
    ).toHaveAttribute(
      "title",
      "Current rides are not matching the event's climbing per mile closely enough.",
    );
    expect(
      screen.getAllByTitle(
        "Keep the effort mostly aerobic and collect steady climbing.",
      ).length,
    ).toBeGreaterThan(0);
    expect(
      screen.queryByText(
        "Useful for accumulating sustained climbing in the event build.",
      ),
    ).not.toBeInTheDocument();
    expect(
      screen.getAllByTitle(
        "Useful for accumulating sustained climbing in the event build.",
      ).length,
    ).toBeGreaterThan(0);
    expect(
      screen
        .getAllByRole("link", { name: "Post Canyon Endurance" })
        .every((link) => link.getAttribute("href") === "/activities/101"),
    ).toBe(true);
    expect(screen.getAllByText("XC endurance").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Mixed XC").length).toBeGreaterThan(0);
    expect(screen.getByText("Showing 1-5 of 6")).toBeInTheDocument();
    expect(
      screen.queryByRole("link", { name: "Ride 106" }),
    ).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Next" }));

    expect(screen.getByRole("link", { name: "Ride 106" })).toHaveAttribute(
      "href",
      "/activities/106",
    );
  });
});
