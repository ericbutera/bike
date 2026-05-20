"use client";

import { auth } from "@ericbutera/kaleido";
import {
  faCircleInfo,
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
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
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
import MapLibreRouteMap from "./MapLibreRouteMap";

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
const EFFORTS_VISIBLE_ROWS = 10;
const EFFORTS_TABLE_MAX_HEIGHT_REM = 31;
const EMPTY_EFFORTS: SegmentEffort[] = [];
const EMPTY_EFFORT_IDS: number[] = [];
const AUTO_PLAYBACK_MIN_SECONDS = 25;
const PLAYBACK_TARGET_MAX_SECONDS = 120;
const PLAYBACK_TARGET_MIN_SECONDS = 15;
const PLAYBACK_END_EPSILON = 0.0001;
const ATHLETE_PANEL_ROW_ANIMATION_MS = 220;
const PLAYBACK_PACE_OPTIONS = [
  { key: "detail", label: "Detail", multiplier: 1.5 },
  { key: "auto", label: "Auto", multiplier: 1 },
  { key: "fast", label: "Fast", multiplier: 0.65 },
] as const;

const EFFORT_TIME_FILTERS = [
  { key: "all", label: "All" },
  { key: "day", label: "Day" },
  { key: "week", label: "Week" },
  { key: "month", label: "Month" },
  { key: "year", label: "Year" },
] as const;

type ChartMetric = "speed" | "heartRate" | "elevation";
type EffortTimeFilter = (typeof EFFORT_TIME_FILTERS)[number]["key"];
type PlaybackPace = (typeof PLAYBACK_PACE_OPTIONS)[number]["key"];

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

function SectionTitleWithTooltip({
  as,
  title,
  tooltip,
  className,
}: {
  as: "h2" | "h3";
  title: string;
  tooltip: string;
  className: string;
}) {
  const HeadingTag = as;

  return (
    <div className="flex items-center gap-2">
      <HeadingTag className={className}>{title}</HeadingTag>
      <span
        className="inline-flex h-5 w-5 items-center justify-center text-base-content/45"
        title={tooltip}
        aria-label={tooltip}
      >
        <FontAwesomeIcon icon={faCircleInfo} className="h-3.5 w-3.5" />
      </span>
    </div>
  );
}

type SelectedEffortRow = {
  effort: SegmentEffort;
  color: string;
  markerLabel: string;
};

type LiveComparisonRow = SelectedEffortRow & {
  currentPoint: ActivityRoutePoint | null;
  gapSeconds: number | null;
  speedDeltaMps: number | null;
  isReference: boolean;
  progress: number | null;
  isFinished: boolean;
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

function areEffortIdListsEqual(left: number[], right: number[]) {
  if (left.length !== right.length) {
    return false;
  }

  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) {
      return false;
    }
  }

  return true;
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

function interpolateOptionalNumber(
  previousValue: number | null | undefined,
  currentValue: number | null | undefined,
  progress: number,
) {
  if (previousValue == null && currentValue == null) {
    return null;
  }

  if (previousValue == null) {
    return currentValue ?? null;
  }

  if (currentValue == null) {
    return previousValue;
  }

  return previousValue + (currentValue - previousValue) * progress;
}

function distanceRange(points: ActivityRoutePoint[] | null | undefined) {
  if (!points || points.length < 2) {
    return null;
  }

  const firstDistance = points[0]?.distance_meters;
  const lastDistance = points.at(-1)?.distance_meters;
  const hasDistanceRange =
    typeof firstDistance === "number" &&
    typeof lastDistance === "number" &&
    lastDistance > firstDistance &&
    points.every((point) => typeof point.distance_meters === "number");

  if (!hasDistanceRange) {
    return null;
  }

  return {
    firstDistance,
    lastDistance,
    totalDistance: lastDistance - firstDistance,
  };
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
        elapsed_seconds:
          previous.elapsed_seconds +
          (current.elapsed_seconds - previous.elapsed_seconds) * progress,
        latitude:
          previous.latitude + (current.latitude - previous.latitude) * progress,
        longitude:
          previous.longitude +
          (current.longitude - previous.longitude) * progress,
        distance_meters: interpolateOptionalNumber(
          previous.distance_meters,
          current.distance_meters,
          progress,
        ),
        elevation_meters: interpolateOptionalNumber(
          previous.elevation_meters,
          current.elevation_meters,
          progress,
        ),
        speed_mps: interpolateOptionalNumber(
          previous.speed_mps,
          current.speed_mps,
          progress,
        ),
        heart_rate_bpm: interpolateOptionalNumber(
          previous.heart_rate_bpm,
          current.heart_rate_bpm,
          progress,
        ),
        cadence_rpm: interpolateOptionalNumber(
          previous.cadence_rpm,
          current.cadence_rpm,
          progress,
        ),
        power_watts: interpolateOptionalNumber(
          previous.power_watts,
          current.power_watts,
          progress,
        ),
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

  const range = distanceRange(points);
  const firstDistance = range?.firstDistance;
  const lastDistance = range?.lastDistance;
  const hasDistanceRange = Boolean(range);
  const targetMeasure = hasDistanceRange
    ? (firstDistance as number) +
      clampedProgress * ((lastDistance as number) - (firstDistance as number))
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
        elapsed_seconds:
          previous.elapsed_seconds +
          (current.elapsed_seconds - previous.elapsed_seconds) * localProgress,
        latitude:
          previous.latitude +
          (current.latitude - previous.latitude) * localProgress,
        longitude:
          previous.longitude +
          (current.longitude - previous.longitude) * localProgress,
        distance_meters: hasDistanceRange
          ? targetMeasure - (firstDistance as number)
          : interpolateOptionalNumber(
              previous.distance_meters,
              current.distance_meters,
              localProgress,
            ),
        elevation_meters: interpolateOptionalNumber(
          previous.elevation_meters,
          current.elevation_meters,
          localProgress,
        ),
        speed_mps: interpolateOptionalNumber(
          previous.speed_mps,
          current.speed_mps,
          localProgress,
        ),
        heart_rate_bpm: interpolateOptionalNumber(
          previous.heart_rate_bpm,
          current.heart_rate_bpm,
          localProgress,
        ),
        cadence_rpm: interpolateOptionalNumber(
          previous.cadence_rpm,
          current.cadence_rpm,
          localProgress,
        ),
        power_watts: interpolateOptionalNumber(
          previous.power_watts,
          current.power_watts,
          localProgress,
        ),
      };
    }
  }

  return points.at(-1) ?? null;
}

