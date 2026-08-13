import type {
  TrainingReportDefinition,
  TrainingReportFilterKey,
  TrainingReportId,
  TrainingReportMetricDirection,
} from "../../lib/queries";

export type ReportId =
  | "ride_summary"
  | "endurance"
  | "climbing"
  | "fatigue"
  | "compare_rides"
  | "reassessment"
  | "aggregate_trends";

export type ReportDefinition = {
  id: ReportId;
  name: string;
  purpose: string;
  metrics: string[];
  supportedFilters: TrainingReportFilterKey[];
  metricDirections: Record<string, TrainingReportMetricDirection>;
};

export const FALLBACK_REPORT_DEFINITIONS: ReportDefinition[] = [
  {
    id: "ride_summary",
    name: "Ride Summary",
    purpose:
      "Overall volume, intensity, climbing, stopped time, and data quality.",
    metrics: ["Distance", "Elevation", "Moving time", "HR zones"],
    supportedFilters: ["min_duration", "min_distance"],
    metricDirections: {},
  },
  {
    id: "endurance",
    name: "Endurance",
    purpose: "Aerobic durability, efficiency, speed, HR, and late-ride drift.",
    metrics: ["Decoupling", "Hourly efficiency", "Z2 speed", "Late fade"],
    supportedFilters: ["min_duration", "min_distance"],
    metricDirections: {},
  },
  {
    id: "climbing",
    name: "Climbing",
    purpose: "Climb summaries, vertical rate trends, and raw climb rows.",
    metrics: [
      "Longest climb",
      "Median climb",
      "95th percentile",
      "Vertical rate",
    ],
    supportedFilters: ["min_duration", "min_distance"],
    metricDirections: {},
  },
  {
    id: "fatigue",
    name: "Fatigue",
    purpose: "Hour-by-hour ride fade across HR, speed, climbing, and stops.",
    metrics: ["Hourly HR", "Hourly speed", "Climb rate", "Stop frequency"],
    supportedFilters: ["min_duration", "min_distance"],
    metricDirections: {},
  },
  {
    id: "compare_rides",
    name: "Compare Rides",
    purpose: "Side-by-side comparison of selected races and benchmark rides.",
    metrics: [
      "Moving speed",
      "Z2 speed",
      "Decoupling",
      "Climb rate",
      "Late fade",
    ],
    supportedFilters: ["activity_ids", "min_duration", "min_distance"],
    metricDirections: {},
  },
  {
    id: "reassessment",
    name: "Reassessment",
    purpose:
      "Reassess the active XC event goal from endurance, climbing density, elapsed long-ride pace, and spring-baseline fitness delta.",
    metrics: [
      "Endurance progression",
      "Climbing density",
      "Elapsed long-ride pace",
      "Fitness delta",
    ],
    supportedFilters: ["min_duration", "min_distance"],
    metricDirections: {},
  },
  {
    id: "aggregate_trends",
    name: "Aggregate Trends",
    purpose: "Existing weekly, monthly, zone, climbing, and elevation charts.",
    metrics: ["Z2 speed", "Decoupling", "Climbing pace", "HR zones"],
    supportedFilters: ["min_duration", "min_distance"],
    metricDirections: {},
  },
];

export const DEFAULT_REPORT_ID: ReportId = "aggregate_trends";

export function toReportDefinitions(
  definitions?: TrainingReportDefinition[] | null,
): ReportDefinition[] {
  if (!definitions || definitions.length === 0) {
    return FALLBACK_REPORT_DEFINITIONS;
  }

  return definitions.map((definition) => ({
    id: normalizeReportId(definition.id),
    name: definition.display_name,
    purpose: definition.short_purpose,
    metrics: definition.metrics.map((metric) => metric.label),
    supportedFilters: definition.supported_filters,
    metricDirections: Object.fromEntries(
      definition.metrics.map((metric) => [metric.key, metric.direction]),
    ),
  }));
}

export function findReportDefinition(
  value: string | null,
  definitions: ReportDefinition[] = FALLBACK_REPORT_DEFINITIONS,
): ReportDefinition {
  return (
    definitions.find((definition) => definition.id === value) ??
    definitions.find((definition) => definition.id === DEFAULT_REPORT_ID) ??
    FALLBACK_REPORT_DEFINITIONS.find(
      (definition) => definition.id === DEFAULT_REPORT_ID,
    )!
  );
}

function normalizeReportId(value: TrainingReportId): ReportId {
  return value;
}
