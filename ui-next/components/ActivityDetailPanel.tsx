"use client";

import { auth } from "@ericbutera/kaleido";
import { faBars } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useMemo, useState, type KeyboardEvent } from "react";
import {
  Area,
  CartesianGrid,
  ComposedChart,
  Line,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import {
  extractApiMessage,
  FEET_PER_METER,
  formatActivityTimestamp,
  formatCadence,
  formatCalories,
  formatDistance,
  formatDuration,
  formatElevation,
  formatHeartRate,
  formatPower,
  formatRelativeEffort,
  formatSpeed,
  formatSport,
  METERS_PER_MILE,
  type UnitSystem,
} from "../lib/activityFormatting";
import {
  buildActivityClimbs,
  type ActivityClimb,
} from "../lib/activityClimbs";
import {
  ACTIVITY_TYPE_OPTIONS,
  ACTIVITY_TYPES,
  formatActivityTypeLabel,
  normalizeActivityType,
  type ActivityType,
} from "../lib/activityTypes";
import {
  type ActivityChartPoint,
  type ActivityLap,
  type ActivityRoutePoint,
  type ActivitySegmentEffort,
  useActivity,
  useDeleteActivity,
  useRegenerateActivity,
  useSegments,
  useUpdateActivity,
  useUpdateSegment,
} from "../lib/queries";
import { zeroBasedDomain } from "../lib/chartDomains";
import { hasSegmentBuilderRoute } from "../lib/segmentBuilder";
import { useUnitPreferences } from "../lib/unitPreferences";
import AuthRequiredCard from "./AuthRequiredCard";
import MapLibreRouteMap from "./MapLibreRouteMap";
import MatchedSegmentsSection from "./MatchedSegmentsSection";
import TrainingProfileSnapshot from "./TrainingProfileSnapshot";

type ChartSeriesPoint = {
  x: number;
  y: number;
};

type SignalMetricKey = "heartRate" | "power" | "speed" | "elevation";

type SegmentTone = {
  mapColor: string;
  dotClassName: string;
  chartClassName: string;
  buttonClassName: string;
  outlineButtonClassName: string;
  highlightClassName: string;
};

type SegmentTrendState = "faster" | "slower" | "steady";

type SegmentMatchGroup = {
  segmentId: number;
  segmentTitle: string;
  efforts: ActivitySegmentEffort[];
  tone: SegmentTone;
  bestEffort: ActivitySegmentEffort;
  bestOverallRank: number | null;
  peakHeartRate: number | null;
  hasHighHeartRate: boolean;
  trendState: SegmentTrendState | null;
  anchorId: string;
};

type SignalSeries = {
  key: SignalMetricKey;
  label: string;
  points: ChartSeriesPoint[];
  buttonClassName: string;
  dotClassName: string;
  strokeColor: string;
  strokeWidth?: number;
  fillColor?: string;
  summary: string;
};

type SignalChartRow = {
  elapsedSeconds: number;
  heartRate?: number | null;
  power?: number | null;
  speed?: number | null;
  elevation?: number | null;
};

const SEGMENT_TONES: SegmentTone[] = [
  {
    mapColor: "#3b82f6",
    dotClassName: "bg-primary",
    chartClassName: "text-primary",
    buttonClassName: "btn-primary",
    outlineButtonClassName: "btn-outline btn-primary",
    highlightClassName: "ring-primary/25",
  },
  {
    mapColor: "#8b5cf6",
    dotClassName: "bg-secondary",
    chartClassName: "text-secondary",
    buttonClassName: "btn-secondary",
    outlineButtonClassName: "btn-outline btn-secondary",
    highlightClassName: "ring-secondary/25",
  },
  {
    mapColor: "#14b8a6",
    dotClassName: "bg-accent",
    chartClassName: "text-accent",
    buttonClassName: "btn-accent",
    outlineButtonClassName: "btn-outline btn-accent",
    highlightClassName: "ring-accent/25",
  },
  {
    mapColor: "#0ea5e9",
    dotClassName: "bg-info",
    chartClassName: "text-info",
    buttonClassName: "btn-info",
    outlineButtonClassName: "btn-outline btn-info",
    highlightClassName: "ring-info/25",
  },
  {
    mapColor: "#f59e0b",
    dotClassName: "bg-warning",
    chartClassName: "text-warning",
    buttonClassName: "btn-warning",
    outlineButtonClassName: "btn-outline btn-warning",
    highlightClassName: "ring-warning/25",
  },
];

const DEFAULT_VISIBLE_SIGNAL_KEYS: SignalMetricKey[] = [
  "heartRate",
  "elevation",
];
const CLIMB_LIST_VISIBLE_ROW_COUNT = 15;
const CLIMB_LIST_MAX_HEIGHT = "40rem";

const SIGNAL_KEY_ORDER: SignalMetricKey[] = [
  "heartRate",
  "power",
  "speed",
  "elevation",
];

function formatElapsedAxisLabel(value: number) {
  if (value <= 0) {
    return "0m";
  }

  const hours = Math.floor(value / 3600);
  const minutes = Math.floor((value % 3600) / 60);

  if (hours > 0) {
    return minutes > 0 ? `${hours}h ${minutes}m` : `${hours}h`;
  }

  if (minutes > 0) {
    return `${minutes}m`;
  }

  return `${value}s`;
}

function buildSeries(
  chartPoints: ActivityChartPoint[] | null | undefined,
  selector: (point: ActivityChartPoint) => number | null | undefined,
) {
  return (chartPoints ?? []).flatMap((point) => {
    const value = selector(point);

    if (value == null || Number.isNaN(value)) {
      return [];
    }

    return [
      { x: Math.max(0, point.elapsed_seconds), y: value },
    ] satisfies ChartSeriesPoint[];
  });
}

function clampRouteIndex(index: number, routeLength: number) {
  if (routeLength <= 0) {
    return 0;
  }

  return Math.max(0, Math.min(index, routeLength - 1));
}

function segmentOverlayPoints(
  routePoints: ActivityRoutePoint[] | null | undefined,
  segmentEffort: ActivitySegmentEffort,
) {
  if (!routePoints || routePoints.length === 0) {
    return [] as ActivityRoutePoint[];
  }

  const startIndex = clampRouteIndex(
    segmentEffort.start_route_point_index,
    routePoints.length,
  );
  const endIndex = clampRouteIndex(
    segmentEffort.end_route_point_index,
    routePoints.length,
  );

  if (startIndex > endIndex) {
    return [];
  }

  return routePoints.slice(startIndex, endIndex + 1);
}

function maxSeriesValue(points: ChartSeriesPoint[]) {
  return points.reduce(
    (maxValue, point) => Math.max(maxValue, point.y),
    points[0]?.y ?? 0,
  );
}

function minSeriesValue(points: ChartSeriesPoint[]) {
  return points.reduce(
    (minValue, point) => Math.min(minValue, point.y),
    points[0]?.y ?? 0,
  );
}

function sortMatchedSegmentEfforts(
  segmentEfforts: ActivitySegmentEffort[] | null | undefined,
) {
  return [...(segmentEfforts ?? [])].sort(
    (left, right) =>
      left.start_route_point_index - right.start_route_point_index ||
      left.end_route_point_index - right.end_route_point_index ||
      left.duration_seconds - right.duration_seconds ||
      (left.overall_rank ?? Number.MAX_SAFE_INTEGER) -
        (right.overall_rank ?? Number.MAX_SAFE_INTEGER) ||
      left.segment_title.localeCompare(right.segment_title) ||
      left.effort_index - right.effort_index,
  );
}

function buildSegmentAnchorId(segmentId: number) {
  return `activity-segment-${segmentId}`;
}

function describeTrendState(
  efforts: ActivitySegmentEffort[],
): SegmentTrendState | null {
  if (efforts.length < 2) {
    return null;
  }

  const firstEffort = efforts[0];
  const lastEffort = efforts[efforts.length - 1];
  const delta = lastEffort.duration_seconds - firstEffort.duration_seconds;
  const threshold = Math.max(
    8,
    Math.round(firstEffort.duration_seconds * 0.03),
  );

  if (delta <= -threshold) {
    return "faster";
  }

  if (delta >= threshold) {
    return "slower";
  }

  return "steady";
}

function groupMatchedSegmentEfforts(
  segmentEfforts: ActivitySegmentEffort[] | null | undefined,
  routePoints: ActivityRoutePoint[] | null | undefined,
  activityAverageHeartRate: number | null | undefined,
  activityMaxHeartRate: number | null | undefined,
): SegmentMatchGroup[] {
  const effortsBySegmentId = new Map<number, ActivitySegmentEffort[]>();

  for (const effort of sortMatchedSegmentEfforts(segmentEfforts)) {
    const existing = effortsBySegmentId.get(effort.segment_id);

    if (existing) {
      existing.push(effort);
      continue;
    }

    effortsBySegmentId.set(effort.segment_id, [effort]);
  }

  return Array.from(effortsBySegmentId.values())
    .map((efforts) => {
      const bestEffort = [...efforts].sort(
        (left, right) =>
          left.duration_seconds - right.duration_seconds ||
          (left.overall_rank ?? Number.MAX_SAFE_INTEGER) -
            (right.overall_rank ?? Number.MAX_SAFE_INTEGER) ||
          left.effort_index - right.effort_index,
      )[0];
      const ranks = efforts
        .flatMap((effort) =>
          effort.overall_rank != null ? [effort.overall_rank] : [],
        )
        .sort((left, right) => left - right);
      const peakHeartRate = efforts.reduce<number | null>((peak, effort) => {
        const maxHeartRate = segmentOverlayPoints(routePoints, effort).reduce<
          number | null
        >((segmentPeak, point) => {
          if (
            point.heart_rate_bpm == null ||
            Number.isNaN(point.heart_rate_bpm)
          ) {
            return segmentPeak;
          }

          return segmentPeak == null || point.heart_rate_bpm > segmentPeak
            ? Math.round(point.heart_rate_bpm)
            : segmentPeak;
        }, null);

        if (maxHeartRate == null) {
          return peak;
        }

        return peak == null || maxHeartRate > peak ? maxHeartRate : peak;
      }, null);
      const hasHighHeartRate =
        peakHeartRate != null &&
        ((activityMaxHeartRate != null &&
          peakHeartRate >= activityMaxHeartRate - 6) ||
          (activityAverageHeartRate != null &&
            peakHeartRate >= activityAverageHeartRate + 10));

      return {
        segmentId: efforts[0].segment_id,
        segmentTitle: efforts[0].segment_title,
        efforts,
        tone: SEGMENT_TONES[0],
        bestEffort,
        bestOverallRank: ranks[0] ?? null,
        peakHeartRate,
        hasHighHeartRate,
        trendState: describeTrendState(efforts),
        anchorId: buildSegmentAnchorId(efforts[0].segment_id),
      };
    })
    .sort(
      (left, right) =>
        left.segmentTitle.localeCompare(right.segmentTitle, undefined, {
          sensitivity: "base",
        }) || left.segmentId - right.segmentId,
    )
    .map((segmentGroup, index) => ({
      ...segmentGroup,
      tone: SEGMENT_TONES[index % SEGMENT_TONES.length],
    }));
}

function buildSignalChartRows(series: SignalSeries[]) {
  const rowsByElapsedSeconds = new Map<number, SignalChartRow>();

  for (const entry of series) {
    for (const point of entry.points) {
      const existing = rowsByElapsedSeconds.get(point.x) ?? {
        elapsedSeconds: point.x,
      };

      rowsByElapsedSeconds.set(point.x, {
        ...existing,
        [entry.key]: point.y,
      });
    }
  }

  return Array.from(rowsByElapsedSeconds.values()).sort(
    (left, right) => left.elapsedSeconds - right.elapsedSeconds,
  );
}

function signalMetricLabel(key: SignalMetricKey) {
  switch (key) {
    case "heartRate":
      return "Heart rate";
    case "power":
      return "Power";
    case "speed":
      return "Speed";
    case "elevation":
      return "Elevation";
  }
}

function formatSignalValue(
  key: SignalMetricKey,
  value: number | null | undefined,
  unitSystem: UnitSystem,
) {
  switch (key) {
    case "heartRate":
      return formatHeartRate(value);
    case "power":
      return formatPower(value);
    case "speed":
      return formatSpeed(value, unitSystem);
    case "elevation":
      return formatElevation(value, unitSystem);
  }
}

function formatGradePercent(value: number | null | undefined) {
  if (value == null || !Number.isFinite(value)) {
    return "--";
  }

  return `${value.toFixed(1)}%`;
}

function distanceUnit(unitSystem: UnitSystem) {
  return unitSystem === "imperial" ? "mi" : "km";
}

function distanceValue(valueMeters: number, unitSystem: UnitSystem) {
  const value =
    unitSystem === "imperial"
      ? valueMeters / METERS_PER_MILE
      : valueMeters / 1000;
  const formatted = value >= 100 ? value.toFixed(0) : value.toFixed(1);

  return formatted.replace(/\.0$/, "");
}

function elevationChartValue(valueMeters: number, unitSystem: UnitSystem) {
  return unitSystem === "imperial" ? valueMeters * FEET_PER_METER : valueMeters;
}

function formatDistanceRange(
  startDistanceMeters: number,
  endDistanceMeters: number,
  unitSystem: UnitSystem,
) {
  return `${distanceValue(startDistanceMeters, unitSystem)} - ${distanceValue(endDistanceMeters, unitSystem)} ${distanceUnit(unitSystem)}`;
}

function formatClimbCategory(category: ActivityClimb["category"]) {
  if (category == null) {
    return "--";
  }

  return category.toString();
}

function buildClimbElevationRows(climb: ActivityClimb, unitSystem: UnitSystem) {
  return climb.routePoints.flatMap((point) => {
    if (
      point.distance_meters == null ||
      point.elevation_meters == null ||
      !Number.isFinite(point.distance_meters) ||
      !Number.isFinite(point.elevation_meters)
    ) {
      return [];
    }

    return [
      {
        distance:
          unitSystem === "imperial"
            ? (point.distance_meters - climb.startDistanceMeters) /
              METERS_PER_MILE
            : (point.distance_meters - climb.startDistanceMeters) / 1000,
        elevation: elevationChartValue(point.elevation_meters, unitSystem),
      },
    ];
  });
}

function ClimbElevationTooltip({
  active,
  payload,
  label,
  unitSystem,
}: {
  active?: boolean;
  payload?: Array<{
    value?: number;
  }>;
  label?: number;
  unitSystem: UnitSystem;
}) {
  if (!active || !payload?.length) {
    return null;
  }

  const elevation = payload[0]?.value;

  return (
    <div className="rounded-box border border-base-300 bg-base-100 px-3 py-2 text-xs shadow-lg">
      <div className="font-semibold text-base-content">
        {typeof label === "number"
          ? `${label.toFixed(1)} ${distanceUnit(unitSystem)}`
          : "--"}
      </div>
      <div className="mt-1 text-base-content/70">
        {unitSystem === "imperial"
          ? `${Math.round(elevation ?? 0)} ft`
          : `${Math.round(elevation ?? 0)} m`}
      </div>
    </div>
  );
}

function SignalChartTooltip({
  active,
  label,
  payload,
  unitSystem,
}: {
  active?: boolean;
  label?: number;
  payload?: Array<{
    color?: string;
    dataKey?: string;
    value?: number;
  }>;
  unitSystem: UnitSystem;
}) {
  if (!active || !payload?.length) {
    return null;
  }

  const orderedItems = payload.flatMap((entry) => {
    if (
      entry.dataKey !== "heartRate" &&
      entry.dataKey !== "power" &&
      entry.dataKey !== "speed" &&
      entry.dataKey !== "elevation"
    ) {
      return [];
    }

    return [
      entry as { color?: string; dataKey: SignalMetricKey; value?: number },
    ];
  });

  return (
    <div className="rounded-box border border-base-300 bg-base-100 px-3 py-3 shadow-lg">
      <p className="text-sm font-semibold text-base-content">
        {formatElapsedAxisLabel(label ?? 0)}
      </p>
      <div className="mt-2 space-y-1.5 text-sm text-base-content/75">
        {orderedItems.map((entry) => (
          <div key={entry.dataKey} className="flex items-center gap-2">
            <span
              aria-hidden
              className="inline-block h-2.5 w-2.5 rounded-full"
              style={{ backgroundColor: entry.color }}
            />
            <span className="font-medium text-base-content">
              {signalMetricLabel(entry.dataKey)}
            </span>
            <span>
              {formatSignalValue(entry.dataKey, entry.value, unitSystem)}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

function SignalChartCard({
  sampleCount,
  series,
  unitSystem,
  visibleKeys,
  onToggle,
}: {
  sampleCount: number;
  series: SignalSeries[];
  unitSystem: UnitSystem;
  visibleKeys: SignalMetricKey[];
  onToggle: (key: SignalMetricKey) => void;
}) {
  const availableSeries = series.filter((entry) => entry.points.length > 1);
  const visibleSeries = availableSeries.filter((entry) =>
    visibleKeys.includes(entry.key),
  );
  const signalXDomain = zeroBasedDomain(
    availableSeries.flatMap((entry) => entry.points.map((point) => point.x)),
  );
  const rows = buildSignalChartRows(availableSeries);

  return (
    <div className="card bg-base-100 shadow-xl">
      <div className="card-body">
        <h2 className="card-title text-xs opacity-60 uppercase">
          Ride signals
        </h2>

        <div className="mt-4 flex flex-wrap gap-2">
          {availableSeries.map((entry) => {
            const isVisible = visibleKeys.includes(entry.key);

            return (
              <button
                key={entry.key}
                type="button"
                className={`btn btn-sm ${isVisible ? entry.buttonClassName : "btn-ghost"}`}
                aria-pressed={isVisible}
                onClick={() => {
                  onToggle(entry.key);
                }}
              >
                {entry.label}
              </button>
            );
          })}
        </div>

        {visibleSeries.length > 0 ? (
          <>
            <div
              role="img"
              aria-label="Activity signals chart"
              className="mt-5 overflow-hidden rounded-box border border-base-300 bg-base-200 p-3"
            >
              <div className="h-[208px] w-full">
                <ResponsiveContainer
                  width="100%"
                  height="100%"
                  minWidth={320}
                  minHeight={208}
                >
                  <ComposedChart
                    data={rows}
                    margin={{ top: 8, right: 8, bottom: 8, left: 0 }}
                  >
                    <CartesianGrid
                      vertical={false}
                      stroke="var(--color-base-content)"
                      strokeOpacity={0.1}
                    />
                    <XAxis
                      axisLine={false}
                      dataKey="elapsedSeconds"
                      domain={signalXDomain}
                      includeHidden
                      tick={{ fill: "var(--color-base-content)", fontSize: 10 }}
                      tickFormatter={(value: number) =>
                        formatElapsedAxisLabel(value)
                      }
                      tickLine={false}
                      type="number"
                    />
                    {visibleSeries.map((entry) => (
                      <YAxis
                        key={`${entry.key}-axis`}
                        hide
                        yAxisId={entry.key}
                        domain={["dataMin", "dataMax"]}
                      />
                    ))}
                    <Tooltip
                      content={<SignalChartTooltip unitSystem={unitSystem} />}
                    />
                    {visibleSeries.map((entry) =>
                      entry.key === "elevation" ? (
                        <Area
                          key={entry.key}
                          type="linear"
                          dataKey={entry.key}
                          yAxisId={entry.key}
                          stroke={entry.strokeColor}
                          fill={entry.fillColor ?? entry.strokeColor}
                          fillOpacity={0.08}
                          strokeOpacity={0.35}
                          strokeWidth={entry.strokeWidth ?? 1.75}
                          dot={false}
                          connectNulls
                        />
                      ) : (
                        <Line
                          key={entry.key}
                          type="linear"
                          dataKey={entry.key}
                          yAxisId={entry.key}
                          stroke={entry.strokeColor}
                          strokeWidth={entry.strokeWidth ?? 2}
                          dot={false}
                          connectNulls
                        />
                      ),
                    )}
                  </ComposedChart>
                </ResponsiveContainer>
              </div>
            </div>

            <div className="mt-4 flex flex-wrap gap-2">
              {visibleSeries.map((entry) => (
                <div
                  key={`${entry.key}-summary`}
                  className="badge badge-outline gap-2 px-3 py-3"
                >
                  <span
                    aria-hidden
                    className={`inline-block h-2.5 w-2.5 rounded-full ${entry.dotClassName}`}
                  />
                  <span>{entry.summary}</span>
                </div>
              ))}
            </div>
          </>
        ) : (
          <div className="alert mt-5">
            <span>Turn on at least one signal layer to render the chart.</span>
          </div>
        )}
      </div>
    </div>
  );
}

function ClimbDetailMetric({
  label,
  value,
}: {
  label: string;
  value: string;
}) {
  return (
    <div className="min-w-0">
      <div className="text-2xl font-semibold text-base-content">{value}</div>
      <div className="mt-1 text-xs text-base-content/55 uppercase">
        {label}
      </div>
    </div>
  );
}

function ActivityClimbsCard({
  climbs,
  selectedClimbId,
  unitSystem,
  onSelectClimb,
  onZoomOutMap,
}: {
  climbs: ActivityClimb[];
  selectedClimbId: string | null;
  unitSystem: UnitSystem;
  onSelectClimb: (climbId: string) => void;
  onZoomOutMap: () => void;
}) {
  const selectedClimb =
    climbs.find((climb) => climb.id === selectedClimbId) ?? null;
  const elevationRows = selectedClimb
    ? buildClimbElevationRows(selectedClimb, unitSystem)
    : [];
  const shouldLimitClimbList = climbs.length > CLIMB_LIST_VISIBLE_ROW_COUNT;

  function handleRowKeyDown(
    event: KeyboardEvent<HTMLTableRowElement>,
    climbId: string,
  ) {
    if (event.key !== "Enter" && event.key !== " ") {
      return;
    }

    event.preventDefault();
    onSelectClimb(climbId);
  }

  return (
    <div id="activity-climbs-card" className="card bg-base-100 shadow-xl">
      <div className="card-body">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <h2 className="card-title text-xl">Climbs</h2>
          <span className="badge badge-outline">
            {climbs.length} climb{climbs.length === 1 ? "" : "s"}
          </span>
        </div>

        {climbs.length > 0 ? (
          <>
            <div
              data-testid="activity-climbs-table-scroll"
              className={`mt-5 overflow-x-auto rounded-box border border-base-300 ${
                shouldLimitClimbList ? "overflow-y-auto" : ""
              }`}
              style={
                shouldLimitClimbList
                  ? { maxHeight: CLIMB_LIST_MAX_HEIGHT }
                  : undefined
              }
            >
              <table className="table table-sm">
                <thead
                  className={
                    shouldLimitClimbList ? "sticky top-0 z-10 bg-base-100" : ""
                  }
                >
                  <tr className="h-10">
                    <th>Climb</th>
                    <th>Distance</th>
                    <th>Elevation</th>
                    <th>Grade</th>
                  </tr>
                </thead>
                <tbody>
                  {climbs.map((climb) => {
                    const isSelected = climb.id === selectedClimbId;

                    return (
                      <tr
                        key={climb.id}
                        role="button"
                        tabIndex={0}
                        aria-label={`Show climb ${climb.sequence} details`}
                        aria-pressed={isSelected}
                        className={`h-10 cursor-pointer transition hover:bg-base-200 focus:bg-base-200 focus:outline-none ${
                          isSelected ? "bg-primary/10" : ""
                        }`}
                        onClick={() => onSelectClimb(climb.id)}
                        onKeyDown={(event) =>
                          handleRowKeyDown(event, climb.id)
                        }
                      >
                        <th scope="row" className="whitespace-nowrap font-semibold">
                          Climb {climb.sequence}
                        </th>
                        <td className="whitespace-nowrap">
                          {formatDistance(climb.distanceMeters, unitSystem)}
                        </td>
                        <td className="whitespace-nowrap">
                          {formatElevation(
                            climb.elevationGainMeters,
                            unitSystem,
                          )}
                        </td>
                        <td className="whitespace-nowrap">
                          {formatGradePercent(climb.avgGradePercent)}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>

            {selectedClimb ? (
              <div className="mt-6 border-t border-base-300 pt-6">
                <div className="flex flex-wrap items-start justify-between gap-3">
                  <h3 className="text-lg font-semibold text-base-content">
                    Climb:{" "}
                    {formatDistanceRange(
                      selectedClimb.startDistanceMeters,
                      selectedClimb.endDistanceMeters,
                      unitSystem,
                    )}
                  </h3>
                  <button
                    type="button"
                    className="btn btn-outline btn-sm"
                    onClick={onZoomOutMap}
                  >
                    Zoom out map
                  </button>
                </div>

                <div className="mt-5 grid gap-x-6 gap-y-5 sm:grid-cols-2 lg:grid-cols-4">
                  <ClimbDetailMetric
                    label="distance"
                    value={formatDistance(
                      selectedClimb.distanceMeters,
                      unitSystem,
                    )}
                  />
                  <ClimbDetailMetric
                    label="elevation gain"
                    value={formatElevation(
                      selectedClimb.elevationGainMeters,
                      unitSystem,
                    )}
                  />
                  <ClimbDetailMetric
                    label="elevation loss"
                    value={formatElevation(
                      selectedClimb.elevationLossMeters,
                      unitSystem,
                    )}
                  />
                  <ClimbDetailMetric
                    label="category"
                    value={formatClimbCategory(selectedClimb.category)}
                  />
                  <ClimbDetailMetric
                    label="max grade"
                    value={formatGradePercent(selectedClimb.maxGradePercent)}
                  />
                  <ClimbDetailMetric
                    label="avg grade"
                    value={formatGradePercent(selectedClimb.avgGradePercent)}
                  />
                  <ClimbDetailMetric
                    label="estimated"
                    value={formatDuration(selectedClimb.durationSeconds)}
                  />
                </div>

                {elevationRows.length > 1 ? (
                  <div
                    role="img"
                    aria-label={`Climb ${selectedClimb.sequence} elevation profile`}
                    className="mt-6 overflow-hidden rounded-box border border-base-300 bg-base-200 p-3"
                  >
                    <div className="h-[180px] w-full">
                      <ResponsiveContainer
                        width="100%"
                        height="100%"
                        minWidth={320}
                        minHeight={180}
                      >
                        <ComposedChart
                          data={elevationRows}
                          margin={{ top: 8, right: 8, bottom: 8, left: 0 }}
                        >
                          <CartesianGrid
                            vertical={false}
                            stroke="var(--color-base-content)"
                            strokeOpacity={0.1}
                          />
                          <XAxis
                            axisLine={false}
                            dataKey="distance"
                            tick={{
                              fill: "var(--color-base-content)",
                              fontSize: 10,
                            }}
                            tickFormatter={(value: number) =>
                              value.toFixed(1)
                            }
                            tickLine={false}
                            type="number"
                          />
                          <YAxis hide domain={["dataMin", "dataMax"]} />
                          <Tooltip
                            content={
                              <ClimbElevationTooltip unitSystem={unitSystem} />
                            }
                          />
                          <Area
                            type="linear"
                            dataKey="elevation"
                            stroke="var(--color-warning)"
                            fill="var(--color-warning)"
                            fillOpacity={0.16}
                            strokeWidth={2}
                            dot={false}
                            connectNulls
                          />
                        </ComposedChart>
                      </ResponsiveContainer>
                    </div>
                  </div>
                ) : null}
              </div>
            ) : null}
          </>
        ) : (
          <div className="alert mt-5">
            <span>No sustained climbs found.</span>
          </div>
        )}
      </div>
    </div>
  );
}

function LapCard({
  lap,
  unitSystem,
}: {
  lap: ActivityLap;
  unitSystem: UnitSystem;
}) {
  return (
    <div className="card bg-base-200 shadow-sm">
      <div className="card-body p-5">
        <div className="flex items-start justify-between gap-3">
          <div>
            <p className="text-sm text-base-content/60">Lap {lap.lap_index}</p>
            <h3 className="card-title text-lg">{lap.title}</h3>
          </div>
          <span className="badge badge-outline">
            {formatDuration(lap.duration_seconds)}
          </span>
        </div>

        <div className="mt-4 grid gap-3 sm:grid-cols-2">
          <DetailMetric
            label="Distance"
            value={formatDistance(lap.distance_meters, unitSystem)}
          />
          <DetailMetric
            label="Average speed"
            value={formatSpeed(lap.average_speed_mps, unitSystem)}
          />
          <DetailMetric
            label="Average heart rate"
            value={formatHeartRate(lap.average_heart_rate_bpm)}
          />
          <DetailMetric
            label="Max heart rate"
            value={formatHeartRate(lap.max_heart_rate_bpm)}
          />
        </div>
      </div>
    </div>
  );
}

function DetailMetric({ label, value }: { label: string; value: string }) {
  return (
    <div className="stats border border-base-300 bg-base-200 shadow-sm">
      <div className="stat px-4 py-4">
        <div className="stat-title">{label}</div>
        <div className="stat-value text-lg sm:text-xl">{value}</div>
      </div>
    </div>
  );
}

function PrimaryActivityStat({
  label,
  value,
  valueClassName,
}: {
  label: string;
  value: string;
  valueClassName?: string;
}) {
  return (
    <div className="min-w-0">
      <div
        className={`truncate text-3xl font-semibold text-base-content sm:text-4xl ${valueClassName ?? ""}`.trim()}
      >
        {value}
      </div>
      <div className="mt-1 text-sm text-base-content/60">{label}</div>
    </div>
  );
}

function SecondaryMetricRow({
  label,
  average,
  maximum,
}: {
  label: string;
  average: string;
  maximum: string;
}) {
  return (
    <tr>
      <th className="font-medium text-base-content">{label}</th>
      <td>{average}</td>
      <td>{maximum}</td>
    </tr>
  );
}

function DenseDetailRow({ label, value }: { label: string; value: string }) {
  return (
    <>
      <dt className="text-base-content/55">{label}</dt>
      <dd className="font-medium text-base-content">{value}</dd>
    </>
  );
}

function ActivityRouteMap({
  routePoints,
  segmentGroups,
  climbs,
  canRegenerate,
  isRegenerating,
  onRegenerate,
  onSelectSegment,
  onSelectClimb,
  selectedSegmentId,
  selectedClimbId,
}: {
  routePoints: ActivityRoutePoint[] | null | undefined;
  segmentGroups: SegmentMatchGroup[];
  climbs: ActivityClimb[];
  canRegenerate: boolean;
  isRegenerating: boolean;
  onRegenerate: () => void;
  onSelectSegment: (segmentId: number) => void;
  onSelectClimb: (climbId: string) => void;
  selectedSegmentId: number | null;
  selectedClimbId: string | null;
}) {
  const hasRouteMap = (routePoints?.length ?? 0) >= 2;
  const selectedClimb =
    climbs.find((climb) => climb.id === selectedClimbId) ?? null;
  const segmentOverlays = segmentGroups
    .flatMap((segmentGroup) =>
      segmentGroup.efforts.map((segmentEffort) => ({
        id: `${segmentEffort.segment_id}-${segmentEffort.effort_index}`,
        color: segmentGroup.tone.mapColor,
        label: segmentGroup.segmentTitle,
        points: segmentOverlayPoints(routePoints, segmentEffort),
        weight: selectedSegmentId === segmentGroup.segmentId ? 8 : 6,
        onClick: () => {
          onSelectSegment(segmentGroup.segmentId);
        },
      })),
    )
    .filter((overlay) => overlay.points.length >= 2);
  const climbOverlays = climbs
    .map((climb) => {
      const isSelected = selectedClimbId === climb.id;

      return {
        id: climb.id,
        color: isSelected ? "#f97316" : "#f59e0b",
        label: `Climb ${climb.sequence}`,
        points: climb.routePoints,
        weight: isSelected ? 9 : 5,
        onClick: () => {
          onSelectClimb(climb.id);
        },
      };
    })
    .filter((overlay) => overlay.points.length >= 2);
  const overlays = [...segmentOverlays, ...climbOverlays];

  return (
    <div className="card bg-base-100 shadow-xl">
      <div className="card-body">
        {hasRouteMap ? (
          <>
            <div className="mt-5 overflow-hidden border border-base-300 bg-base-200">
              <MapLibreRouteMap
                routePoints={routePoints}
                overlays={overlays}
                ariaLabel="Activity route map"
                emptyMessage="This activity does not have enough stored route points for the map yet."
                showZoomControls
                showLayerPicker
                defaultBasemap="topo"
                basemapOptions={["topo", "street", "satellite"]}
                fitBoundsPoints={selectedClimb?.routePoints ?? null}
                fitBoundsKey={selectedClimb ? selectedClimb.id : "activity"}
                fitBoundsMaxZoom={selectedClimb ? 15 : undefined}
                className="h-96 w-full"
              />
            </div>

            {segmentGroups.length > 0 ? (
              <div className="card-actions mt-4 gap-2">
                {segmentGroups.map((segmentGroup) => {
                  const isSelected =
                    selectedSegmentId === segmentGroup.segmentId;

                  return (
                    <button
                      key={`${segmentGroup.segmentId}-legend`}
                      type="button"
                      className={`btn btn-sm ${isSelected ? segmentGroup.tone.buttonClassName : segmentGroup.tone.outlineButtonClassName}`}
                      aria-label={`Jump to ${segmentGroup.segmentTitle} matches`}
                      onClick={() => {
                        onSelectSegment(segmentGroup.segmentId);
                      }}
                    >
                      <span
                        aria-hidden
                        className={`inline-block h-2.5 w-2.5 rounded-full ${segmentGroup.tone.dotClassName}`}
                      />
                      <span>{segmentGroup.segmentTitle}</span>
                      <span className="badge badge-ghost badge-sm">
                        {segmentGroup.efforts.length}
                      </span>
                    </button>
                  );
                })}
              </div>
            ) : null}
          </>
        ) : (
          <div className="alert mt-5">
            <div>
              <p>
                This activity does not have enough stored route points for the
                map yet. Regenerate it once to rebuild the full route geometry
                and re-run segment matching.
              </p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export default function ActivityDetailPanel({
  activityId,
}: {
  activityId: number | string;
}) {
  const [selectedSegmentId, setSelectedSegmentId] = useState<number | null>(
    null,
  );
  const [selectedClimbId, setSelectedClimbId] = useState<string | null>(null);
  const [expandedSegmentIds, setExpandedSegmentIds] = useState<number[]>([]);
  const [isActivityTypeDialogOpen, setIsActivityTypeDialogOpen] =
    useState(false);
  const [activityTypeDraft, setActivityTypeDraft] = useState<ActivityType>(
    ACTIVITY_TYPES.Training,
  );
  const [visibleSignalKeys, setVisibleSignalKeys] = useState<SignalMetricKey[]>(
    DEFAULT_VISIBLE_SIGNAL_KEYS,
  );
  const authApi = auth.useAuthApi();
  const router = useRouter();
  const { user, isLoading: isLoadingUser } = authApi.useCurrentUser();
  const { unitSystem } = useUnitPreferences();
  const activityQuery = useActivity(user ? activityId : null);
  const segmentsQuery = useSegments({ enabled: !!user });
  const regenerateMutation = useRegenerateActivity();
  const deleteMutation = useDeleteActivity();
  const updateActivityMutation = useUpdateActivity();
  const updateSegmentMutation = useUpdateSegment();
  const activity = activityQuery.data;
  const canBuildSegment = hasSegmentBuilderRoute(activity?.route_points);
  const segmentBuilderHref = activity
    ? `/segments/builder?activityId=${activity.id}`
    : "/segments/builder";
  const matchedSegmentEfforts = sortMatchedSegmentEfforts(
    activity?.segment_efforts,
  );
  const activityClimbs = useMemo(
    () => buildActivityClimbs(activity?.route_points),
    [activity?.route_points],
  );
  const matchedSegmentGroups = useMemo(
    () =>
      groupMatchedSegmentEfforts(
        matchedSegmentEfforts,
        activity?.route_points,
        activity?.average_heart_rate_bpm,
        activity?.max_heart_rate_bpm,
      ),
    [
      activity?.average_heart_rate_bpm,
      activity?.max_heart_rate_bpm,
      activity?.route_points,
      matchedSegmentEfforts,
    ],
  );
  const starredSegmentIds = useMemo(
    () =>
      new Set(
        segmentsQuery.data
          .filter((segment) => segment.starred)
          .map((segment) => segment.id),
      ),
    [segmentsQuery.data],
  );
  const starredSegmentIdsKey = useMemo(
    () =>
      Array.from(starredSegmentIds)
        .sort((left, right) => left - right)
        .join(","),
    [starredSegmentIds],
  );

  useEffect(() => {
    if (starredSegmentIds.size === 0) {
      return;
    }

    setExpandedSegmentIds((current) => {
      const next = new Set(current);

      for (const segmentId of starredSegmentIds) {
        next.add(segmentId);
      }

      return next.size === current.length ? current : Array.from(next);
    });
  }, [starredSegmentIds, starredSegmentIdsKey]);

  useEffect(() => {
    if (
      selectedClimbId &&
      !activityClimbs.some((climb) => climb.id === selectedClimbId)
    ) {
      setSelectedClimbId(null);
    }
  }, [activityClimbs, selectedClimbId]);

  useEffect(() => {
    setActivityTypeDraft(normalizeActivityType(activity?.activity_type));
  }, [activity?.activity_type]);

  const heartRateSeries = buildSeries(
    activity?.chart_points,
    (point) => point.heart_rate_bpm,
  );
  const powerSeries = buildSeries(
    activity?.chart_points,
    (point) => point.power_watts,
  );
  const speedSeries = buildSeries(
    activity?.chart_points,
    (point) => point.speed_mps,
  );
  const elevationSeries = buildSeries(
    activity?.chart_points,
    (point) => point.elevation_meters,
  );
  const signalSeries = useMemo<SignalSeries[]>(
    () => [
      {
        key: "heartRate",
        label: "Heart rate",
        points: heartRateSeries,
        buttonClassName: "btn-error",
        dotClassName: "bg-error",
        strokeColor: "var(--color-error)",
        strokeWidth: 2,
        summary: `Peak ${formatHeartRate(maxSeriesValue(heartRateSeries))}`,
      },
      {
        key: "power",
        label: "Power",
        points: powerSeries,
        buttonClassName: "btn-warning",
        dotClassName: "bg-warning",
        strokeColor: "var(--color-warning)",
        strokeWidth: 1.75,
        summary: `Peak ${formatPower(maxSeriesValue(powerSeries))}`,
      },
      {
        key: "speed",
        label: "Speed",
        points: speedSeries,
        buttonClassName: "btn-info",
        dotClassName: "bg-info",
        strokeColor: "var(--color-info)",
        strokeWidth: 1.5,
        summary: `Top speed ${formatSpeed(maxSeriesValue(speedSeries), unitSystem)}`,
      },
      {
        key: "elevation",
        label: "Elevation",
        points: elevationSeries,
        buttonClassName: "btn-success",
        dotClassName: "bg-success",
        strokeColor: "var(--color-success)",
        strokeWidth: 1.75,
        fillColor: "var(--color-success)",
        summary: `${formatElevation(minSeriesValue(elevationSeries), unitSystem)} to ${formatElevation(maxSeriesValue(elevationSeries), unitSystem)}`,
      },
    ],
    [elevationSeries, heartRateSeries, powerSeries, speedSeries, unitSystem],
  );

  function focusSegmentMatch(segmentId: number) {
    setSelectedSegmentId(segmentId);
    setExpandedSegmentIds((current) =>
      current.includes(segmentId) ? current : [...current, segmentId],
    );

    if (typeof document === "undefined") {
      return;
    }

    const matchCard = document.getElementById(buildSegmentAnchorId(segmentId));

    if (!(matchCard instanceof HTMLElement)) {
      return;
    }

    matchCard.scrollIntoView({ behavior: "smooth", block: "start" });

    const firstLink = matchCard.querySelector("a");

    if (firstLink instanceof HTMLElement) {
      firstLink.focus({ preventScroll: true });
    }
  }

  function focusClimb(climbId: string) {
    setSelectedClimbId(climbId);

    if (typeof document === "undefined") {
      return;
    }

    const climbsCard = document.getElementById("activity-climbs-card");

    if (
      climbsCard instanceof HTMLElement &&
      typeof climbsCard.scrollIntoView === "function"
    ) {
      climbsCard.scrollIntoView({ behavior: "smooth", block: "start" });
    }
  }

  function toggleSignalLayer(key: SignalMetricKey) {
    setVisibleSignalKeys((current) => {
      if (current.includes(key)) {
        return current.filter((entry) => entry !== key);
      }

      return SIGNAL_KEY_ORDER.filter(
        (entry) => entry === key || current.includes(entry),
      );
    });
  }

  function toggleSegmentMatch(segmentId: number) {
    setSelectedSegmentId((current) => (current === segmentId ? null : current));
    setExpandedSegmentIds((current) =>
      current.includes(segmentId)
        ? current.filter((entry) => entry !== segmentId)
        : [...current, segmentId],
    );
  }

  async function toggleSegmentStar(segmentId: number, starred: boolean) {
    try {
      await updateSegmentMutation.updateAsync({ id: segmentId, starred });
      if (starred) {
        setExpandedSegmentIds((current) =>
          current.includes(segmentId) ? current : [...current, segmentId],
        );
      }
    } catch {
      // The mutation exposes error state where segment controls are rendered.
    }
  }

  async function handleRegenerate() {
    if (!activity) {
      return;
    }

    try {
      await regenerateMutation.regenerateAsync(activity.id);
    } catch {
      // The mutation exposes the API error state used below.
    }
  }

  async function handleSaveActivityType() {
    if (!activity) {
      return;
    }

    try {
      await updateActivityMutation.updateAsync(activity.id, {
        activity_type: activityTypeDraft,
      });
      setIsActivityTypeDialogOpen(false);
    } catch {
      // The mutation exposes the API error state used below.
    }
  }

  async function handleDelete() {
    if (!activity) {
      return;
    }

    const confirmed =
      typeof globalThis.confirm === "function"
        ? globalThis.confirm(
            "Delete this activity? This removes the activity and clears any derived segment matches.",
          )
        : true;

    if (!confirmed) {
      return;
    }

    try {
      await deleteMutation.deleteAsync(activity.id);
      router.push("/");
    } catch {
      // The mutation exposes the API error state used below.
    }
  }

  if (isLoadingUser || activityQuery.isLoading) {
    return (
      <section className="card bg-base-100 shadow-xl">
        <div className="card-body items-center py-10">
          <span className="loading loading-spinner loading-md" />
        </div>
      </section>
    );
  }

  if (!user) {
    return (
      <AuthRequiredCard
        eyebrow="Activity detail"
        title="Sign in to view activity details"
        description="Activity summaries are scoped per user account, so sign in first to inspect the metrics for this upload."
      />
    );
  }

  if (activityQuery.isError || !activity) {
    return (
      <section className="card bg-base-100 shadow-xl">
        <div className="card-body">
          <div className="alert alert-error">
            {extractApiMessage(activityQuery.error) || "Activity not found"}
          </div>
        </div>
      </section>
    );
  }

  return (
    <section className="space-y-8">
      <div className="card bg-base-100 shadow-xl">
        <div className="card-body gap-6">
          <div className="flex flex-wrap items-start justify-between gap-4">
            <div>
              <h1 className="mt-2 text-4xl font-semibold">{activity.title}</h1>
              <p className="mt-3 text-sm text-base-content/70">
                {formatActivityTimestamp(activity.started_at)}
              </p>
              {activity.location ? (
                <p className="mt-2 text-sm text-base-content/60">
                  {activity.location}
                </p>
              ) : null}
            </div>
            <div className="flex flex-col items-start gap-3 sm:items-end">
              <div className="flex flex-wrap gap-2">
                <span className="badge badge-outline">
                  {formatSport(activity.sport)}
                </span>
                <span className="badge badge-outline">
                  {formatActivityTypeLabel(activity.activity_type)}
                </span>

                <div className="dropdown dropdown-end">
                  <button
                    type="button"
                    tabIndex={0}
                    className="btn btn-ghost btn-square btn-sm"
                    aria-label="Open activity actions"
                  >
                    <FontAwesomeIcon icon={faBars} className="h-4 w-4" />
                  </button>
                  <ul
                    tabIndex={0}
                    className="dropdown-content menu z-20 mt-2 w-56 rounded-box border border-base-300 bg-base-100 p-2 shadow-lg"
                  >
                    <li>
                      {canBuildSegment && (
                        <Link href={segmentBuilderHref}>Build segment</Link>
                      )}
                    </li>
                    <li>
                      <button
                        type="button"
                        onClick={() => {
                          setActivityTypeDraft(
                            normalizeActivityType(activity.activity_type),
                          );
                          setIsActivityTypeDialogOpen(true);
                        }}
                      >
                        Activity type
                      </button>
                    </li>

                    {activity.can_regenerate ? (
                      <li>
                        <button
                          type="button"
                          onClick={() => {
                            void handleRegenerate();
                          }}
                          disabled={regenerateMutation.isPending}
                        >
                          {regenerateMutation.isPending
                            ? "Regenerating..."
                            : "Regenerate derived data"}
                        </button>
                      </li>
                    ) : null}
                    <li>
                      <button
                        type="button"
                        className="text-error"
                        onClick={() => {
                          void handleDelete();
                        }}
                        disabled={deleteMutation.isPending}
                      >
                        {deleteMutation.isPending
                          ? "Deleting..."
                          : "Delete activity"}
                      </button>
                    </li>
                  </ul>
                </div>
              </div>

              {!canBuildSegment ? (
                <p className="text-sm text-base-content/60 sm:text-right">
                  This ride needs stored route points before Bike can build a
                  segment from it.
                </p>
              ) : null}
            </div>
          </div>

          {regenerateMutation.isError ? (
            <div className="alert alert-error">
              {extractApiMessage(regenerateMutation.error)}
            </div>
          ) : null}

          {deleteMutation.isError ? (
            <div className="alert alert-error">
              {extractApiMessage(deleteMutation.error)}
            </div>
          ) : null}

          {updateActivityMutation.isError ? (
            <div className="alert alert-error">
              {extractApiMessage(updateActivityMutation.error)}
            </div>
          ) : null}

          {isActivityTypeDialogOpen ? (
            <div className="modal modal-open">
              <div className="modal-box max-w-md">
                <h2 className="text-xl font-semibold text-base-content">
                  Activity type
                </h2>
                <div className="mt-5 space-y-3">
                  {ACTIVITY_TYPE_OPTIONS.map((option) => (
                    <label
                      key={option.value}
                      className={`flex cursor-pointer items-start gap-3 rounded-box border p-4 ${
                        activityTypeDraft === option.value
                          ? "border-primary bg-primary/10"
                          : "border-base-300 bg-base-100"
                      }`}
                    >
                      <input
                        type="radio"
                        name="activity-type"
                        className="radio radio-primary mt-1"
                        value={option.value}
                        checked={activityTypeDraft === option.value}
                        onChange={(event) =>
                          setActivityTypeDraft(
                            normalizeActivityType(event.target.value),
                          )
                        }
                      />
                      <span>
                        <span className="block font-medium text-base-content">
                          {option.label}
                        </span>
                        <span className="mt-1 block text-sm leading-6 text-base-content/65">
                          {option.description}
                        </span>
                      </span>
                    </label>
                  ))}
                </div>
                <div className="modal-action">
                  <button
                    type="button"
                    className="btn btn-ghost"
                    disabled={updateActivityMutation.isPending}
                    onClick={() => setIsActivityTypeDialogOpen(false)}
                  >
                    Cancel
                  </button>
                  <button
                    type="button"
                    className="btn btn-primary"
                    disabled={updateActivityMutation.isPending}
                    onClick={() => {
                      void handleSaveActivityType();
                    }}
                  >
                    {updateActivityMutation.isPending ? "Saving..." : "Save type"}
                  </button>
                </div>
              </div>
              <button
                type="button"
                className="modal-backdrop"
                aria-label="Close activity type dialog"
                disabled={updateActivityMutation.isPending}
                onClick={() => setIsActivityTypeDialogOpen(false)}
              />
            </div>
          ) : null}

          <div className="grid gap-x-6 gap-y-4 border-b border-base-300 pb-5 sm:grid-cols-2 xl:grid-cols-4">
            <PrimaryActivityStat
              label="Distance"
              value={formatDistance(activity.distance_meters, unitSystem)}
            />
            <PrimaryActivityStat
              label="Moving time"
              value={formatDuration(
                activity.moving_time_seconds ?? activity.total_time_seconds,
              )}
            />
            <PrimaryActivityStat
              label="Elevation"
              value={formatElevation(
                activity.elevation_gain_meters,
                unitSystem,
              )}
            />
            <PrimaryActivityStat
              label="Relative effort"
              value={formatRelativeEffort(activity.relative_effort)}
              valueClassName="text-error"
            />
          </div>

          <div className="space-y-3">
            <div>
              <h2 className="text-lg font-semibold text-base-content">
                Activity data
              </h2>
              <p className="text-sm text-base-content/70">
                Secondary fields are grouped into a tighter stats list so the
                summary stays readable.
              </p>
            </div>

            <div className="grid gap-3 lg:grid-cols-[minmax(0,1.15fr)_minmax(0,0.85fr)] lg:items-start">
              <div className="overflow-x-auto rounded-box border border-base-300 bg-base-100">
                <table className="table table-sm">
                  <thead>
                    <tr>
                      <th>Metric</th>
                      <th>Avg</th>
                      <th>Max</th>
                    </tr>
                  </thead>
                  <tbody>
                    <SecondaryMetricRow
                      label="Speed"
                      average={formatSpeed(
                        activity.average_speed_mps,
                        unitSystem,
                      )}
                      maximum={formatSpeed(activity.max_speed_mps, unitSystem)}
                    />
                    <SecondaryMetricRow
                      label="Heart rate"
                      average={formatHeartRate(activity.average_heart_rate_bpm)}
                      maximum={formatHeartRate(activity.max_heart_rate_bpm)}
                    />
                    <SecondaryMetricRow
                      label="Cadence"
                      average={formatCadence(activity.average_cadence_rpm)}
                      maximum={formatCadence(activity.max_cadence_rpm)}
                    />
                  </tbody>
                </table>
              </div>

              <dl className="grid gap-x-4 gap-y-2 rounded-box border border-base-300 bg-base-100 px-4 py-3 text-sm sm:grid-cols-[auto_1fr]">
                <DenseDetailRow
                  label="Sport"
                  value={formatSport(activity.sport)}
                />
                <DenseDetailRow
                  label="Format"
                  value={activity.format?.toUpperCase() ?? "--"}
                />
                <DenseDetailRow label="Source" value={activity.source} />
                <DenseDetailRow
                  label="Uploaded file"
                  value={activity.original_filename ?? "--"}
                />
                <DenseDetailRow
                  label="Started"
                  value={formatActivityTimestamp(activity.started_at)}
                />
                <DenseDetailRow
                  label="Ended"
                  value={
                    activity.ended_at
                      ? formatActivityTimestamp(activity.ended_at)
                      : "--"
                  }
                />
                <DenseDetailRow
                  label="Location"
                  value={activity.location ?? "--"}
                />
                <DenseDetailRow
                  label="Total time"
                  value={formatDuration(activity.total_time_seconds)}
                />
                <DenseDetailRow
                  label="Elevation loss"
                  value={formatElevation(
                    activity.elevation_loss_meters,
                    unitSystem,
                  )}
                />
                <DenseDetailRow
                  label="Calories"
                  value={formatCalories(activity.calories)}
                />
              </dl>
            </div>
          </div>
        </div>
      </div>

      <ActivityRouteMap
        routePoints={activity.route_points}
        segmentGroups={matchedSegmentGroups}
        climbs={activityClimbs}
        canRegenerate={!!activity.can_regenerate}
        isRegenerating={regenerateMutation.isPending}
        onRegenerate={() => {
          void handleRegenerate();
        }}
        onSelectSegment={focusSegmentMatch}
        onSelectClimb={focusClimb}
        selectedSegmentId={selectedSegmentId}
        selectedClimbId={selectedClimbId}
      />

      <ActivityClimbsCard
        climbs={activityClimbs}
        selectedClimbId={selectedClimbId}
        unitSystem={unitSystem}
        onSelectClimb={focusClimb}
        onZoomOutMap={() => setSelectedClimbId(null)}
      />

      <MatchedSegmentsSection
        segmentGroups={matchedSegmentGroups}
        routePoints={activity.route_points}
        selectedSegmentId={selectedSegmentId}
        expandedSegmentIds={expandedSegmentIds}
        starredSegmentIds={starredSegmentIds}
        updateSegmentPending={updateSegmentMutation.isPending}
        onToggleSegmentMatch={toggleSegmentMatch}
        onToggleSegmentStar={(segmentId, starred) => {
          void toggleSegmentStar(segmentId, starred);
        }}
      />

      <TrainingProfileSnapshot
        estimatedFtpWatts={activity.estimated_ftp_watts}
        heartRateZones={activity.heart_rate_zones}
      />

      <SignalChartCard
        sampleCount={(activity.chart_points ?? []).length}
        series={signalSeries}
        unitSystem={unitSystem}
        visibleKeys={visibleSignalKeys}
        onToggle={toggleSignalLayer}
      />

      <div className="card bg-base-100 shadow-xl">
        <div className="card-body">
          <div className="flex flex-wrap items-start justify-between gap-4">
            <div>
              <h2 className="card-title text-xl">Lap splits</h2>
              <p className="text-sm text-base-content/70">
                These lap rollups come from the upload-time read side and can be
                regenerated when the development flag is enabled.
              </p>
            </div>
            <span className="badge badge-outline">
              {(activity.laps ?? []).length} lap
              {(activity.laps ?? []).length === 1 ? "" : "s"}
            </span>
          </div>

          {(activity.laps ?? []).length > 0 ? (
            <div className="mt-5 grid gap-4 xl:grid-cols-2">
              {(activity.laps ?? []).map((lap) => (
                <LapCard
                  key={`${lap.lap_index}-${lap.title}`}
                  lap={lap}
                  unitSystem={unitSystem}
                />
              ))}
            </div>
          ) : (
            <div className="alert mt-5">
              <span>This upload did not contain explicit lap data.</span>
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
