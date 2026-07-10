import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import FitnessFreshnessPanel from "../FitnessFreshnessPanel";

const mocks = vi.hoisted(() => ({
  useCurrentUser: vi.fn(),
  useFitnessFreshness: vi.fn(),
}));

vi.mock("@ericbutera/kaleido", () => ({
  auth: {
    useAuthApi: () => ({
      useCurrentUser: mocks.useCurrentUser,
    }),
  },
}));

vi.mock("../../lib/queries", () => ({
  useFitnessFreshness: mocks.useFitnessFreshness,
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

describe("FitnessFreshnessPanel", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-05-07T12:00:00Z"));
    vi.clearAllMocks();

    mocks.useCurrentUser.mockReturnValue({
      user: { id: 1, email: "rider@example.com" },
      isLoading: false,
    });

    mocks.useFitnessFreshness.mockReturnValue({
      data: {
        start_date: "2025-11-07",
        end_date: "2026-05-07",
        fitness_window_days: 42,
        fatigue_window_days: 7,
        points: [
          {
            date: "2026-05-05",
            training_load: 52.4,
            fitness: 31.2,
            fatigue: 44.3,
            form: -13.1,
          },
          {
            date: "2026-05-06",
            training_load: 0,
            fitness: 30.4,
            fatigue: 38.0,
            form: -7.6,
          },
          {
            date: "2026-05-07",
            training_load: 24.1,
            fitness: 30.2,
            fatigue: 32.8,
            form: -2.6,
          },
        ],
      },
      isLoading: false,
      isError: false,
      isFetching: false,
      error: null,
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders a sign-in prompt when the user is signed out", () => {
    mocks.useCurrentUser.mockReturnValue({
      user: null,
      isLoading: false,
    });

    render(<FitnessFreshnessPanel />);

    expect(
      screen.getByText("Sign in to view fitness and freshness"),
    ).toBeInTheDocument();
  });

  it("renders the summary cards and charts", () => {
    render(<FitnessFreshnessPanel />);

    expect(screen.getByText("Fitness & Freshness")).toBeInTheDocument();
    expect(screen.getByText("Current fitness")).toBeInTheDocument();
    expect(screen.getByText("Current fatigue")).toBeInTheDocument();
    expect(screen.getByText("Current form")).toBeInTheDocument();
    expect(screen.getByText("Neutral")).toBeInTheDocument();
    expect(
      screen.getByRole("img", { name: "Fitness and fatigue chart" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Form chart" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Fitness" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByRole("button", { name: "Fatigue" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });

  it("toggles fitness and fatigue series independently", () => {
    render(<FitnessFreshnessPanel />);

    const fitnessToggle = screen.getByRole("button", { name: "Fitness" });
    const fatigueToggle = screen.getByRole("button", { name: "Fatigue" });

    fireEvent.click(fitnessToggle);
    expect(fitnessToggle).toHaveAttribute("aria-pressed", "false");
    expect(fatigueToggle).toHaveAttribute("aria-pressed", "true");

    fireEvent.click(fatigueToggle);
    expect(fitnessToggle).toHaveAttribute("aria-pressed", "false");
    expect(fatigueToggle).toHaveAttribute("aria-pressed", "false");

    fireEvent.click(fitnessToggle);
    expect(fitnessToggle).toHaveAttribute("aria-pressed", "true");
    expect(fatigueToggle).toHaveAttribute("aria-pressed", "false");
  });

  it("keeps the fitness line extent stable when fatigue is toggled off", () => {
    const { container } = render(<FitnessFreshnessPanel />);

    const fitnessPath = () =>
      container.querySelector(
        'path.recharts-line-curve[stroke="#2563eb"]',
      )?.getAttribute("d") ?? "";
    const maxPathX = (path: string) => {
      const values =
        path.match(/[+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?/gi)?.map(Number) ??
        [];
      const xValues = values.filter((_, index) => index % 2 === 0);

      return Math.max(...xValues);
    };
    const beforeMaxX = maxPathX(fitnessPath());

    fireEvent.click(screen.getByRole("button", { name: "Fatigue" }));

    expect(maxPathX(fitnessPath())).toBeCloseTo(beforeMaxX, 3);
  });

  it("requests the selected preset range", () => {
    render(<FitnessFreshnessPanel />);

    fireEvent.click(screen.getByRole("button", { name: "3 months" }));

    expect(mocks.useFitnessFreshness).toHaveBeenLastCalledWith({
      enabled: true,
      startDate: "2026-02-07",
      endDate: "2026-05-07",
    });
  });
});
