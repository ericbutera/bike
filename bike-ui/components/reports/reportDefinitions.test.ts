import { describe, expect, it } from "vitest";
import { findReportDefinition, toReportDefinitions } from "./reportDefinitions";

describe("report definitions", () => {
  it("adapts a supported report and its metric direction for display", () => {
    const definitions = toReportDefinitions([
      {
        id: "distance",
        display_name: "Distance",
        short_purpose: "Track riding volume.",
        supported_filters: ["min_distance"],
        required_data_quality: [],
        result_sections: ["summary"],
        metrics: [
          { key: "distance_miles", label: "Distance", direction: "higher" },
        ],
      },
    ]);

    expect(findReportDefinition("distance", definitions)).toEqual({
      id: "distance",
      name: "Distance",
      purpose: "Track riding volume.",
      supportedFilters: ["min_distance"],
      metrics: ["Distance"],
      metricDirections: { distance_miles: "higher" },
    });
  });

  it("provides the default trends report before remote definitions load", () => {
    const report = findReportDefinition(null, toReportDefinitions());
    expect(report.id).toBe("aggregate_trends");
    expect(report.name).toBe("Aggregate Trends");
    expect(report.metrics).toContain("HR zones");
  });
});
