"use client";

import { auth } from "@ericbutera/kaleido";
import {
  faCrown,
  faFileLines,
  faMagnifyingGlass,
  faMedal,
  faMinus,
  faPause,
  faPlay,
  faPlus,
  faRoute,
  faTrophy,
  faXmark,
} from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import Link from "next/link";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  Area,
  CartesianGrid,
  ComposedChart,
  Line,
  ReferenceDot,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import {
  extractApiMessage,
  formatActivityTimestamp,
  formatDistance,
  formatDuration,
  formatElevation,
  formatHeartRate,
  formatSpeed,
  type UnitSystem,
} from "../lib/activityFormatting";
import {
  type ActivityRoutePoint,
  type SegmentEffort,
  useSegment,
} from "../lib/queries";
import { primarySegmentAchievement } from "../lib/segmentAchievements";
import { useUnitPreferences } from "../lib/unitPreferences";
import AuthRequiredCard from "./AuthRequiredCard";
import LeafletRouteMap from "./LeafletRouteMap";

const EFFORT_COLORS = [
  "#0f766e",
  "#dc2626",
  "#2563eb",
  "#d97706",
  "#7c3aed",
  "#0891b2",
  "#65a30d",
  "#be123c",
  "#4338ca",
  "#c2410c",
];
const MAX_SELECTED_EFFORTS = 10;
const EFFORTS_VISIBLE_ROWS = 10;
const EFFORTS_TABLE_MAX_HEIGHT_REM = 31;

const EFFORT_TIME_FILTERS = [
  { key: "all", label: "All" },
  { key: "day", label: "Day" },
  { key: "week", label: "Week" },
  { key: "month", label: "Month" },
  { key: "year", label: "Year" },
] as const;

type ChartMetric = "speed" | "heartRate" | "elevation";
type EffortTimeFilter = (typeof EFFORT_TIME_FILTERS)[number]["key"];

type ComparisonChartRow = {
  elapsedSeconds: number;
  elevation?: number | null;
  [key: string]: number | null | undefined;
};

type ComparisonSeries = {
  effort: SegmentEffort;
  color: string;
  dataKey: string;
  points: Array<{ x: number; y: number }>;
};

type SelectedEffortRow = {
  effort: SegmentEffort;
  color: string;
};

function startOfDay(value: Date) {
  const next = new Date(value);
  next.setHours(0, 0, 0, 0);
  return next;
}

function effortWindowStart(filter: EffortTimeFilter, now: Date) {
  if (filter === "all") {
    return null;
  }

  const next = startOfDay(now);

  if (filter === "day") {
    return next;
  }

  if (filter === "week") {
    next.setDate(next.getDate() - ((next.getDay() + 6) % 7));
    return next;
  }

  if (filter === "month") {
    next.setDate(1);
    return next;
  }

  next.setMonth(0, 1);
  return next;
}

function filterEffortsByTimeWindow(
  efforts: SegmentEffort[] | null | undefined,
  filter: EffortTimeFilter,
  nowMs = Date.now(),
) {
  const windowStart = effortWindowStart(filter, new Date(nowMs));

  if (!windowStart) {
    return [...(efforts ?? [])];
  }

  return (efforts ?? []).filter((effort) => {
    const startedAt = new Date(effort.activity_started_at);
    return startedAt >= windowStart;
  });
}

function fastestEffort(efforts: SegmentEffort[] | null | undefined) {
  return (efforts ?? []).reduce<SegmentEffort | null>((best, effort) => {
    if (!best || effort.duration_seconds < best.duration_seconds) {
      return effort;
    }

    return best;
  }, null);
}

function overallEffortRanks(efforts: SegmentEffort[] | null | undefined) {
  const ranked = [...(efforts ?? [])].sort(
    (left, right) =>
      left.duration_seconds - right.duration_seconds || left.id - right.id,
  );

  return new Map(ranked.map((effort, index) => [effort.id, index + 1]));
}

function filterEffortsBySearchQuery(
  efforts: SegmentEffort[],
  rawQuery: string,
) {
  const query = rawQuery.trim().toLowerCase();

  if (!query) {
    return efforts;
  }

  return efforts.filter((effort) => {
    const haystacks = [
      effort.rider_name,
      effort.activity_title,
      effort.activity_started_at,
    ];

    return haystacks.some((value) => value.toLowerCase().includes(query));
  });
}

function interpolateRoutePoint(
  points: ActivityRoutePoint[] | null | undefined,
  elapsedSeconds: number,
) {
  if (!points || points.length === 0) {
    return null;
  }

  if (elapsedSeconds <= points[0].elapsed_seconds) {
    return points[0];
  }

  for (let index = 1; index < points.length; index += 1) {
    const previous = points[index - 1];
    const current = points[index];

    if (elapsedSeconds <= current.elapsed_seconds) {
      const span = Math.max(
        current.elapsed_seconds - previous.elapsed_seconds,
        1,
      );
      const progress = (elapsedSeconds - previous.elapsed_seconds) / span;
      return {
        ...current,
        latitude:
          previous.latitude + (current.latitude - previous.latitude) * progress,
        longitude:
          previous.longitude +
          (current.longitude - previous.longitude) * progress,
      };
    }
  }

  return points.at(-1) ?? null;
}

function clampProgress(value: number) {
  return Math.min(Math.max(value, 0), 1);
}

