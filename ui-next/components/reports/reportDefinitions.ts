export type ReportId =
  | "ride_summary"
  | "endurance"
  | "climbing"
  | "fatigue"
  | "compare_rides"
  | "aggregate_trends";

export type ReportStatus = "available" | "planned";

export type ReportDefinition = {
  id: ReportId;
  name: string;
  purpose: string;
  status: ReportStatus;
  metrics: string[];
};

export const REPORT_DEFINITIONS: ReportDefinition[] = [
  {
    id: "ride_summary",
    name: "Ride Summary",
    purpose: "Overall volume, intensity, climbing, stopped time, and data quality.",
    status: "available",
    metrics: ["Distance", "Elevation", "Moving time", "HR zones"],
  },
  {
    id: "endurance",
    name: "Endurance",
    purpose: "Aerobic durability, efficiency, speed, HR, and late-ride drift.",
    status: "available",
    metrics: ["Decoupling", "Hourly efficiency", "Z2 speed", "Late fade"],
  },
  {
    id: "climbing",
    name: "Climbing",
    purpose: "Climb summaries, vertical rate trends, and raw climb rows.",
    status: "available",
    metrics: ["Longest climb", "Median climb", "95th percentile", "Vertical rate"],
  },
  {
    id: "fatigue",
    name: "Fatigue",
    purpose: "Hour-by-hour ride fade across HR, speed, climbing, and stops.",
    status: "available",
    metrics: ["Hourly HR", "Hourly speed", "Climb rate", "Stop frequency"],
  },
  {
    id: "compare_rides",
    name: "Compare Rides",
    purpose: "Side-by-side comparison of selected races and benchmark rides.",
    status: "planned",
    metrics: ["Decoupling", "Climb rate", "Late fade", "Stopped time"],
  },
  {
    id: "aggregate_trends",
    name: "Aggregate Trends",
    purpose: "Existing weekly, monthly, zone, climbing, and elevation charts.",
    status: "available",
    metrics: ["Z2 speed", "Decoupling", "Climbing pace", "HR zones"],
  },
];

export const DEFAULT_REPORT_ID: ReportId = "aggregate_trends";

export function findReportDefinition(value: string | null): ReportDefinition {
  return (
    REPORT_DEFINITIONS.find((definition) => definition.id === value) ??
    REPORT_DEFINITIONS.find((definition) => definition.id === DEFAULT_REPORT_ID)!
  );
}
