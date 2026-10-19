import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { climbingRoutePoints } from "../../tests/fixtures/activities";
import { buildActivityClimbs } from "../../lib/activityClimbs";
import ActivityClimbsPanel from "../activity-detail/ActivityClimbsPanel";

vi.mock("recharts", async (importOriginal) => {
  const actual = await importOriginal<typeof import("recharts")>();
  const React = await import("react");
  return {
    ...actual,
    ResponsiveContainer: ({ children }: { children: React.ReactElement }) =>
      React.cloneElement(
        children as React.ReactElement<{ width: number; height: number }>,
        { width: 960, height: 240 },
      ),
  };
});

describe("ActivityClimbsPanel", () => {
  it("identifies the selected climb separately from its activity distance range", () => {
    render(
      <ActivityClimbsPanel
        activityId={42}
        climbs={buildActivityClimbs(climbingRoutePoints())}
        active
        unitSystem="imperial"
      />,
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Show climb 1 details" }),
    );
    expect(
      screen.getByRole("heading", { name: "Climb 1" }),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Activity distance: 0 - 1.5 mi"),
    ).toBeInTheDocument();
  });

  it.each(["metric", "imperial"] as const)(
    "extends the %s elevation curve to both ends of the plot",
    (unitSystem) => {
      const routePoints = climbingRoutePoints().map((point, index) => ({
        ...point,
        distance_meters: index * 570,
      }));
      render(
        <ActivityClimbsPanel
          activityId={42}
          climbs={buildActivityClimbs(routePoints)}
          active
          unitSystem={unitSystem}
        />,
      );
      fireEvent.click(
        screen.getByRole("button", { name: "Show climb 1 details" }),
      );
      const profile = screen.getByRole("img", {
        name: "Climb 1 elevation profile",
      });
      const curve = profile.querySelector(".recharts-area-curve")!;
      const coordinates = [
        ...curve.getAttribute("d")!.matchAll(/[ML]([\d.]+),/g),
      ].map((match) => Number(match[1]));
      const grid = profile.querySelector(
        ".recharts-cartesian-grid-horizontal line",
      )!;
      expect(coordinates[0]).toBeCloseTo(Number(grid.getAttribute("x1")), 1);
      expect(coordinates.at(-1)).toBeCloseTo(
        Number(grid.getAttribute("x2")),
        1,
      );
    },
  );
});