function interpolateRoutePointByProgress(
  points: ActivityRoutePoint[] | null | undefined,
  progress: number,
) {
  if (!points || points.length === 0) {
    return null;
  }

  if (points.length === 1) {
    return points[0];
  }

  const clampedProgress = clampProgress(progress);

  if (clampedProgress <= 0) {
    return points[0];
  }

  const firstDistance = points[0].distance_meters;
  const lastDistance = points.at(-1)?.distance_meters;
  const hasDistanceRange =
    typeof firstDistance === "number" &&
    typeof lastDistance === "number" &&
    lastDistance > firstDistance &&
    points.every((point) => typeof point.distance_meters === "number");
  const targetMeasure = hasDistanceRange
    ? firstDistance + clampedProgress * (lastDistance - firstDistance)
    : clampedProgress * (points.length - 1);

  for (let index = 1; index < points.length; index += 1) {
    const previous = points[index - 1];
    const current = points[index];
    const previousMeasure = hasDistanceRange
      ? (previous.distance_meters as number)
      : index - 1;
    const currentMeasure = hasDistanceRange
      ? (current.distance_meters as number)
      : index;

    if (targetMeasure <= currentMeasure) {
      const span = Math.max(currentMeasure - previousMeasure, Number.EPSILON);
      const localProgress = (targetMeasure - previousMeasure) / span;

      return {
        ...current,
        elapsed_seconds: Math.round(
          previous.elapsed_seconds +
            (current.elapsed_seconds - previous.elapsed_seconds) *
              localProgress,
        ),
        latitude:
          previous.latitude +
          (current.latitude - previous.latitude) * localProgress,
        longitude:
          previous.longitude +
          (current.longitude - previous.longitude) * localProgress,
        distance_meters: hasDistanceRange
          ? targetMeasure - firstDistance
          : current.distance_meters,
      };
    }
  }

  return points.at(-1) ?? null;
}

function comparisonMarkerPoint(
  segmentRoutePoints: ActivityRoutePoint[] | null | undefined,
  effort: SegmentEffort,
  playbackSeconds: number,
) {
  const effortPoint = interpolateRoutePoint(
    effort.route_points,
    playbackSeconds,
  );

  if (!effortPoint) {
    return null;
  }

  const effortPoints = effort.route_points ?? [];
  const firstDistance = effortPoints[0]?.distance_meters;
  const lastDistance = effortPoints.at(-1)?.distance_meters;
  const progress =
    typeof effortPoint.distance_meters === "number" &&
    typeof firstDistance === "number" &&
    typeof lastDistance === "number" &&
    lastDistance > firstDistance
      ? clampProgress(
          (effortPoint.distance_meters - firstDistance) /
            (lastDistance - firstDistance),
        )
      : effort.duration_seconds > 0
        ? clampProgress(playbackSeconds / effort.duration_seconds)
        : 0;

  return interpolateRoutePointByProgress(segmentRoutePoints, progress);
}

function formatMetricValue(
  metric: ChartMetric,
  value: number | null | undefined,
  unitSystem: UnitSystem,
) {
  if (metric === "speed") {
    return formatSpeed(value, unitSystem);
  }

  if (metric === "heartRate") {
    return formatHeartRate(value);
  }

  return formatElevation(value, unitSystem);
}

function getMetricValue(metric: ChartMetric, point: ActivityRoutePoint) {
  if (metric === "speed") {
    return point.speed_mps;
  }

  if (metric === "heartRate") {
    return point.heart_rate_bpm;
  }

  return point.elevation_meters;
}

function buildChartSeries(metric: ChartMetric, effort: SegmentEffort) {
  return (effort.route_points ?? [])
    .map((point) => {
      const value = getMetricValue(metric, point);

      return value == null
        ? null
        : {
            x: point.elapsed_seconds,
            y: value,
          };
    })
    .filter((point): point is { x: number; y: number } => point !== null);
}

function buildElevationBackdropSeries(
  routePoints: ActivityRoutePoint[] | null | undefined,
  maxX: number,
) {
  if (!routePoints || routePoints.length < 2 || maxX <= 0) {
    return [] as Array<{ x: number; y: number }>;
  }

  const firstDistance = routePoints[0]?.distance_meters;
  const lastDistance = routePoints.at(-1)?.distance_meters;
  const hasDistanceRange =
    typeof firstDistance === "number" &&
    typeof lastDistance === "number" &&
    lastDistance > firstDistance &&
    routePoints.every((point) => typeof point.distance_meters === "number");

  return routePoints
    .map((point, index) => {
      if (point.elevation_meters == null) {
        return null;
      }

      const progress = hasDistanceRange
        ? ((point.distance_meters as number) - firstDistance) /
          (lastDistance - firstDistance)
        : index / Math.max(routePoints.length - 1, 1);

      return {
        x: progress * maxX,
        y: point.elevation_meters,
      };
    })
    .filter((point): point is { x: number; y: number } => point !== null);
}

function metricLabel(metric: ChartMetric) {
  return metric === "heartRate"
    ? "Heart rate"
    : metric === "speed"
      ? "Speed"
      : "Elevation";
}

function effortSeriesDataKey(effortId: number) {
  return `effort_${effortId}`;
}

function interpolateComparisonValue(
  points: Array<{ x: number; y: number }>,
  elapsedSeconds: number,
  clampOutsideRange = false,
) {
  if (points.length === 0) {
    return null;
  }

  if (elapsedSeconds < points[0].x) {
    return clampOutsideRange ? points[0].y : null;
  }

  if (elapsedSeconds > points[points.length - 1].x) {
    return clampOutsideRange ? points[points.length - 1].y : null;
  }

  if (elapsedSeconds === points[0].x) {
    return points[0].y;
  }

  for (let index = 1; index < points.length; index += 1) {
    const previous = points[index - 1];
    const current = points[index];

    if (elapsedSeconds === current.x) {
      return current.y;
    }

    if (elapsedSeconds < current.x) {
      const span = Math.max(current.x - previous.x, Number.EPSILON);
      const progress = (elapsedSeconds - previous.x) / span;

      return previous.y + (current.y - previous.y) * progress;
    }
  }

  return points.at(-1)?.y ?? null;
}