function effortProgressAtElapsed(
  effort: SegmentEffort,
  elapsedSeconds: number,
) {
  const effortPoint = interpolateRoutePoint(
    effort.route_points,
    elapsedSeconds,
  );

  if (!effortPoint) {
    return null;
  }

  const effortRange = distanceRange(effort.route_points);

  if (effortRange && typeof effortPoint.distance_meters === "number") {
    return clampProgress(
      effortPoint.distance_meters / effortRange.totalDistance,
    );
  }

  return effort.duration_seconds > 0
    ? clampProgress(elapsedSeconds / effort.duration_seconds)
    : 0;
}

function comparisonMarkerPoint(
  segmentRoutePoints: ActivityRoutePoint[] | null | undefined,
  effort: SegmentEffort,
  playbackSeconds: number,
) {
  const progress = effortProgressAtElapsed(effort, playbackSeconds);

  if (progress == null) {
    return null;
  }

  return interpolateRoutePointByProgress(segmentRoutePoints, progress);
}

function resolveRouteDistanceMeters(
  routePoints: ActivityRoutePoint[] | null | undefined,
  fallbackDistanceMeters: number | null | undefined,
) {
  return (
    distanceRange(routePoints)?.totalDistance ?? fallbackDistanceMeters ?? null
  );
}

function resolveRouteNetElevationMeters(
  routePoints: ActivityRoutePoint[] | null | undefined,
) {
  if (!routePoints || routePoints.length < 2) {
    return null;
  }

  const start = routePoints[0]?.elevation_meters;
  const end = routePoints.at(-1)?.elevation_meters;

  if (start == null || end == null) {
    return null;
  }

  return end - start;
}

function formatGradePercent(value: number | null | undefined) {
  if (value == null || Number.isNaN(value)) {
    return "-- grade";
  }

  const rounded =
    Math.abs(value) >= 10 ? Math.round(value) : Number(value.toFixed(1));

  return `${rounded > 0 ? "+" : ""}${rounded}% grade`;
}

function formatSignedSecondsDelta(value: number | null | undefined) {
  if (value == null || Number.isNaN(value)) {
    return "--";
  }

  const rounded = Math.round(value);

  if (rounded === 0) {
    return "0s";
  }

  return `${rounded > 0 ? "+" : ""}${rounded}s`;
}

function formatSignedSpeedDelta(
  value: number | null | undefined,
  unitSystem: UnitSystem,
) {
  if (value == null || Number.isNaN(value)) {
    return "--";
  }

  const rounded = Math.abs(value) < 0.01 ? 0 : value;
  const sign = rounded > 0 ? "+" : rounded < 0 ? "-" : "";

  return `${sign}${formatSpeed(Math.abs(rounded), unitSystem)}`;
}

function routeDistanceAtProgress(
  routePoints: ActivityRoutePoint[] | null | undefined,
  progress: number,
  fallbackDistanceMeters: number | null | undefined,
) {
  const point = interpolateRoutePointByProgress(routePoints, progress);

  if (typeof point?.distance_meters === "number") {
    return point.distance_meters;
  }

  return fallbackDistanceMeters != null
    ? clampProgress(progress) * fallbackDistanceMeters
    : progress;
}

function playbackSecondsForEffort(
  effort: SegmentEffort,
  playbackSeconds: number,
) {
  return Math.min(playbackSeconds, effort.duration_seconds);
}

function autoPlaybackTargetSeconds(durationSeconds: number) {
  if (durationSeconds <= 0) {
    return 0;
  }

  if (durationSeconds <= AUTO_PLAYBACK_MIN_SECONDS) {
    return durationSeconds;
  }

  if (durationSeconds <= 10 * 60) {
    return AUTO_PLAYBACK_MIN_SECONDS;
  }

  if (durationSeconds <= 30 * 60) {
    return 35;
  }

  if (durationSeconds <= 60 * 60) {
    return 45;
  }

  return 60;
}

function playbackTargetSeconds(
  durationSeconds: number,
  playbackPace: PlaybackPace,
) {
  const option = PLAYBACK_PACE_OPTIONS.find(
    (entry) => entry.key === playbackPace,
  );
  const multiplier = option?.multiplier ?? 1;
  const autoTarget = autoPlaybackTargetSeconds(durationSeconds);

  if (autoTarget <= 0) {
    return 0;
  }

  return Math.min(
    Math.max(
      autoTarget * multiplier,
      Math.min(
        durationSeconds,
        Math.max(AUTO_PLAYBACK_MIN_SECONDS, PLAYBACK_TARGET_MIN_SECONDS),
      ),
    ),
    PLAYBACK_TARGET_MAX_SECONDS,
  );
}

