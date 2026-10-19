import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { activityLap } from "../../tests/fixtures/activities";
import ActivityLapsTable from "../activity-detail/ActivityLapsTable";

describe("ActivityLapsTable", () => {
  it.each([
    ["metric", "14.0 km", "31.3 km/h"],
    ["imperial", "8.7 mi", "19.5 mph"],
  ] as const)(
    "shows each lap in a compact %s table row",
    (unitSystem, distance, speed) => {
      render(
        <ActivityLapsTable laps={[activityLap()]} unitSystem={unitSystem} />,
      );
      const table = screen.getByRole("table", { name: "Lap splits" });
      const row = within(table).getByRole("row", { name: /1 Warmup/ });
      expect(
        within(row).getByRole("rowheader", { name: "1" }),
      ).toBeInTheDocument();
      for (const value of [
        "Warmup",
        "26m 40s",
        distance,
        speed,
        "142 bpm",
        "162 bpm",
      ]) {
        expect(
          within(row).getByRole("cell", { name: value }),
        ).toBeInTheDocument();
      }
      expect(table.querySelector(".badge, .stats, .card")).toBeNull();
    },
  );

  it("keeps laps with missing measurements readable", () => {
    render(
      <ActivityLapsTable
        laps={[
          activityLap({
            duration_seconds: null,
            distance_meters: null,
            average_speed_mps: null,
            average_heart_rate_bpm: null,
            max_heart_rate_bpm: null,
          }),
        ]}
        unitSystem="metric"
      />,
    );
    expect(screen.getAllByRole("cell", { name: "--" })).toHaveLength(5);
  });
});