function buildComparisonChartRowAtX(
  elapsedSeconds: number,
  series: ComparisonSeries[],
  elevationPoints: Array<{ x: number; y: number }>,
  clampOutsideRange = false,
) {
  const row: ComparisonChartRow = {
    elapsedSeconds,
    elevation: interpolateComparisonValue(
      elevationPoints,
      elapsedSeconds,
      clampOutsideRange,
    ),
  };

  for (const entry of series) {
    row[entry.dataKey] = interpolateComparisonValue(
      entry.points,
      elapsedSeconds,
      clampOutsideRange,
    );
  }

  return row;
}

function buildComparisonChartRows(
  series: ComparisonSeries[],
  elevationPoints: Array<{ x: number; y: number }>,
) {
  const xValues = new Set<number>();

  for (const point of elevationPoints) {
    xValues.add(point.x);
  }

  for (const entry of series) {
    for (const point of entry.points) {
      xValues.add(point.x);
    }
  }

  return Array.from(xValues)
    .sort((left, right) => left - right)
    .map((elapsedSeconds) =>
      buildComparisonChartRowAtX(elapsedSeconds, series, elevationPoints),
    );
}

function ComparisonChartTooltip({
  active,
  label,
  payload,
  metric,
  series,
  unitSystem,
}: {
  active?: boolean;
  label?: number;
  payload?: Array<{
    color?: string;
    dataKey?: string;
    value?: number | null;
  }>;
  metric: ChartMetric;
  series: ComparisonSeries[];
  unitSystem: UnitSystem;
}) {
  if (!active || typeof label !== "number") {
    return null;
  }

  const elevationValue =
    payload?.find((entry) => entry.dataKey === "elevation")?.value ?? null;

  return (
    <div className="rounded-box border border-base-300 bg-base-100 px-3 py-3 shadow-lg">
      <p className="text-sm font-semibold text-base-content">
        {formatDuration(Math.round(label))}
      </p>
      <p className="mt-1 text-sm text-base-content/70">
        {`Elevation ${formatElevation(elevationValue, unitSystem)}`}
      </p>
      <div className="mt-2 space-y-1.5 text-sm text-base-content/75">
        {series.map((entry) => {
          const value =
            payload?.find((item) => item.dataKey === entry.dataKey)?.value ??
            null;

          return (
            <div
              key={entry.effort.id}
              className="rounded-box border border-base-300 bg-base-200/70 px-2 py-2"
              style={{ borderLeftColor: entry.color, borderLeftWidth: 4 }}
            >
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <span
                      aria-hidden
                      className="inline-block h-2.5 w-2.5 rounded-full"
                      style={{ backgroundColor: entry.color }}
                    />
                    <span className="font-medium text-base-content">
                      {entry.effort.rider_name}
                    </span>
                  </div>
                  <div className="truncate pl-4 text-xs text-base-content/65">
                    {entry.effort.activity_title}
                  </div>
                </div>
                <span className="whitespace-nowrap">
                  {formatMetricValue(metric, value, unitSystem)}
                </span>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function RouteComparisonMap({
  routePoints,
  selectedEfforts,
  playbackSeconds,
  maxDuration,
  isPlaying,
  focusedEffortId,
  onTogglePlayback,
  onSeek,
}: {
  routePoints: ActivityRoutePoint[] | null | undefined;
  selectedEfforts: SegmentEffort[];
  playbackSeconds: number;
  maxDuration: number;
  isPlaying: boolean;
  focusedEffortId: number | null;
  onTogglePlayback: () => void;
  onSeek: (value: number) => void;
}) {
  const hasRouteMap = (routePoints?.length ?? 0) >= 2;

  const markers = selectedEfforts
    .map((effort, index) => {
      const point = comparisonMarkerPoint(routePoints, effort, playbackSeconds);
      if (!point) {
        return null;
      }

      const isDimmed = focusedEffortId != null && effort.id !== focusedEffortId;

      return {
        id: effort.id,
        color: EFFORT_COLORS[index % EFFORT_COLORS.length],
        point,
        isDimmed,
      };
    })
    .filter(
      (
        marker,
      ): marker is {
        id: number;
        color: string;
        point: ActivityRoutePoint;
        isDimmed: boolean;
      } => marker !== null,
    );

  return (
    <div className="flex h-full flex-col">
      <div>
        <h3 className="text-sm font-semibold uppercase tracking-[0.16em] text-base-content/55">
          Route playback
        </h3>
        <p className="mt-1 text-sm text-base-content/70">
          Play the selected attempts against the same route to see where each
          ride is gaining or losing time.
        </p>
      </div>

      <div className="mt-4 min-h-[20rem] flex-1 overflow-hidden rounded-box border border-base-300 bg-base-200">
        {hasRouteMap ? (
          <LeafletRouteMap
            routePoints={routePoints}
            movingMarkers={markers.map((marker) => ({
              id: String(marker.id),
              point: marker.point,
              color: marker.color,
              opacity: marker.isDimmed ? 0.28 : 1,
            }))}
            ariaLabel="Segment comparison map"
            emptyMessage="Segment route geometry is not available yet."
            className="h-full min-h-[20rem] w-full"
          />
        ) : (
          <div className="flex h-full min-h-[20rem] items-center justify-center p-4">
            <div className="alert">
              Segment route geometry is not available yet.
            </div>
          </div>
        )}
      </div>

      <div className="mt-4 flex items-center gap-3">
        <button
          type="button"
          className="btn btn-outline btn-sm btn-square shrink-0"
          disabled={selectedEfforts.length === 0 || maxDuration <= 0}
          aria-label={
            isPlaying
              ? "Pause comparison playback"
              : playbackSeconds >= maxDuration
                ? "Replay comparison playback"
                : "Play comparison playback"
          }
          onClick={onTogglePlayback}
        >
          <FontAwesomeIcon
            icon={isPlaying ? faPause : faPlay}
            className="h-4 w-4"
          />
        </button>

        <input
          type="range"
          min={0}
          max={Math.max(maxDuration, 1)}
          step={1}
          value={Math.min(playbackSeconds, Math.max(maxDuration, 1))}
          className="range range-primary flex-1"
          disabled={selectedEfforts.length === 0 || maxDuration <= 0}
          aria-label="Playback timeline"
          onChange={(event) => {
            onSeek(Number(event.target.value));
          }}
        />
        <span className="badge badge-outline whitespace-nowrap">
          {formatDuration(Math.round(playbackSeconds))} /{" "}
          {formatDuration(maxDuration)}
        </span>
      </div>
    </div>
  );
}

function ComparisonChart({
  metric,
  routePoints,
  selectedEfforts,
  playbackSeconds,
  focusedEffortId,
  unitSystem,
  onMetricChange,
}: {
  metric: ChartMetric;
  routePoints: ActivityRoutePoint[] | null | undefined;
  selectedEfforts: SegmentEffort[];
  playbackSeconds: number;
  focusedEffortId: number | null;
  unitSystem: UnitSystem;
  onMetricChange: (value: ChartMetric) => void;
}) {
  const [hoveredRow, setHoveredRow] = useState<ComparisonChartRow | null>(null);
  const series = useMemo<ComparisonSeries[]>(
    () =>
      selectedEfforts.map((effort, index) => ({
        effort,
        color: EFFORT_COLORS[index % EFFORT_COLORS.length],
        dataKey: effortSeriesDataKey(effort.id),
        points: buildChartSeries(metric, effort),
      })),
    [metric, selectedEfforts],
  );
  const allPoints = series.flatMap((entry) => entry.points);
  const hasComparisonData = allPoints.length >= 2;
  const maxX = hasComparisonData
    ? Math.max(...allPoints.map((point) => point.x), 1)
    : 1;
  const elevationPoints = hasComparisonData
    ? buildElevationBackdropSeries(routePoints, maxX)
    : [];
  const chartRows = hasComparisonData
    ? buildComparisonChartRows(series, elevationPoints)
    : [];
  const playbackRow = hasComparisonData
    ? buildComparisonChartRowAtX(
        Math.min(playbackSeconds, maxX),
        series,
        elevationPoints,
        true,
      )
    : null;
  const displayRow = hoveredRow ?? playbackRow;
  const displaySeconds = displayRow?.elapsedSeconds ?? 0;

  return (
    <div className="flex h-full flex-col">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <h3 className="text-sm font-semibold uppercase tracking-[0.16em] text-base-content/55">
            Chart comparison
          </h3>
          <p className="mt-1 text-sm text-base-content/70">
            Compare the selected attempts across elapsed time while the map dots
            advance.
          </p>
        </div>
        <div className="join">
          {(["speed", "heartRate"] as ChartMetric[]).map((nextMetric) => (
            <button
              key={nextMetric}
              type="button"
              className={`join-item btn btn-sm ${metric === nextMetric ? "btn-primary" : "btn-ghost"}`}
              onClick={() => {
                onMetricChange(nextMetric);
              }}
            >
              {nextMetric === "heartRate" ? "Heart rate" : nextMetric}
            </button>
          ))}
        </div>
      </div>

      <div className="mt-4 flex min-h-[20rem] flex-1 overflow-hidden rounded-box border border-base-300 bg-base-200 p-3">
        {hasComparisonData ? (
          <div
            role="img"
            aria-label="Segment comparison chart"
            className="flex flex-1"
          >
            <div className="h-full min-h-[20rem] w-full">
              <ResponsiveContainer
                width="100%"
                height="100%"
                minWidth={320}
                minHeight={320}
              >
                <ComposedChart
                  data={chartRows}
                  margin={{ top: 12, right: 8, bottom: 12, left: 8 }}
                  onMouseLeave={() => {
                    setHoveredRow(null);
                  }}
                  onMouseMove={(state) => {
                    const nextIndex = Number(state?.activeTooltipIndex);

                    if (
                      state?.isTooltipActive &&
                      Number.isInteger(nextIndex) &&
                      nextIndex >= 0 &&
                      nextIndex < chartRows.length
                    ) {
                      setHoveredRow(chartRows[nextIndex]);
                    } else {
                      setHoveredRow(null);
                    }
                  }}
                >
                  <CartesianGrid
                    vertical={false}
                    stroke="var(--color-base-content)"
                    strokeOpacity={0.12}
                  />
                  <XAxis
                    axisLine={false}
                    dataKey="elapsedSeconds"
                    domain={[0, maxX]}
                    label={{
                      value: "Elapsed time",
                      position: "insideBottom",
                      offset: -6,
                    }}
                    tick={{ fill: "var(--color-base-content)", fontSize: 10 }}
                    tickFormatter={(value: number) =>
                      formatDuration(Math.round(value))
                    }
                    tickLine={false}
                    type="number"
                  />
                  <YAxis
                    axisLine={false}
                    label={{
                      angle: -90,
                      fill: "var(--color-base-content)",
                      fontSize: 10,
                      position: "insideLeft",
                      style: { opacity: 0.65 },
                      value: metricLabel(metric),
                    }}
                    tick={{ fill: "var(--color-base-content)", fontSize: 10 }}
                    tickFormatter={(value: number) =>
                      formatMetricValue(metric, value, unitSystem)
                    }
                    tickLine={false}
                    tickMargin={10}
                    width={74}
                    yAxisId="metric"
                  />
                  <YAxis
                    axisLine={false}
                    label={{
                      angle: 90,
                      fill: "var(--color-base-content)",
                      fontSize: 10,
                      position: "insideRight",
                      style: { opacity: 0.65 },
                      value: "Elevation",
                    }}
                    orientation="right"
                    tick={{ fill: "var(--color-base-content)", fontSize: 10 }}
                    tickFormatter={(value: number) =>
                      formatElevation(value, unitSystem)
                    }
                    tickLine={false}
                    tickMargin={10}
                    width={74}
                    yAxisId="elevation"
                  />
                  <Tooltip
                    content={
                      <ComparisonChartTooltip
                        metric={metric}
                        series={series}
                        unitSystem={unitSystem}
                      />
                    }
                    cursor={{
                      stroke: "#78716c",
                      strokeDasharray: "4 4",
                      strokeOpacity: 0.9,
                    }}
                  />

                  {elevationPoints.length > 1 ? (
                    <Area
                      type="linear"
                      dataKey="elevation"
                      yAxisId="elevation"
                      stroke="var(--color-success)"
                      fill="var(--color-success)"
                      fillOpacity={0.15}
                      strokeOpacity={0.35}
                      strokeWidth={2}
                      dot={false}
                      connectNulls
                    />
                  ) : null}

                  {series.map((entry) => (
                    <Line
                      key={entry.effort.id}
                      type="linear"
                      dataKey={entry.dataKey}
                      yAxisId="metric"
                      stroke={entry.color}
                      strokeWidth={metric === "speed" ? 2.5 : 3.5}
                      strokeOpacity={
                        focusedEffortId != null &&
                        focusedEffortId !== entry.effort.id
                          ? 0.24
                          : 1
                      }
                      dot={false}
                      activeDot={{
                        r: 6,
                        fill: entry.color,
                        fillOpacity:
                          focusedEffortId != null &&
                          focusedEffortId !== entry.effort.id
                            ? 0.24
                            : 1,
                        stroke: "var(--color-base-100)",
                        strokeOpacity:
                          focusedEffortId != null &&
                          focusedEffortId !== entry.effort.id
                            ? 0.4
                            : 1,
                        strokeWidth: 1.25,
                      }}
                      connectNulls
                    />
                  ))}

                  {!hoveredRow && displayRow && displaySeconds > 0 ? (
                    <ReferenceLine
                      x={displaySeconds}
                      stroke="#78716c"
                      strokeDasharray="4 4"
                    />
                  ) : null}

                  {!hoveredRow && displayRow
                    ? series.map((entry) => {
                        const value = displayRow[entry.dataKey];

                        if (typeof value !== "number") {
                          return null;
                        }

                        return (
                          <ReferenceDot
                            key={`${entry.effort.id}-marker`}
                            x={displaySeconds}
                            y={value}
                            fill={entry.color}
                            fillOpacity={
                              focusedEffortId != null &&
                              focusedEffortId !== entry.effort.id
                                ? 0.24
                                : 1
                            }
                            r={6}
                            stroke="var(--color-base-100)"
                            strokeOpacity={
                              focusedEffortId != null &&
                              focusedEffortId !== entry.effort.id
                                ? 0.4
                                : 1
                            }
                            strokeWidth={1.25}
                            yAxisId="metric"
                          />
                        );
                      })
                    : null}
                </ComposedChart>
              </ResponsiveContainer>
            </div>
          </div>
        ) : (
          <div className="flex flex-1 items-center justify-center">
            <div className="alert">
              The selected efforts do not have enough point-level data for a
              shared chart.
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function SelectedEffortsPanel({
  selectedRows,
  focusedEffortId,
  pinnedEffortId,
  onHoverEffort,
  onTogglePinnedEffort,
  onRemoveEffort,
}: {
  selectedRows: SelectedEffortRow[];
  focusedEffortId: number | null;
  pinnedEffortId: number | null;
  onHoverEffort: (effortId: number | null) => void;
  onTogglePinnedEffort: (effortId: number) => void;
  onRemoveEffort: (effortId: number) => void;
}) {
  return (
    <div className="rounded-box border border-base-300 bg-base-200 p-4">
      <div>
        <h3 className="text-sm font-semibold uppercase tracking-[0.16em] text-base-content/55">
          Selected rides
        </h3>
        <p className="mt-1 text-sm text-base-content/60">
          Hover or pin a ride here to focus the shared map and chart.
        </p>
      </div>

      {selectedRows.length > 0 ? (
        <div className="mt-4 space-y-2">
          {selectedRows.map(({ effort, color }) => {
            const isFocused = focusedEffortId === effort.id;
            const isPinned = pinnedEffortId === effort.id;

            return (
              <div
                key={effort.id}
                className={`flex items-center gap-3 rounded-box border border-base-300 bg-base-100 p-2 transition-opacity ${focusedEffortId != null && focusedEffortId !== effort.id ? "opacity-45" : "opacity-100"}`}
                style={{ borderLeftColor: color, borderLeftWidth: 4 }}
              >
                <button
                  type="button"
                  className={`min-w-0 flex-1 rounded-box px-2 py-1 text-left ${isFocused ? "bg-base-200/80" : "bg-transparent"}`}
                  aria-pressed={isPinned}
                  onMouseEnter={() => {
                    onHoverEffort(effort.id);
                  }}
                  onMouseLeave={() => {
                    onHoverEffort(null);
                  }}
                  onFocus={() => {
                    onHoverEffort(effort.id);
                  }}
                  onBlur={() => {
                    onHoverEffort(null);
                  }}
                  onClick={() => {
                    onTogglePinnedEffort(effort.id);
                  }}
                >
                  <div className="min-w-0 flex items-center gap-3">
                    <span
                      aria-hidden
                      className="inline-block h-2.5 w-2.5 shrink-0 rounded-full"
                      style={{ backgroundColor: color }}
                    />
                    <div className="min-w-0">
                      <div className="truncate font-medium text-base-content">
                        {effort.rider_name} ·{" "}
                        {formatDuration(effort.duration_seconds)}
                      </div>
                      <div className="truncate text-xs text-base-content/65">
                        {effort.activity_title} ·{" "}
                        {formatActivityTimestamp(effort.activity_started_at)}
                      </div>
                    </div>
                  </div>
                </button>

                <button
                  type="button"
                  className="btn btn-ghost btn-xs btn-circle shrink-0"
                  aria-label={`Remove ${effort.activity_title} from comparison`}
                  onClick={() => {
                    onRemoveEffort(effort.id);
                  }}
                >
                  <FontAwesomeIcon icon={faXmark} className="h-3.5 w-3.5" />
                </button>
              </div>
            );
          })}
        </div>
      ) : (
        <div className="alert mt-4 bg-base-100 text-sm text-base-content/70">
          <span>Add rides from the left column to start the comparison.</span>
        </div>
      )}
    </div>
  );
}

export default function SegmentDetailPanel({
  segmentId,
}: {
  segmentId: number | string;
}) {
  const authApi = auth.useAuthApi();
  const { user, isLoading: isLoadingUser } = authApi.useCurrentUser();
  const { unitSystem } = useUnitPreferences();
  const segmentQuery = useSegment(user ? segmentId : null);
  const [selectedEffortIds, setSelectedEffortIds] = useState<number[]>([]);
  const initializedSelectionSegmentIdRef = useRef<number | null>(null);
  const [playbackSeconds, setPlaybackSeconds] = useState(0);
  const [isPlaying, setIsPlaying] = useState(false);
  const [metric, setMetric] = useState<ChartMetric>("speed");
  const [hoveredEffortId, setHoveredEffortId] = useState<number | null>(null);
  const [pinnedEffortId, setPinnedEffortId] = useState<number | null>(null);
  const [effortTimeFilter, setEffortTimeFilter] =
    useState<EffortTimeFilter>("all");
  const [effortSearchQuery, setEffortSearchQuery] = useState("");
  const segment = segmentQuery.data;
  const allEfforts = segment?.efforts ?? [];
  const visibleEfforts = filterEffortsByTimeWindow(
    segment?.efforts,
    effortTimeFilter,
  );
  const filteredVisibleEfforts = useMemo(
    () => filterEffortsBySearchQuery(visibleEfforts, effortSearchQuery),
    [effortSearchQuery, visibleEfforts],
  );
  const currentUserId = user?.id ?? null;
  const currentUserName = user?.name?.trim() || null;
  const overallRankByEffortId = overallEffortRanks(allEfforts);
  const overallKom = fastestEffort(allEfforts);
  const currentUserPr =
    currentUserId != null
      ? fastestEffort(
          allEfforts.filter((effort) => effort.rider_user_id === currentUserId),
        )
      : currentUserName
        ? fastestEffort(
            allEfforts.filter(
              (effort) => effort.rider_name === currentUserName,
            ),
          )
        : allEfforts.length > 0 &&
            new Set(allEfforts.map((effort) => effort.rider_name)).size === 1
          ? fastestEffort(allEfforts)
          : null;
  const selectedEfforts = useMemo(() => {
    const effortById = new Map(allEfforts.map((effort) => [effort.id, effort]));

    return selectedEffortIds
      .map((id) => effortById.get(id))
      .filter((effort): effort is SegmentEffort => Boolean(effort));
  }, [allEfforts, selectedEffortIds]);
  const selectedRows = useMemo(
    () =>
      selectedEfforts.map((effort, index) => ({
        effort,
        color: EFFORT_COLORS[index % EFFORT_COLORS.length],
      })),
    [selectedEfforts],
  );
  const focusedEffortId = hoveredEffortId ?? pinnedEffortId;
  const maxDuration = selectedEfforts.reduce(
    (max, effort) => Math.max(max, effort.duration_seconds),
    0,
  );

  useEffect(() => {
    setEffortSearchQuery("");
  }, [segment?.id]);

  useEffect(() => {
    if (!segment || allEfforts.length === 0) {
      initializedSelectionSegmentIdRef.current = null;
      setSelectedEffortIds([]);
      setPlaybackSeconds(0);
      setIsPlaying(false);
      return;
    }

    const shouldSeedSelection =
      initializedSelectionSegmentIdRef.current !== segment.id;
    initializedSelectionSegmentIdRef.current = segment.id;

    setSelectedEffortIds((current) => {
      const valid = current.filter((id) =>
        allEfforts.some((effort) => effort.id === id),
      );
      if (valid.length > 0) {
        return valid.slice(0, MAX_SELECTED_EFFORTS);
      }

      if (!shouldSeedSelection) {
        return [];
      }

      return allEfforts
        .slice(0, Math.min(3, allEfforts.length))
        .map((effort) => effort.id);
    });
  }, [allEfforts, segment?.id]);

  useEffect(() => {
    if (
      pinnedEffortId != null &&
      !selectedEfforts.some((effort) => effort.id === pinnedEffortId)
    ) {
      setPinnedEffortId(null);
    }

    if (
      hoveredEffortId != null &&
      !selectedEfforts.some((effort) => effort.id === hoveredEffortId)
    ) {
      setHoveredEffortId(null);
    }
  }, [hoveredEffortId, pinnedEffortId, selectedEfforts]);

  function togglePinnedEffort(effortId: number) {
    setPinnedEffortId((current) => (current === effortId ? null : effortId));
  }

  function addEffortToComparison(effortId: number) {
    setSelectedEffortIds((current) => {
      if (
        current.includes(effortId) ||
        current.length >= MAX_SELECTED_EFFORTS
      ) {
        return current;
      }

      return [...current, effortId];
    });
  }

  function removeEffortFromComparison(effortId: number) {
    setSelectedEffortIds((current) => current.filter((id) => id !== effortId));
  }

  useEffect(() => {
    if (maxDuration <= 0) {
      setPlaybackSeconds(0);
      setIsPlaying(false);
      return;
    }

    setPlaybackSeconds((current) => Math.min(current, maxDuration));
  }, [maxDuration]);

  useEffect(() => {
    if (!isPlaying || maxDuration <= 0) {
      return undefined;
    }

    const interval = window.setInterval(() => {
      setPlaybackSeconds((current) => {
        const next = Math.min(current + 1, maxDuration);
        if (next >= maxDuration) {
          setIsPlaying(false);
        }
        return next;
      });
    }, 220);

    return () => {
      window.clearInterval(interval);
    };
  }, [isPlaying, maxDuration]);

  if (isLoadingUser || segmentQuery.isLoading) {
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
        eyebrow="Segment comparison"
        title="Sign in to compare segment efforts"
        description="Select up to ten attempts, then use time to open the full activity detail."
      />
    );
  }

  if (segmentQuery.isError || !segment) {
    return (
      <section className="card bg-base-100 shadow-xl">
        <div className="card-body">
          <div className="alert alert-error">
            {extractApiMessage(segmentQuery.error) || "Segment not found"}
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
              <p className="text-sm text-base-content/60">Segment comparison</p>
              <h1 className="mt-2 text-4xl font-semibold">{segment.title}</h1>
              <p className="mt-3 text-sm text-base-content/70">
                Imported {formatActivityTimestamp(segment.created_at)} from{" "}
                {segment.source}
              </p>
            </div>
            <div className="flex flex-wrap gap-2">
              {segment.format ? (
                <span className="badge badge-outline uppercase">
                  {segment.format}
                </span>
              ) : null}
              <span className="badge badge-ghost">
                {segment.effort_count} efforts
              </span>
            </div>
          </div>

          <div className="stats stats-vertical border border-base-300 bg-base-200 shadow lg:stats-horizontal">
            <div className="stat">
              <div className="stat-figure text-primary">
                <FontAwesomeIcon icon={faRoute} className="h-8 w-8" />
              </div>
              <div className="stat-title">Distance</div>
              <div className="stat-value text-xl">
                {formatDistance(segment.distance_meters, unitSystem)}
              </div>
              <div className="stat-desc">Saved route length</div>
            </div>
            <div className="stat">
              <div className="stat-figure text-warning">
                <FontAwesomeIcon icon={faCrown} className="h-8 w-8" />
              </div>
              <div className="stat-title flex items-center gap-1">
                <FontAwesomeIcon
                  icon={faCrown}
                  className="h-3.5 w-3.5 text-warning"
                />
                <span>Overall KOM</span>
              </div>
              <div className="stat-value text-xl">
                {formatDuration(overallKom?.duration_seconds ?? null)}
              </div>
              <div className="stat-desc">
                {overallKom
                  ? `${overallKom.rider_name} · ${overallKom.activity_title}`
                  : "No efforts yet"}
              </div>
            </div>
            <div className="stat">
              <div className="stat-figure text-primary">
                <FontAwesomeIcon icon={faMedal} className="h-8 w-8" />
              </div>
              <div className="stat-title">Your PR</div>
              <div className="stat-value text-xl">
                {formatDuration(currentUserPr?.duration_seconds ?? null)}
              </div>
              <div className="stat-desc">
                {currentUserPr ? currentUserPr.activity_title : "No PR yet"}
              </div>
            </div>
            <div className="stat">
              <div className="stat-figure text-info">
                <FontAwesomeIcon icon={faTrophy} className="h-8 w-8" />
              </div>
              <div className="stat-title">Attempts</div>
              <div className="stat-value text-xl">{segment.effort_count}</div>
              <div className="stat-desc">Stored matched efforts</div>
            </div>
            <div className="stat">
              <div className="stat-figure text-secondary">
                <FontAwesomeIcon icon={faFileLines} className="h-8 w-8" />
              </div>
              <div className="stat-title">Imported</div>
              <div className="stat-value text-xl">
                {segment.format ? segment.format.toUpperCase() : segment.source}
              </div>
              <div className="stat-desc">
                {segment.original_filename ?? "Route import details"}
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="card border border-base-300 bg-base-100 shadow-xl">
        <div className="card-body">
          <h2 className="card-title text-xl">Efforts</h2>
          <p className="text-sm text-base-content/70">
            Select up to ten attempts, then use time to open the full activity
            detail.
          </p>
          <div className="mt-5 grid gap-5 xl:grid-cols-[minmax(0,1.55fr)_minmax(20rem,0.95fr)]">
            <div className="min-w-0 rounded-box border border-base-300 bg-base-200 p-4">
              <div className="flex flex-col gap-3">
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <div className="text-sm text-base-content/70">
                    {filteredVisibleEfforts.length} of{" "}
                    {(segment.efforts ?? []).length} efforts
                  </div>
                  <div className="join">
                    {EFFORT_TIME_FILTERS.map((filter) => (
                      <button
                        key={filter.key}
                        type="button"
                        className={`join-item btn btn-sm ${effortTimeFilter === filter.key ? "btn-neutral" : "btn-ghost"}`}
                        onClick={() => {
                          setEffortTimeFilter(filter.key);
                        }}
                      >
                        {filter.label}
                      </button>
                    ))}
                  </div>
                </div>

                <label className="input input-bordered flex items-center gap-2 bg-base-100">
                  <FontAwesomeIcon
                    icon={faMagnifyingGlass}
                    className="h-4 w-4 text-base-content/50"
                  />
                  <input
                    type="search"
                    value={effortSearchQuery}
                    onChange={(event) => {
                      setEffortSearchQuery(event.target.value);
                    }}
                    className="grow"
                    placeholder="Search rides or riders"
                    aria-label="Search efforts"
                  />
                </label>

                {visibleEfforts.length > EFFORTS_VISIBLE_ROWS ? (
                  <div className="text-xs text-base-content/55">
                    Scroll to see more than {EFFORTS_VISIBLE_ROWS} efforts
                  </div>
                ) : null}
              </div>

              {filteredVisibleEfforts.length > 0 ? (
                <div
                  aria-label="Segment efforts table"
                  className="mt-5 overflow-x-auto overflow-y-auto rounded-box border border-base-300 bg-base-100"
                  style={{ maxHeight: `${EFFORTS_TABLE_MAX_HEIGHT_REM}rem` }}
                >
                  <table className="table table-pin-rows table-sm">
                    <thead>
                      <tr>
                        <th className="w-14">Place</th>
                        <th className="w-16">
                          <span className="sr-only">Compare</span>
                        </th>
                        <th>Time</th>
                        <th>Rider</th>
                        <th>Date</th>
                      </tr>
                    </thead>
                    <tbody>
                      {filteredVisibleEfforts.map((effort) => {
                        const checked = selectedEffortIds.includes(effort.id);
                        const overallRank =
                          overallRankByEffortId.get(effort.id) ?? null;
                        const isCurrentUserPr = currentUserPr?.id === effort.id;
                        const achievement = primarySegmentAchievement({
                          overallRank,
                          personalRank: isCurrentUserPr ? 1 : null,
                        });
                        const rowClassName =
                          achievement?.kind === "pr"
                            ? "bg-primary/10"
                            : achievement?.kind === "kom"
                              ? "bg-warning/10"
                              : checked
                                ? "bg-base-200/70"
                                : undefined;

                        return (
                          <tr key={effort.id} className={rowClassName}>
                            <td className="font-mono text-sm font-semibold tabular-nums text-base-content/70">
                              {overallRank ?? "--"}
                            </td>
                            <td>
                              {checked ? (
                                <button
                                  type="button"
                                  className="btn btn-ghost btn-xs btn-circle"
                                  aria-label={`Remove ${effort.activity_title} from comparison`}
                                  onClick={() => {
                                    removeEffortFromComparison(effort.id);
                                  }}
                                >
                                  <FontAwesomeIcon
                                    icon={faMinus}
                                    className="h-3.5 w-3.5"
                                  />
                                  <span className="sr-only">
                                    Remove from comparison
                                  </span>
                                </button>
                              ) : (
                                <button
                                  type="button"
                                  className="btn btn-ghost btn-xs btn-circle"
                                  disabled={
                                    selectedEffortIds.length >=
                                    MAX_SELECTED_EFFORTS
                                  }
                                  aria-label={`Add ${effort.activity_title} to comparison`}
                                  onClick={() => {
                                    addEffortToComparison(effort.id);
                                  }}
                                >
                                  <FontAwesomeIcon
                                    icon={faPlus}
                                    className="h-3.5 w-3.5"
                                  />
                                  <span className="sr-only">
                                    {selectedEffortIds.length >=
                                    MAX_SELECTED_EFFORTS
                                      ? "Comparison full"
                                      : "Add to comparison"}
                                  </span>
                                </button>
                              )}
                            </td>
                            <td className="font-semibold text-base-content">
                              <div className="flex flex-wrap items-center gap-2">
                                <Link
                                  href={`/activities/${effort.activity_id}`}
                                  className="transition hover:text-primary"
                                  title={effort.activity_title}
                                >
                                  {formatDuration(effort.duration_seconds)}
                                </Link>
                                {achievement?.kind === "kom" ? (
                                  <span className="badge badge-warning badge-xs gap-1">
                                    <FontAwesomeIcon
                                      icon={faCrown}
                                      className="h-3 w-3"
                                    />
                                    KOM
                                  </span>
                                ) : achievement?.kind === "top-10" ? (
                                  <span className="badge badge-warning badge-xs gap-1">
                                    <FontAwesomeIcon
                                      icon={faTrophy}
                                      className="h-3 w-3"
                                    />
                                    {achievement.longLabel}
                                  </span>
                                ) : achievement?.kind === "pr" ? (
                                  <span className="badge badge-primary badge-xs">
                                    PR
                                  </span>
                                ) : null}
                              </div>
                            </td>
                            <td>{effort.rider_name}</td>
                            <td className="whitespace-nowrap text-base-content/65">
                              {formatActivityTimestamp(
                                effort.activity_started_at,
                              )}
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>
              ) : (
                <div className="alert mt-5">
                  <span>
                    {effortSearchQuery.trim().length > 0
                      ? "No efforts match this search."
                      : "No efforts match this time window."}
                  </span>
                </div>
              )}
            </div>

            <SelectedEffortsPanel
              selectedRows={selectedRows}
              focusedEffortId={focusedEffortId}
              pinnedEffortId={pinnedEffortId}
              onHoverEffort={setHoveredEffortId}
              onTogglePinnedEffort={togglePinnedEffort}
              onRemoveEffort={removeEffortFromComparison}
            />
          </div>
        </div>
      </div>

      <div className="card bg-base-100 shadow-xl">
        <div className="card-body gap-6">
          <div>
            <h2 className="card-title text-xl">Comparison workspace</h2>
            <p className="text-sm text-base-content/70">
              The selected rides drive both the route playback and the shared
              chart, so each ride only needs to be identified once.
            </p>
          </div>

          <div className="grid items-stretch gap-6 xl:grid-cols-[minmax(0,1.02fr)_minmax(0,1fr)]">
            <div className="h-full">
              <RouteComparisonMap
                routePoints={segment.route_points}
                selectedEfforts={selectedEfforts}
                playbackSeconds={playbackSeconds}
                maxDuration={maxDuration}
                isPlaying={isPlaying}
                focusedEffortId={focusedEffortId}
                onTogglePlayback={() => {
                  if (playbackSeconds >= maxDuration) {
                    setPlaybackSeconds(0);
                  }
                  setIsPlaying((current) => !current);
                }}
                onSeek={(value) => {
                  setPlaybackSeconds(value);
                  setIsPlaying(false);
                }}
              />
            </div>

            <div className="h-full">
              <ComparisonChart
                metric={metric}
                routePoints={segment.route_points}
                selectedEfforts={selectedEfforts}
                playbackSeconds={playbackSeconds}
                focusedEffortId={focusedEffortId}
                unitSystem={unitSystem}
                onMetricChange={(value) => {
                  setMetric(value);
                }}
              />
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
