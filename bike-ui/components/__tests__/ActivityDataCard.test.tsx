import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { activity as makeActivity } from "../../tests/fixtures/activities";
import ActivityDataCard from "../activity-detail/ActivityDataCard";

const activity = makeActivity({
  title: "Fixture walk",
  sport: "walk",
  source: "strava_sync",
});

describe("ActivityDataCard source metadata", () => {
  it("shows a description and named gear while rendering description as text", () => {
    render(
      <ActivityDataCard
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
      <ActivityDataCard
        activity={{ ...activity, gear: { id: "fixture-gear" } }}
        unitSystem="metric"
      />,
    );
    expect(screen.getByText("fixture-gear")).toBeInTheDocument();
  });

  it("keeps legacy activities with no metadata readable", () => {
    render(<ActivityDataCard activity={activity} unitSystem="metric" />);
    expect(screen.queryByText("Gear")).not.toBeInTheDocument();
    expect(screen.queryByText("strava_sync")).not.toBeInTheDocument();
    expect(
      screen.getByText("Elapsed time").nextElementSibling,
    ).toHaveTextContent("--");
  });

  it("shows elapsed time and notes in the grid and omits provenance and empty measurement rows", () => {
    render(
      <ActivityDataCard
        activity={{
          ...activity,
          format: "fit",
          original_filename: "fixture.fit",
          average_speed_mps: 4,
          max_speed_mps: 8,
          total_time_seconds: 900,
          private_notes: "Personal note",
        }}
        unitSystem="metric"
      />,
    );
    expect(screen.getByText("14.4 km/h")).toBeVisible();
    const measurements = screen.getByRole("table", {
      name: "Activity measurements",
    });
    expect(
      within(measurements).queryByText("Elapsed time"),
    ).not.toBeInTheDocument();
    const elapsedTime = screen.getByText("Elapsed time");
    expect(elapsedTime.tagName).toBe("DT");
    expect(elapsedTime.nextElementSibling).toHaveTextContent("15m 00s");
    expect(elapsedTime.nextElementSibling).toBeVisible();
    expect(
      screen.queryByRole("row", { name: /Heart rate|Cadence/ }),
    ).not.toBeInTheDocument();
    expect(screen.getByText("Personal note")).toBeVisible();
    expect(screen.getByText("Started")).toBeVisible();
    expect(screen.getByText("Ended")).toBeVisible();
    for (const label of [
      "Sport",
      "Source",
      "Format",
      "Uploaded file",
      "FIT",
      "fixture.fit",
      "strava_sync",
    ]) {
      expect(
        screen.queryByText(label, { exact: true }),
      ).not.toBeInTheDocument();
    }
    expect(
      screen.queryByRole("button", { name: /Show more|Show less/ }),
    ).not.toBeInTheDocument();
  });
});