function isEffortFinishedAtPlayback(
  effort: SegmentEffort,
  playbackSeconds: number,
) {
  return playbackSeconds > effort.duration_seconds + PLAYBACK_END_EPSILON;
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

type GapChartRow = {
  progress: number;
  distanceMeters: number;
  elevation?: number | null;
  [key: string]: number | null | undefined;
};

function buildGapChartRowAtProgress(
  progress: number,
  routePoints: ActivityRoutePoint[] | null | undefined,
  selectedRows: SelectedEffortRow[],
  referenceEffort: SegmentEffort | null,
  fallbackDistanceMeters: number | null | undefined,
) {
  if (!referenceEffort) {
    return null;
  }

  const referencePoint = interpolateRoutePointByProgress(
    referenceEffort.route_points,
    progress,
  );

  if (!referencePoint) {
    return null;
  }

  const routePoint = interpolateRoutePointByProgress(routePoints, progress);
  const row: GapChartRow = {
    progress,
    distanceMeters: routeDistanceAtProgress(
      routePoints,
      progress,
      fallbackDistanceMeters,
    ),
    elevation: routePoint?.elevation_meters ?? null,
  };

  for (const selectedRow of selectedRows) {
    const comparisonPoint = interpolateRoutePointByProgress(
      selectedRow.effort.route_points,
      progress,
    );

    row[effortSeriesDataKey(selectedRow.effort.id)] = comparisonPoint
      ? referencePoint.elapsed_seconds - comparisonPoint.elapsed_seconds
      : null;
  }

  return row;
}

function buildGapChartRows(
  routePoints: ActivityRoutePoint[] | null | undefined,
  selectedRows: SelectedEffortRow[],
  referenceEffort: SegmentEffort | null,
  fallbackDistanceMeters: number | null | undefined,
) {
  if (!routePoints || routePoints.length < 2 || !referenceEffort) {
    return [] as GapChartRow[];
  }

  const range = distanceRange(routePoints);

  return routePoints
    .map((point, index) => {
      const progress = range
        ? ((point.distance_meters as number) - range.firstDistance) /
          range.totalDistance
        : index / Math.max(routePoints.length - 1, 1);

      return buildGapChartRowAtProgress(
        progress,
        routePoints,
        selectedRows,
        referenceEffort,
        fallbackDistanceMeters,
      );
    })
    .filter((row): row is GapChartRow => row !== null);
}

function buildPlaybackGapMarker(
  selectedRow: SelectedEffortRow,
  routePoints: ActivityRoutePoint[] | null | undefined,
  selectedRows: SelectedEffortRow[],
  referenceEffort: SegmentEffort | null,
  fallbackDistanceMeters: number | null | undefined,
  playbackSeconds: number,
) {
  if (!referenceEffort) {
    return null;
  }

  const progress = effortProgressAtElapsed(
    selectedRow.effort,
    playbackSecondsForEffort(selectedRow.effort, playbackSeconds),
  );

  if (progress == null) {
    return null;
  }

  const row = buildGapChartRowAtProgress(
    progress,
    routePoints,
    selectedRows,
    referenceEffort,
    fallbackDistanceMeters,
  );

  if (!row) {
    return null;
  }

  const value = row[effortSeriesDataKey(selectedRow.effort.id)];

  if (typeof value !== "number") {
    return null;
  }

  return {
    distanceMeters: row.distanceMeters,
    value,
    isFinished: isEffortFinishedAtPlayback(selectedRow.effort, playbackSeconds),
  };
}

function buildLiveComparisonRows(
  selectedRows: SelectedEffortRow[],
  referenceEffort: SegmentEffort | null,
  playbackSeconds: number,
) {
  if (!referenceEffort) {
    return [] as LiveComparisonRow[];
  }

  const referenceCurrentPoint = interpolateRoutePoint(
    referenceEffort.route_points,
    playbackSeconds,
  );
  const referenceProgress = effortProgressAtElapsed(
    referenceEffort,
    playbackSeconds,
  );
  const referenceProgressPoint =
    referenceProgress != null
      ? interpolateRoutePointByProgress(
          referenceEffort.route_points,
          referenceProgress,
        )
      : null;

  return [...selectedRows].map((selectedRow) => {
    const activePlaybackSeconds = playbackSecondsForEffort(
      selectedRow.effort,
      playbackSeconds,
    );
    const progress = effortProgressAtElapsed(
      selectedRow.effort,
      activePlaybackSeconds,
    );
    const isFinished = isEffortFinishedAtPlayback(
      selectedRow.effort,
      playbackSeconds,
    );
    const currentPoint = isFinished
      ? null
      : interpolateRoutePoint(
          selectedRow.effort.route_points,
          activePlaybackSeconds,
        );
    const progressPoint =
      referenceProgress != null
        ? interpolateRoutePointByProgress(
            selectedRow.effort.route_points,
            referenceProgress,
          )
        : null;

    return {
      ...selectedRow,
      currentPoint,
      gapSeconds:
        referenceProgressPoint && progressPoint
          ? referenceProgressPoint.elapsed_seconds -
            progressPoint.elapsed_seconds
          : null,
      speedDeltaMps:
        !isFinished &&
        currentPoint?.speed_mps != null &&
        referenceCurrentPoint?.speed_mps != null
          ? currentPoint.speed_mps - referenceCurrentPoint.speed_mps
          : null,
      isReference: selectedRow.effort.id === referenceEffort.id,
      progress,
      isFinished,
    };
  });
}

function ComparisonGapChartTooltip({
  active,
  label,
  payload,
  selectedRows,
  unitSystem,
}: {
  active?: boolean;
  label?: number;
  payload?: Array<{
    color?: string;
    dataKey?: string;
    value?: number | string | null;
  }>;
  selectedRows: SelectedEffortRow[];
  unitSystem: UnitSystem;
}) {
  if (!active || typeof label !== "number") {
    return null;
  }

  const elevationValue = payload?.find(
    (entry) => entry.dataKey === "elevation",
  )?.value;

  return (
    <div className="rounded-box border border-base-300 bg-base-100 px-3 py-3 shadow-lg">
      <p className="text-sm font-semibold text-base-content">
        {formatDistance(label, unitSystem)}
      </p>
      <p className="mt-1 text-sm text-base-content/70">
        Elevation {formatElevation(Number(elevationValue ?? null), unitSystem)}
      </p>
      <div className="mt-2 space-y-1.5 text-sm text-base-content/75">
        {selectedRows.map((selectedRow) => {
          const value = payload?.find(
            (entry) =>
              entry.dataKey === effortSeriesDataKey(selectedRow.effort.id),
          )?.value;

          return (
            <div
              key={selectedRow.effort.id}
              className="rounded-box border border-base-300 bg-base-200/70 px-2 py-2"
              style={{ borderLeftColor: selectedRow.color, borderLeftWidth: 4 }}
            >
              <div className="flex items-center justify-between gap-3">
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <span
                      aria-hidden
                      className="inline-flex h-5 w-5 items-center justify-center rounded-full text-[0.65rem] font-semibold text-white"
                      style={{ backgroundColor: selectedRow.color }}
                    >
                      {selectedRow.markerLabel}
                    </span>
                    <span className="truncate font-medium text-base-content">
                      {selectedRow.effort.rider_name}
                    </span>
                  </div>
                  <div className="truncate pl-7 text-xs text-base-content/65">
                    {selectedRow.effort.activity_title}
                  </div>
                </div>
                <span className="whitespace-nowrap font-medium text-base-content">
                  {formatSignedSecondsDelta(
                    typeof value === "number" ? value : null,
                  )}
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
  selectedRows,
  playbackSeconds,
}: {
  routePoints: ActivityRoutePoint[] | null | undefined;
  selectedRows: SelectedEffortRow[];
  playbackSeconds: number;
}) {
  const hasRouteMap = (routePoints?.length ?? 0) >= 2;

  const markers = selectedRows
    .map((selectedRow) => {
      const point = comparisonMarkerPoint(
        routePoints,
        selectedRow.effort,
        playbackSeconds,
      );

      if (!point) {
        return null;
      }

      return {
        id: selectedRow.effort.id,
        color: selectedRow.color,
        point,
        label: selectedRow.markerLabel,
      };
    })
    .filter(
      (
        marker,
      ): marker is {
        id: number;
        color: string;
        point: ActivityRoutePoint;
        label: string;
      } => marker !== null,
    );

  return hasRouteMap ? (
    <MapLibreRouteMap
      routePoints={routePoints}
      movingMarkers={markers.map((marker) => ({
        id: String(marker.id),
        point: marker.point,
        color: marker.color,
        opacity: 1,
        label: marker.label,
      }))}
      ariaLabel="Segment comparison map"
      emptyMessage="Segment route geometry is not available yet."
      fitBoundsPadding={40}
      fitBoundsMaxZoom={18}
      className="h-full min-h-[24rem] w-full rounded-none border-0"
    />
  ) : (
    <div className="flex h-full min-h-[24rem] items-center justify-center p-4">
      <div className="alert">Segment route geometry is not available yet.</div>
    </div>
  );
}

function ComparisonChart({
  routePoints,
  routeDistanceMeters,
  selectedRows,
  referenceEffortId,
  playbackSeconds,
  unitSystem,
}: {
  routePoints: ActivityRoutePoint[] | null | undefined;
  routeDistanceMeters: number | null | undefined;
  selectedRows: SelectedEffortRow[];
  referenceEffortId: number | null;
  playbackSeconds: number;
  unitSystem: UnitSystem;
}) {
  const [hoveredRow, setHoveredRow] = useState<GapChartRow | null>(null);
  const referenceEffort =
    selectedRows.find(
      (selectedRow) => selectedRow.effort.id === referenceEffortId,
    )?.effort ?? null;
  const chartRows = useMemo(
    () =>
      buildGapChartRows(
        routePoints,
        selectedRows,
        referenceEffort,
        routeDistanceMeters,
      ),
    [referenceEffort, routeDistanceMeters, routePoints, selectedRows],
  );
  const maxDistance =
    chartRows.at(-1)?.distanceMeters ?? routeDistanceMeters ?? 1;
  const playbackProgress = referenceEffort
    ? effortProgressAtElapsed(referenceEffort, playbackSeconds)
    : null;
  const playbackRow =
    referenceEffort && playbackProgress != null
      ? buildGapChartRowAtProgress(
          playbackProgress,
          routePoints,
          selectedRows,
          referenceEffort,
          routeDistanceMeters,
        )
      : null;
  const displayRow = hoveredRow ?? playbackRow;
  const displayDistance = displayRow?.distanceMeters ?? 0;

  return chartRows.length >= 2 ? (
    <div
      role="img"
      aria-label="Segment comparison chart"
      className="h-[18rem] p-3"
    >
      <ResponsiveContainer
        width="100%"
        height="100%"
        minWidth={320}
        minHeight={240}
      >
        <ComposedChart
          data={chartRows}
          margin={{ top: 12, right: 12, bottom: 4, left: 8 }}
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
            strokeOpacity={0.1}
          />
          <XAxis
            axisLine={false}
            dataKey="distanceMeters"
            domain={[0, maxDistance]}
            tick={{ fill: "var(--color-base-content)", fontSize: 10 }}
            tickFormatter={(value: number) => formatDistance(value, unitSystem)}
            tickLine={false}
            type="number"
          />
          <YAxis
            axisLine={false}
            tick={{ fill: "var(--color-base-content)", fontSize: 10 }}
            tickFormatter={(value: number) =>
              formatElevation(value, unitSystem)
            }
            tickLine={false}
            tickMargin={10}
            width={68}
            yAxisId="elevation"
          />
          <YAxis
            axisLine={false}
            orientation="right"
            tick={{ fill: "var(--color-base-content)", fontSize: 10 }}
            tickFormatter={(value: number) => formatSignedSecondsDelta(value)}
            tickLine={false}
            tickMargin={10}
            width={72}
            yAxisId="gap"
          />
          <Tooltip
            content={
              <ComparisonGapChartTooltip
                selectedRows={selectedRows}
                unitSystem={unitSystem}
              />
            }
            cursor={{
              stroke: "#71717a",
              strokeDasharray: "4 4",
              strokeOpacity: 0.95,
            }}
          />

          <Area
            type="linear"
            dataKey="elevation"
            yAxisId="elevation"
            stroke="#9ca3af"
            fill="#d1d5db"
            fillOpacity={0.45}
            strokeOpacity={0.7}
            strokeWidth={1.5}
            dot={false}
            connectNulls
          />
          <ReferenceLine
            y={0}
            yAxisId="gap"
            stroke="#52525b"
            strokeOpacity={0.8}
          />

          {selectedRows.map((selectedRow) => {
            const isReference = selectedRow.effort.id === referenceEffortId;

            return (
              <Line
                key={selectedRow.effort.id}
                type="linear"
                dataKey={effortSeriesDataKey(selectedRow.effort.id)}
                yAxisId="gap"
                stroke={selectedRow.color}
                strokeWidth={isReference ? 3.2 : 2.4}
                strokeOpacity={1}
                dot={false}
                activeDot={false}
                connectNulls
              />
            );
          })}

          {!hoveredRow && displayRow && displayDistance > 0 ? (
            <ReferenceLine
              x={displayDistance}
              stroke="#52525b"
              strokeDasharray="4 4"
            />
          ) : null}

          {!hoveredRow && displayRow
            ? selectedRows.map((selectedRow) => {
                const playbackMarker = buildPlaybackGapMarker(
                  selectedRow,
                  routePoints,
                  selectedRows,
                  referenceEffort,
                  routeDistanceMeters,
                  playbackSeconds,
                );

                if (!playbackMarker) {
                  return null;
                }

                return (
                  <ReferenceDot
                    key={`${selectedRow.effort.id}-marker`}
                    x={playbackMarker.distanceMeters}
                    y={playbackMarker.value}
                    fill={selectedRow.color}
                    fillOpacity={1}
                    r={playbackMarker.isFinished ? 4.75 : 5.5}
                    stroke="var(--color-base-100)"
                    strokeOpacity={1}
                    strokeWidth={1.2}
                    yAxisId="gap"
                  />
                );
              })
            : null}
        </ComposedChart>
      </ResponsiveContainer>
    </div>
  ) : (
    <div className="flex h-[18rem] items-center justify-center p-4">
      <div className="alert">
        The selected efforts do not have enough point-level data for a shared
        chart.
      </div>
    </div>
  );
}

function SelectedEffortsPanel({
  comparisonRows,
  focusedEffortId,
  referenceEffortId,
  playbackSeconds,
  unitSystem,
  onHoverEffort,
  onTogglePinnedEffort,
  onRemoveEffort,
}: {
  comparisonRows: LiveComparisonRow[];
  focusedEffortId: number | null;
  referenceEffortId: number | null;
  playbackSeconds: number;
  unitSystem: UnitSystem;
  onHoverEffort: (effortId: number | null) => void;
  onTogglePinnedEffort: (effortId: number) => void;
  onRemoveEffort: (effortId: number) => void;
}) {
  const rowRefs = useRef(new Map<number, HTMLDivElement>());
  const previousRowTopByEffortIdRef = useRef(new Map<number, number>());
  const animationFrameRef = useRef<number | null>(null);
  const gridTemplateColumns = "minmax(0,1fr) 5.25rem 6.5rem 4.5rem 1.25rem";
  const sortedComparisonRows = useMemo(() => {
    const fallbackIndexByEffortId = new Map(
      comparisonRows.map((comparisonRow, index) => [
        comparisonRow.effort.id,
        index,
      ]),
    );

    return [...comparisonRows].sort((left, right) => {
      const progressDelta = (right.progress ?? -1) - (left.progress ?? -1);

      if (Math.abs(progressDelta) > Number.EPSILON) {
        return progressDelta;
      }

      const gapDelta = (right.gapSeconds ?? 0) - (left.gapSeconds ?? 0);

      if (Math.abs(gapDelta) > Number.EPSILON) {
        return gapDelta;
      }

      return (
        (fallbackIndexByEffortId.get(left.effort.id) ?? 0) -
        (fallbackIndexByEffortId.get(right.effort.id) ?? 0)
      );
    });
  }, [comparisonRows]);
  const sortedComparisonRowOrder = useMemo(
    () =>
      sortedComparisonRows
        .map((comparisonRow) => comparisonRow.effort.id)
        .join(","),
    [sortedComparisonRows],
  );

  useLayoutEffect(() => {
    if (animationFrameRef.current != null) {
      window.cancelAnimationFrame(animationFrameRef.current);
      animationFrameRef.current = null;
    }

    const nextRowTopByEffortId = new Map<number, number>();

    for (const comparisonRow of sortedComparisonRows) {
      const row = rowRefs.current.get(comparisonRow.effort.id);

      if (!row) {
        continue;
      }

      const nextTop = row.getBoundingClientRect().top;
      nextRowTopByEffortId.set(comparisonRow.effort.id, nextTop);

      const previousTop = previousRowTopByEffortIdRef.current.get(
        comparisonRow.effort.id,
      );

      if (previousTop == null) {
        continue;
      }

      const deltaY = previousTop - nextTop;

      if (Math.abs(deltaY) < 1) {
        continue;
      }

      row.style.transition = "none";
      row.style.transform = `translateY(${deltaY}px)`;
    }

    animationFrameRef.current = window.requestAnimationFrame(() => {
      for (const comparisonRow of sortedComparisonRows) {
        const row = rowRefs.current.get(comparisonRow.effort.id);

        if (!row || !row.style.transform) {
          continue;
        }

        row.style.transition = `transform ${ATHLETE_PANEL_ROW_ANIMATION_MS}ms cubic-bezier(0.22, 1, 0.36, 1)`;
        row.style.transform = "translateY(0)";
      }

      animationFrameRef.current = null;
    });

    previousRowTopByEffortIdRef.current = nextRowTopByEffortId;

    return () => {
      if (animationFrameRef.current != null) {
        window.cancelAnimationFrame(animationFrameRef.current);
        animationFrameRef.current = null;
      }
    };
  }, [sortedComparisonRowOrder]);

  return sortedComparisonRows.length > 0 ? (
    <div className="flex h-full min-h-[24rem] flex-col bg-base-100">
      <div className="flex-1 overflow-hidden">
        <div
          className="h-full overflow-y-auto overflow-x-hidden"
          style={{ scrollbarGutter: "stable" }}
        >
          <div className="sticky top-0 z-10 border-b border-base-300 bg-base-100 px-4 py-3">
            <div
              className="grid items-center gap-4 text-xs font-semibold uppercase tracking-[0.14em] text-base-content/55"
              style={{ gridTemplateColumns }}
            >
              <span>Athletes</span>
              <span className="justify-self-end text-right">Time</span>
              <span className="justify-self-end text-right">Speed</span>
              <span className="justify-self-end text-right">HR</span>
              <span aria-hidden="true" className="block" />
            </div>
          </div>

          {sortedComparisonRows.map((comparisonRow) => {
            const isFocused = focusedEffortId === comparisonRow.effort.id;
            const speedValue = comparisonRow.isReference
              ? formatSpeed(
                  comparisonRow.currentPoint?.speed_mps ?? null,
                  unitSystem,
                )
              : formatSignedSpeedDelta(comparisonRow.speedDeltaMps, unitSystem);
            const heartRateValue = comparisonRow.isFinished
              ? "--"
              : formatHeartRate(
                  comparisonRow.currentPoint?.heart_rate_bpm ?? null,
                );
            const timeValue = comparisonRow.isReference
              ? formatDuration(
                  Math.round(
                    Math.min(
                      playbackSeconds,
                      comparisonRow.effort.duration_seconds,
                    ),
                  ),
                )
              : formatSignedSecondsDelta(comparisonRow.gapSeconds);
            const isPositiveGap = (comparisonRow.gapSeconds ?? 0) > 0;
            const isNegativeGap = (comparisonRow.gapSeconds ?? 0) < 0;
            const isPositiveSpeed = (comparisonRow.speedDeltaMps ?? 0) > 0;
            const isNegativeSpeed = (comparisonRow.speedDeltaMps ?? 0) < 0;

            return (
              <div
                key={comparisonRow.effort.id}
                ref={(element) => {
                  if (element) {
                    rowRefs.current.set(comparisonRow.effort.id, element);
                  } else {
                    rowRefs.current.delete(comparisonRow.effort.id);
                  }
                }}
                className={`grid min-w-0 items-center gap-4 border-b border-base-300 px-4 py-3 transition-colors ${isFocused ? "bg-base-200/80" : "bg-transparent"}`}
                style={{ gridTemplateColumns, willChange: "transform" }}
                onMouseEnter={() => {
                  onHoverEffort(comparisonRow.effort.id);
                }}
                onMouseLeave={() => {
                  onHoverEffort(null);
                }}
              >
                <button
                  type="button"
                  className="min-w-0 text-left"
                  aria-pressed={comparisonRow.effort.id === referenceEffortId}
                  aria-label={`Make ${comparisonRow.effort.activity_title} the reference ride`}
                  onFocus={() => {
                    onHoverEffort(comparisonRow.effort.id);
                  }}
                  onBlur={() => {
                    onHoverEffort(null);
                  }}
                  onClick={() => {
                    onTogglePinnedEffort(comparisonRow.effort.id);
                  }}
                >
                  <div className="flex min-w-0 items-center gap-3">
                    <span
                      aria-hidden
                      className="inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-full text-sm font-semibold text-white"
                      style={{ backgroundColor: comparisonRow.color }}
                    >
                      {comparisonRow.markerLabel}
                    </span>
                    <span className="min-w-0 whitespace-normal break-words font-semibold leading-tight text-base-content">
                      {comparisonRow.effort.rider_name}
                    </span>
                  </div>
                </button>

                <div
                  className={`justify-self-end text-right font-semibold ${comparisonRow.isReference ? "text-base-content" : isPositiveGap ? "text-success" : isNegativeGap ? "text-error" : "text-base-content"}`}
                >
                  {timeValue}
                </div>

                <div
                  className={`justify-self-end text-right font-semibold ${comparisonRow.isReference ? "text-base-content" : isPositiveSpeed ? "text-success" : isNegativeSpeed ? "text-error" : "text-base-content"}`}
                >
                  {speedValue}
                </div>

                <div className="justify-self-end text-right text-sm font-medium text-base-content">
                  {heartRateValue}
                </div>

                <button
                  type="button"
                  className="inline-flex h-4 w-4 justify-self-end items-center justify-center text-base-content/50 transition hover:text-base-content"
                  aria-label={`Remove ${comparisonRow.effort.activity_title} from comparison`}
                  onClick={() => {
                    onRemoveEffort(comparisonRow.effort.id);
                  }}
                >
                  <FontAwesomeIcon icon={faXmark} className="h-3 w-3" />
                </button>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  ) : (
    <div className="flex h-full min-h-[24rem] items-center justify-center p-4">
      <div className="alert bg-base-100 text-sm text-base-content/70">
        <span>Add rides from the effort list to start the comparison.</span>
      </div>
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
  const playbackAnimationFrameRef = useRef<number | null>(null);
  const playbackLastTimestampRef = useRef<number | null>(null);
  const [playbackSeconds, setPlaybackSeconds] = useState(0);
  const [isPlaying, setIsPlaying] = useState(false);
  const [playbackPace, setPlaybackPace] = useState<PlaybackPace>("auto");
  const [hoveredEffortId, setHoveredEffortId] = useState<number | null>(null);
  const [pinnedEffortId, setPinnedEffortId] = useState<number | null>(null);
  const [effortTimeFilter, setEffortTimeFilter] =
    useState<EffortTimeFilter>("all");
  const [effortSearchQuery, setEffortSearchQuery] = useState("");
  const segment = segmentQuery.data;
  const allEfforts = segment?.efforts ?? EMPTY_EFFORTS;
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
  const currentUserPrDurationSeconds =
    currentUserPr?.duration_seconds ??
    segment?.current_user_pr_duration_seconds ??
    null;
  const currentUserPrLabel = currentUserPr
    ? currentUserPr.activity_title
    : segment?.current_user_pr_duration_seconds != null
      ? "Personal best across matched efforts"
      : "No PR yet";
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
        markerLabel: String(index + 1),
      })),
    [selectedEfforts],
  );
  const focusedEffortId = hoveredEffortId ?? pinnedEffortId;
  const referenceEffortId =
    selectedEfforts.find((effort) => effort.id === pinnedEffortId)?.id ??
    selectedEfforts.find((effort) => {
      if (currentUserId != null) {
        return effort.rider_user_id === currentUserId;
      }

      return currentUserName ? effort.rider_name === currentUserName : false;
    })?.id ??
    selectedEfforts[0]?.id ??
    null;
  const referenceEffort =
    selectedEfforts.find((effort) => effort.id === referenceEffortId) ?? null;
  const playbackLimitSeconds = referenceEffort?.duration_seconds ?? 0;
  const targetPlaybackDurationSeconds = playbackTargetSeconds(
    playbackLimitSeconds,
    playbackPace,
  );
  const liveComparisonRows = useMemo(
    () =>
      buildLiveComparisonRows(selectedRows, referenceEffort, playbackSeconds),
    [playbackSeconds, referenceEffort, selectedRows],
  );
  const routeDistanceMeters = resolveRouteDistanceMeters(
    segment?.route_points,
    segment?.distance_meters,
  );
  const routeNetElevationMeters = resolveRouteNetElevationMeters(
    segment?.route_points,
  );
  const routeGradePercent =
    routeDistanceMeters && routeNetElevationMeters != null
      ? (routeNetElevationMeters / routeDistanceMeters) * 100
      : null;
  const comparisonSelectionLabel =
    selectedRows.length === 1
      ? "1 ride selected"
      : `${selectedRows.length} rides selected`;
  const referenceSummaryLabel = referenceEffort
    ? `${referenceEffort.rider_name} · ${formatDuration(referenceEffort.duration_seconds)}`
    : "No reference ride";

  useEffect(() => {
    setEffortSearchQuery("");
  }, [segment?.id]);

  useEffect(() => {
    if (!segment || allEfforts.length === 0) {
      initializedSelectionSegmentIdRef.current = null;
      setSelectedEffortIds((current) =>
        current.length === 0 ? current : EMPTY_EFFORT_IDS,
      );
      setPlaybackSeconds((current) => (current === 0 ? current : 0));
      setIsPlaying((current) => (current ? false : current));
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
        return areEffortIdListsEqual(current, valid) ? current : valid;
      }

      if (!shouldSeedSelection) {
        return current.length === 0 ? current : EMPTY_EFFORT_IDS;
      }

      const seeded = allEfforts
        .slice(0, Math.min(3, allEfforts.length))
        .map((effort) => effort.id);

      return areEffortIdListsEqual(current, seeded) ? current : seeded;
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
      if (current.includes(effortId)) {
        return current;
      }

      return [...current, effortId];
    });
  }

  function removeEffortFromComparison(effortId: number) {
    setSelectedEffortIds((current) => current.filter((id) => id !== effortId));
  }

  useEffect(() => {
    if (playbackLimitSeconds <= 0) {
      setPlaybackSeconds(0);
      setIsPlaying(false);
      return;
    }

    setPlaybackSeconds((current) => Math.min(current, playbackLimitSeconds));
  }, [playbackLimitSeconds]);

  useEffect(() => {
    if (!isPlaying || playbackLimitSeconds <= 0) {
      playbackLastTimestampRef.current = null;
      return undefined;
    }

    const tick = (timestamp: number) => {
      const previousTimestamp = playbackLastTimestampRef.current ?? timestamp;
      const deltaSeconds =
        targetPlaybackDurationSeconds > 0
          ? ((timestamp - previousTimestamp) / 1000) *
            (playbackLimitSeconds / targetPlaybackDurationSeconds)
          : 0;

      playbackLastTimestampRef.current = timestamp;

      let reachedEnd = false;

      setPlaybackSeconds((current) => {
        const next = Math.min(current + deltaSeconds, playbackLimitSeconds);

        if (next >= playbackLimitSeconds - PLAYBACK_END_EPSILON) {
          reachedEnd = true;
          return playbackLimitSeconds;
        }

        return next;
      });

      if (reachedEnd) {
        playbackLastTimestampRef.current = null;
        playbackAnimationFrameRef.current = null;
        setIsPlaying(false);
        return;
      }

      playbackAnimationFrameRef.current = window.requestAnimationFrame(tick);
    };

    playbackAnimationFrameRef.current = window.requestAnimationFrame(tick);

    return () => {
      if (playbackAnimationFrameRef.current != null) {
        window.cancelAnimationFrame(playbackAnimationFrameRef.current);
        playbackAnimationFrameRef.current = null;
      }
      playbackLastTimestampRef.current = null;
    };
  }, [isPlaying, playbackLimitSeconds, targetPlaybackDurationSeconds]);

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
        description="Select attempts, then use time to open the full activity detail."
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
    <section className="space-y-6">
      <div className="card bg-base-100 shadow-xl">
        <div className="card-body gap-6">
          <div className="flex flex-wrap items-start justify-between gap-6">
            <div>
              <p className="text-xs font-semibold uppercase tracking-[0.22em] text-base-content/45">
                FMR / Effort Comparison
              </p>
              <h1 className="mt-2 text-4xl font-semibold tracking-tight">
                {segment.title}
              </h1>
              <div className="mt-4 flex flex-wrap items-center gap-4 text-sm text-base-content/70">
                <span className="inline-flex items-center gap-2">
                  <FontAwesomeIcon
                    icon={faRoute}
                    className="h-3.5 w-3.5 text-base-content/40"
                  />
                  {formatDistance(
                    routeDistanceMeters ?? segment.distance_meters,
                    unitSystem,
                  )}
                </span>
                <span>{formatGradePercent(routeGradePercent)}</span>
                <span>
                  {formatElevation(
                    routeNetElevationMeters != null
                      ? Math.abs(routeNetElevationMeters)
                      : null,
                    unitSystem,
                  )}{" "}
                  elev
                </span>
              </div>
              <div className="mt-3 flex flex-wrap items-center gap-2 text-xs text-base-content/60">
                <span>
                  Imported {formatActivityTimestamp(segment.created_at)} from{" "}
                  {segment.source}
                </span>
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

            <div className="grid gap-3 sm:grid-cols-2">
              <div className="min-w-[14rem] rounded-[1.25rem] border border-base-300 bg-base-200 px-4 py-4">
                <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.16em] text-base-content/55">
                  <FontAwesomeIcon
                    icon={faMedal}
                    className="h-3.5 w-3.5 text-primary"
                  />
                  <span>
                    Your PR {formatDuration(currentUserPrDurationSeconds)}
                  </span>
                </div>
                <div className="mt-2 font-semibold text-base-content">
                  {currentUserPr?.rider_name ?? currentUserName ?? "You"}
                </div>
                <div className="mt-1 text-sm text-base-content/65">
                  {currentUserPrLabel}
                </div>
              </div>

              <div className="min-w-[14rem] rounded-[1.25rem] border border-base-300 bg-base-200 px-4 py-4">
                <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.16em] text-base-content/55">
                  <FontAwesomeIcon
                    icon={faCrown}
                    className="h-3.5 w-3.5 text-warning"
                  />
                  <span>
                    KOM {formatDuration(overallKom?.duration_seconds ?? null)}
                  </span>
                </div>
                <div className="mt-2 font-semibold text-base-content">
                  {overallKom?.rider_name ?? "No efforts yet"}
                </div>
                <div className="mt-1 text-sm text-base-content/65">
                  {overallKom?.activity_title ??
                    "Waiting for the first matched effort"}
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="card border border-base-300 bg-base-100 shadow-xl">
        <div className="card-body">
          <div className="flex flex-wrap items-end justify-between gap-4">
            <div>
              <h2 className="card-title text-xl">Efforts</h2>
              <p className="text-sm text-base-content/70">
                Select as many attempts as you want, then use time to open the
                full activity detail.
              </p>
            </div>
            <span className="badge badge-outline whitespace-nowrap">
              {comparisonSelectionLabel}
            </span>
          </div>

          <div className="mt-5 min-w-0 rounded-box border border-base-300 bg-base-200 p-4">
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
                      <th className="w-20">
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
                      const selectedRow = selectedRows.find(
                        (row) => row.effort.id === effort.id,
                      );
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
                              <div className="flex items-center gap-1.5">
                                {selectedRow ? (
                                  <span
                                    aria-hidden
                                    className="inline-flex h-5 w-5 items-center justify-center rounded-full text-[0.65rem] font-semibold text-white"
                                    style={{
                                      backgroundColor: selectedRow.color,
                                    }}
                                  >
                                    {selectedRow.markerLabel}
                                  </span>
                                ) : null}
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
                              </div>
                            ) : (
                              <button
                                type="button"
                                className="btn btn-ghost btn-xs btn-circle"
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
                                  Add to comparison
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
        </div>
      </div>

      <div className="card bg-base-100 shadow-xl">
        <div className="card-body gap-4">
          <div className="flex flex-wrap items-end justify-between gap-4">
            <div>
              <h2 className="text-xl font-semibold text-base-content">
                Comparison workspace
              </h2>
              <p className="text-sm text-base-content/70">
                Playback follows the reference ride so time gaps, speed, and
                heart rate update on every frame.
              </p>
            </div>
            <span className="badge badge-outline whitespace-nowrap">
              Ref: {referenceSummaryLabel}
            </span>
          </div>

          <div className="overflow-hidden border border-base-300 bg-base-200">
            <div className="grid gap-0 xl:grid-cols-[minmax(0,1fr)_minmax(24rem,0.78fr)]">
              <div className="min-h-[24rem] border-b border-base-300 xl:border-b-0 xl:border-r">
                <RouteComparisonMap
                  routePoints={segment.route_points}
                  selectedRows={selectedRows}
                  playbackSeconds={playbackSeconds}
                />
              </div>

              <SelectedEffortsPanel
                comparisonRows={liveComparisonRows}
                focusedEffortId={focusedEffortId}
                referenceEffortId={referenceEffortId}
                playbackSeconds={playbackSeconds}
                unitSystem={unitSystem}
                onHoverEffort={setHoveredEffortId}
                onTogglePinnedEffort={togglePinnedEffort}
                onRemoveEffort={removeEffortFromComparison}
              />
            </div>

            <div className="border-t border-base-300 bg-base-100/95">
              <ComparisonChart
                routePoints={segment.route_points}
                routeDistanceMeters={routeDistanceMeters}
                selectedRows={selectedRows}
                referenceEffortId={referenceEffortId}
                playbackSeconds={playbackSeconds}
                unitSystem={unitSystem}
              />
            </div>

            <div className="flex flex-wrap items-center gap-3 border-t border-base-300 bg-base-100 px-4 py-3">
              <button
                type="button"
                className="btn btn-sm btn-circle shrink-0 border-0 bg-orange-500 text-white hover:bg-orange-600"
                disabled={
                  selectedRows.length === 0 || playbackLimitSeconds <= 0
                }
                aria-label={
                  isPlaying
                    ? "Pause comparison playback"
                    : playbackSeconds >= playbackLimitSeconds
                      ? "Replay comparison playback"
                      : "Play comparison playback"
                }
                onClick={() => {
                  if (playbackSeconds >= playbackLimitSeconds) {
                    setPlaybackSeconds(0);
                  }
                  setIsPlaying((current) => !current);
                }}
              >
                <FontAwesomeIcon
                  icon={isPlaying ? faPause : faPlay}
                  className="h-4 w-4"
                />
              </button>

              <input
                type="range"
                min={0}
                max={Math.max(playbackLimitSeconds, 1)}
                step={0.1}
                value={Math.min(
                  playbackSeconds,
                  Math.max(playbackLimitSeconds, 1),
                )}
                className="range range-primary min-w-[14rem] flex-1"
                disabled={
                  selectedRows.length === 0 || playbackLimitSeconds <= 0
                }
                aria-label="Playback timeline"
                onChange={(event) => {
                  setPlaybackSeconds(Number(event.target.value));
                  setIsPlaying(false);
                }}
              />

              <div className="join">
                {PLAYBACK_PACE_OPTIONS.map((option) => (
                  <button
                    key={option.key}
                    type="button"
                    className={`join-item btn btn-sm ${playbackPace === option.key ? "btn-neutral" : "btn-ghost"}`}
                    aria-pressed={playbackPace === option.key}
                    onClick={() => {
                      setPlaybackPace(option.key);
                    }}
                  >
                    {option.label}
                  </button>
                ))}
              </div>

              <span className="badge badge-outline min-w-[11.5rem] justify-center whitespace-nowrap text-center">
                {formatDuration(Math.round(playbackSeconds))} /{" "}
                {formatDuration(playbackLimitSeconds)}
              </span>

              <span className="badge badge-ghost whitespace-nowrap">
                {PLAYBACK_PACE_OPTIONS.find(
                  (option) => option.key === playbackPace,
                )?.label ?? "Auto"}{" "}
                {formatDuration(Math.round(targetPlaybackDurationSeconds))}{" "}
                target
              </span>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
