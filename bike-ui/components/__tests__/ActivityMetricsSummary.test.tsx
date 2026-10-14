import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { Activity } from "../../lib/queries";
import ActivityMetricsSummary from "../activity-detail/ActivityMetricsSummary";

const activity: Activity = {
  id: 42,
  title: "Fixture walk",
  sport: "walk",
  source: "strava_sync",
  started_at: "2026-10-14T12:00:00Z",
};

describe("ActivityMetricsSummary source metadata", () => {
  it("shows a description and named gear while rendering description as text", () => {
    render(
      <ActivityMetricsSummary
        activity={{
          ...activity,
          description: "A walk <script>fixture</script>",
          gear: { id: "fixture-gear", name: "Fixture shoes" },
        }}
        unitSystem="metric"
      />,
    );
    expect(screen.getByText("Fixture shoes")).toBeInTheDocument();
    expect(
      screen.getByText("A walk <script>fixture</script>"),
    ).toBeInTheDocument();
    expect(document.querySelector("script")).toBeNull();
  });

  it("uses the retained gear ID when Strava supplies no gear name", () => {
    render(
      <ActivityMetricsSummary
        activity={{ ...activity, gear: { id: "fixture-gear" } }}
        unitSystem="metric"
      />,
    );
    expect(screen.getByText("fixture-gear")).toBeInTheDocument();
  });

  it("keeps legacy activities with no metadata readable", () => {
    render(<ActivityMetricsSummary activity={activity} unitSystem="metric" />);
    expect(screen.queryByText("Gear")).not.toBeInTheDocument();
    expect(screen.getByText("strava_sync")).toBeInTheDocument();
  });
});
