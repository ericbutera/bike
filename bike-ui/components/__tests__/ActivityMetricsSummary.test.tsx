import { render, screen, within } from "@testing-library/react";
import { expect, it } from "vitest";
import { activity } from "../../tests/fixtures/activities";
import ActivityMetricsSummary from "../activity-detail/ActivityMetricsSummary";

it("groups the main stats in their own summary card", () => {
  render(
    <ActivityMetricsSummary
      activity={activity({
        distance_meters: 14000,
        total_time_seconds: 1600,
        elevation_gain_meters: 250,
        relative_effort: 106,
      })}
      unitSystem="metric"
    />,
  );
  const summary = within(
    screen.getByRole("region", { name: "Activity summary" }),
  );
  expect(summary.getByText("14.0 km")).toBeInTheDocument();
  expect(summary.getByText("26m 40s")).toBeInTheDocument();
  expect(summary.getByText("250 m")).toBeInTheDocument();
  expect(summary.getByText("106")).toBeInTheDocument();
  expect(summary.queryByText("Activity data")).not.toBeInTheDocument();
});
