"use client";

import { auth } from "@ericbutera/kaleido";
import {
  faBars,
  faChevronDown,
  faChevronUp,
  faCrown,
  faMedal,
  faRocket,
  faStar,
  faTrophy,
} from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useMemo, useState } from "react";
import {
  Area,
  CartesianGrid,
  ComposedChart,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import {
  extractApiMessage,
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
  type UnitSystem,
} from "../lib/activityFormatting";
import {
  type ActivityChartPoint,
  type ActivityHeartRateZone,
  type ActivityLap,
  type ActivityRoutePoint,
  type ActivitySegmentEffort,
  useActivity,
  useDeleteActivity,
  useRegenerateActivity,
  useSegments,
  useUpdateSegment,
} from "../lib/queries";
import {
  primarySegmentAchievement,
  type SegmentAchievement,
  type SegmentAchievementKind,
} from "../lib/segmentAchievements";
import { hasSegmentBuilderRoute } from "../lib/segmentBuilder";
import { useUnitPreferences } from "../lib/unitPreferences";
import AuthRequiredCard from "./AuthRequiredCard";
import MapLibreRouteMap from "./MapLibreRouteMap";

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

type SegmentAttemptChartPoint = {
  effort: ActivitySegmentEffort;
  segmentTitle: string;
  runNumber: number;
  runLabel: string;
  durationSeconds: number;
  maxHeartRate: number | null;
  overallRank: number | null | undefined;
  personalRank: number | null | undefined;
  personalBestDurationSeconds: number | null | undefined;
  isKom: boolean;
  isPersonalRecord: boolean;
  isFastestOfDay: boolean;
};

type SegmentAttemptTooltipState = {
  attempt: SegmentAttemptChartPoint;
  x: number;
  y: number;
};

const SEGMENT_TONES: SegmentTone[] = [
  {
    mapColor: "var(--color-primary)",
    dotClassName: "bg-primary",
    chartClassName: "text-primary",
    buttonClassName: "btn-primary",
    outlineButtonClassName: "btn-outline btn-primary",
    highlightClassName: "ring-primary/25",
  },
  {
    mapColor: "var(--color-secondary)",
    dotClassName: "bg-secondary",
    chartClassName: "text-secondary",
    buttonClassName: "btn-secondary",
    outlineButtonClassName: "btn-outline btn-secondary",
    highlightClassName: "ring-secondary/25",
  },
  {
    mapColor: "var(--color-accent)",
    dotClassName: "bg-accent",
    chartClassName: "text-accent",
    buttonClassName: "btn-accent",
    outlineButtonClassName: "btn-outline btn-accent",
    highlightClassName: "ring-accent/25",
  },
  {
    mapColor: "var(--color-info)",
    dotClassName: "bg-info",
    chartClassName: "text-info",
    buttonClassName: "btn-info",
    outlineButtonClassName: "btn-outline btn-info",
    highlightClassName: "ring-info/25",
  },
  {
    mapColor: "var(--color-warning)",
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

function formatOverallRank(rank: number | null | undefined) {
  return rank != null ? `#${rank} overall` : "No global rank";
}

function formatPersonalRank(rank: number | null | undefined) {
  return rank != null ? `#${rank} all-time` : "No PR history";
}

function formatPrDelta(
  durationSeconds: number,
  personalBestDurationSeconds: number | null | undefined,
) {
  if (personalBestDurationSeconds == null) {
    return null;
  }

  const deltaSeconds = durationSeconds - personalBestDurationSeconds;

  if (deltaSeconds === 0) {
    return "At PR";
  }

  const magnitude = formatDuration(Math.abs(deltaSeconds));

  if (deltaSeconds > 0) {
    return `${magnitude} off PR`;
  }

  return `${magnitude} faster than prior PR`;
}

function segmentHistoricalAchievements(segmentGroup: SegmentMatchGroup) {
  return primarySegmentAchievement({
    overallRank: segmentGroup.bestEffort.overall_rank,
    personalRank: segmentGroup.bestEffort.personal_rank,
  });
}

function achievementIcon(kind: SegmentAchievementKind) {
  switch (kind) {
    case "kom":
      return faCrown;
    case "top-10":
      return faTrophy;
    case "pr":
      return faMedal;
    case "fastest":
      return faRocket;
  }
}

function achievementBadgeClassName(kind: SegmentAchievementKind) {
  switch (kind) {
    case "kom":
      return "badge badge-warning badge-outline gap-1";
    case "top-10":
      return "badge badge-info badge-outline gap-1";
    case "pr":
      return "badge badge-primary badge-outline gap-1";
    case "fastest":
      return "badge badge-success badge-outline gap-1";
  }
}

function achievementMarkerFill(kind: SegmentAchievementKind) {
  switch (kind) {
    case "kom":
      return "var(--color-warning)";
    case "top-10":
      return "var(--color-info)";
    case "pr":
      return "var(--color-primary)";
    case "fastest":
      return "var(--color-success)";
  }
}

function SegmentAchievementBadge({
  achievement,
}: {
  achievement: SegmentAchievement;
}) {
  return (
    <span className={achievementBadgeClassName(achievement.kind)}>
      <FontAwesomeIcon
        icon={achievementIcon(achievement.kind)}
        className="h-3 w-3"
      />
      <span>{achievement.longLabel}</span>
    </span>
  );
}

function isSameSegmentEffort(
  left: ActivitySegmentEffort,
  right: ActivitySegmentEffort,
) {
  return (
    left.segment_id === right.segment_id &&
    left.effort_index === right.effort_index &&
    left.start_route_point_index === right.start_route_point_index &&
    left.end_route_point_index === right.end_route_point_index
  );
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

function maxHeartRateForSegmentEffort(
  routePoints: ActivityRoutePoint[] | null | undefined,
  segmentEffort: ActivitySegmentEffort,
) {
  const heartRateValues = segmentOverlayPoints(
    routePoints,
    segmentEffort,
  ).flatMap((point) =>
    point.heart_rate_bpm == null || Number.isNaN(point.heart_rate_bpm)
      ? []
      : [point.heart_rate_bpm],
  );

  if (heartRateValues.length === 0) {
    return null;
  }

  return Math.round(Math.max(...heartRateValues));
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
        const maxHeartRate = maxHeartRateForSegmentEffort(routePoints, effort);

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

function AttemptAchievementBadges({
  attempt,
}: {
  attempt: SegmentAttemptChartPoint;
}) {
  const achievement = primarySegmentAchievement({
    overallRank: attempt.overallRank,
    personalRank: attempt.personalRank,
    isFastestOfDay: attempt.isFastestOfDay,
  });

  return achievement ? (
    <SegmentAchievementBadge achievement={achievement} />
  ) : null;
}

function iconPaths(
  icon: typeof faCrown | typeof faMedal | typeof faRocket | typeof faTrophy,
) {
  const pathData = icon.icon[4];
  return Array.isArray(pathData) ? pathData : [pathData];
}

function ChartMarkerIcon({
  icon,
  cx,
  cy,
  fill,
  size,
  stroke,
  strokeWidth = 0,
  offsetX = 0,
  offsetY = 0,
}: {
  icon: typeof faCrown | typeof faMedal | typeof faRocket | typeof faTrophy;
  cx: number;
  cy: number;
  fill: string;
  size: number;
  stroke?: string;
  strokeWidth?: number;
  offsetX?: number;
  offsetY?: number;
}) {
  const [iconWidth, iconHeight] = icon.icon;
  const paths = iconPaths(icon);

  return (
    <svg
      x={cx - size / 2 + offsetX}
      y={cy - size / 2 + offsetY}
      width={size}
      height={size}
      viewBox={`0 0 ${iconWidth} ${iconHeight}`}
      overflow="visible"
      pointerEvents="none"
    >
      {paths.map((path, index) => (
        <path
          key={`${icon.iconName}-${index}`}
          d={path}
          fill={fill}
          stroke={stroke}
          strokeWidth={strokeWidth}
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      ))}
    </svg>
  );
}

function SegmentAttemptTooltipContent({
  attempt,
}: {
  attempt: SegmentAttemptChartPoint;
}) {
  const prDelta = formatPrDelta(
    attempt.durationSeconds,
    attempt.personalBestDurationSeconds,
  );

  return (
    <div className="rounded-box border border-base-300 bg-base-100 px-3 py-3 shadow-lg">
      <p className="text-sm font-semibold text-base-content">
        {`Run ${attempt.runNumber} · ${formatDuration(attempt.durationSeconds)}`}
      </p>
      <p className="mt-1 text-sm text-base-content/70">
        {`Leaderboard ${formatOverallRank(attempt.overallRank)} · Max heart rate ${formatHeartRate(attempt.maxHeartRate)}`}
      </p>
      <p className="mt-1 text-xs text-base-content/60">
        {`Personal rank ${formatPersonalRank(attempt.personalRank)}`}
      </p>
      {prDelta ? (
        <p className="mt-1 text-xs font-medium text-base-content/75">
          {prDelta}
        </p>
      ) : null}
      <div className="mt-2 flex flex-wrap gap-2">
        <AttemptAchievementBadges attempt={attempt} />
      </div>
    </div>
  );
}

function SegmentAttemptDot({
  active = false,
  cx,
  cy,
  payload,
  toneColor,
  isHighlighted,
  onDismiss,
  onSelect,
  ...interactionProps
}: {
  active?: boolean;
  cx?: number;
  cy?: number;
  payload?: SegmentAttemptChartPoint;
  toneColor: string;
  isHighlighted: boolean;
  onDismiss: () => void;
  onSelect: (state: SegmentAttemptTooltipState) => void;
  [key: string]: unknown;
}) {
  if (cx == null || cy == null || !payload) {
    return null;
  }

  const achievement = primarySegmentAchievement({
    overallRank: payload.overallRank,
    personalRank: payload.personalRank,
    isFastestOfDay: payload.isFastestOfDay,
  });
  const showKomMarker = achievement?.kind === "kom";
  const showTopMarker = achievement?.kind === "top-10";
  const showPrMarker = achievement?.kind === "pr";
  const showFastestMarker = achievement?.kind === "fastest";
  const hitRadius =
    showKomMarker || showTopMarker || showPrMarker || showFastestMarker
      ? 14
      : 11;
  const highlightRadius =
    showKomMarker || showTopMarker || showPrMarker
      ? 8.5
      : showFastestMarker
        ? 7.5
        : 5.25;

  return (
    <g
      {...interactionProps}
      role="button"
      tabIndex={0}
      aria-label={`${payload.segmentTitle} run ${payload.runNumber} point`}
      className="cursor-pointer"
      onBlur={() => {
        onDismiss();
      }}
      onClick={() => {
        onSelect({ attempt: payload, x: cx, y: cy });
      }}
      onFocus={() => {
        onSelect({ attempt: payload, x: cx, y: cy });
      }}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onSelect({ attempt: payload, x: cx, y: cy });
        }
      }}
      onMouseEnter={() => {
        onSelect({ attempt: payload, x: cx, y: cy });
      }}
      onMouseLeave={() => {
        onDismiss();
      }}
    >
      <circle cx={cx} cy={cy} r={hitRadius} fill="transparent" />
      {isHighlighted ? (
        <circle
          cx={cx}
          cy={cy}
          r={highlightRadius}
          fill="none"
          stroke="var(--color-base-content)"
          strokeOpacity={0.35}
          strokeWidth={1.25}
        />
      ) : null}

      {showKomMarker ? (
        <ChartMarkerIcon
          icon={faCrown}
          cx={cx}
          cy={cy}
          fill={achievementMarkerFill("kom")}
          size={active || isHighlighted ? 20 : 18}
          stroke="var(--color-base-100)"
          strokeWidth={24}
        />
      ) : null}

      {showTopMarker ? (
        <ChartMarkerIcon
          icon={faTrophy}
          cx={cx}
          cy={cy}
          fill={achievementMarkerFill("top-10")}
          size={active || isHighlighted ? 18 : 16}
          stroke="var(--color-base-100)"
          strokeWidth={24}
        />
      ) : null}

      {showPrMarker ? (
        <ChartMarkerIcon
          icon={faMedal}
          cx={cx}
          cy={cy}
          fill={achievementMarkerFill("pr")}
          size={active || isHighlighted ? 20 : 18}
          stroke="var(--color-base-100)"
          strokeWidth={24}
        />
      ) : null}

      {showFastestMarker ? (
        <ChartMarkerIcon
          icon={faRocket}
          cx={cx}
          cy={cy}
          fill="var(--color-success)"
          size={active || isHighlighted ? 18 : 16}
          stroke="var(--color-base-100)"
          strokeWidth={26}
        />
      ) : null}

      {!showKomMarker && !showPrMarker && !showFastestMarker ? (
        <circle
          cx={cx}
          cy={cy}
          r={active || isHighlighted ? 4.25 : 3.5}
          fill={toneColor}
          stroke="var(--color-base-100)"
          strokeWidth={1}
        />
      ) : null}
    </g>
  );
}

function SegmentAttemptsChart({
  segmentGroup,
  routePoints,
}: {
  segmentGroup: SegmentMatchGroup;
  routePoints: ActivityRoutePoint[] | null | undefined;
}) {
  const [tooltipState, setTooltipState] =
    useState<SegmentAttemptTooltipState | null>(null);
  const chartWidth = 520;
  const chartHeight = 120;
  const attempts: SegmentAttemptChartPoint[] = [...segmentGroup.efforts]
    .sort(
      (left, right) =>
        left.effort_index - right.effort_index ||
        left.start_route_point_index - right.start_route_point_index ||
        left.end_route_point_index - right.end_route_point_index,
    )
    .map((effort) => ({
      effort,
      segmentTitle: segmentGroup.segmentTitle,
      runNumber: effort.effort_index,
      runLabel: `Run ${effort.effort_index}`,
      durationSeconds: effort.duration_seconds,
      maxHeartRate: maxHeartRateForSegmentEffort(routePoints, effort),
      overallRank: effort.overall_rank,
      personalRank: effort.personal_rank,
      personalBestDurationSeconds: effort.personal_best_duration_seconds,
      isKom: effort.overall_rank === 1,
      isPersonalRecord: effort.personal_rank === 1,
      isFastestOfDay: isSameSegmentEffort(effort, segmentGroup.bestEffort),
    }));

  if (attempts.length === 0) {
    return null;
  }

  const minDuration = Math.min(
    ...attempts.map((attempt) => attempt.durationSeconds),
  );
  const maxDuration = Math.max(
    ...attempts.map((attempt) => attempt.durationSeconds),
  );
  const durationSpread = Math.max(maxDuration - minDuration, 1);
  const chartPadding = Math.max(4, Math.round(durationSpread * 0.12));
  const yAxisDomain: [number, number] = [
    Math.max(0, minDuration - chartPadding),
    maxDuration + chartPadding,
  ];
  const xAxisTicks = attempts.map((attempt) => attempt.runNumber);
  const xAxisDomain: [number, number] = [
    Math.min(...xAxisTicks),
    Math.max(...xAxisTicks),
  ];

  return (
    <div
      role="img"
      aria-label={`${segmentGroup.segmentTitle} attempts chart`}
      className="relative overflow-visible rounded-box border border-base-300 bg-base-100 p-2"
    >
      {tooltipState ? (
        <div
          className="pointer-events-none absolute z-10 w-max max-w-64 -translate-x-1/2 -translate-y-[calc(100%+0.75rem)]"
          style={{
            left: `${(tooltipState.x / chartWidth) * 100}%`,
            top: `${(tooltipState.y / chartHeight) * 100}%`,
          }}
        >
          <SegmentAttemptTooltipContent attempt={tooltipState.attempt} />
        </div>
      ) : null}
      <LineChart
        width={chartWidth}
        height={chartHeight}
        data={attempts}
        margin={{ top: 10, right: 14, bottom: 4, left: 4 }}
        style={{ width: "100%", height: "auto" }}
        onMouseLeave={() => {
          setTooltipState(null);
        }}
      >
        <CartesianGrid
          vertical={false}
          stroke="var(--color-base-content)"
          strokeOpacity={0.12}
        />
        <XAxis
          axisLine={false}
          allowDecimals={false}
          dataKey="runNumber"
          domain={xAxisDomain}
          tickFormatter={(value: number) => `Run ${value}`}
          tick={{ fill: "var(--color-base-content)", fontSize: 8 }}
          tickLine={false}
          ticks={xAxisTicks}
          type="number"
        />
        <YAxis
          axisLine={false}
          domain={yAxisDomain}
          label={{
            angle: -90,
            fill: "var(--color-base-content)",
            fontSize: 8,
            position: "insideLeft",
            style: { opacity: 0.65 },
            value: "Time",
          }}
          tick={{ fill: "var(--color-base-content)", fontSize: 8 }}
          tickFormatter={(value: number) => formatDuration(Math.round(value))}
          tickLine={false}
          tickMargin={6}
          width={56}
        />
        <Line
          type="linear"
          dataKey="durationSeconds"
          stroke={segmentGroup.tone.mapColor}
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth={2}
          dot={(dotProps) => (
            <SegmentAttemptDot
              active={false}
              cx={dotProps.cx}
              cy={dotProps.cy}
              isHighlighted={
                tooltipState?.attempt.runNumber ===
                (dotProps.payload as SegmentAttemptChartPoint).runNumber
              }
              onDismiss={() => {
                setTooltipState(null);
              }}
              payload={dotProps.payload as SegmentAttemptChartPoint}
              onSelect={setTooltipState}
              toneColor={segmentGroup.tone.mapColor}
            />
          )}
          activeDot={false}
        />
      </LineChart>
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
  const maxX = visibleSeries.reduce(
    (maxValue, entry) =>
      Math.max(maxValue, entry.points[entry.points.length - 1]?.x ?? 0),
    0,
  );
  const rows = buildSignalChartRows(visibleSeries);

  return (
    <div className="card bg-base-100 shadow-xl">
      <div className="card-body">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <h2 className="card-title text-xl">Ride signals</h2>
            <p className="text-sm leading-6 text-base-content/70">
              Heart rate, power, speed, and elevation share the same time axis.
              Toggle layers to focus on effort, terrain, or both at once.
            </p>
          </div>
          <span className="badge badge-ghost uppercase">
            {sampleCount} samples
          </span>
        </div>

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
                      domain={[0, Math.max(maxX, 1)]}
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

function formatHeartRateZoneRange(zone: ActivityHeartRateZone) {
  if (zone.min_bpm == null && zone.max_bpm == null) {
    return "Range unavailable";
  }

  if (zone.min_bpm == null) {
    return `Up to ${formatHeartRate(zone.max_bpm)}`;
  }

  if (zone.max_bpm == null) {
    return `Above ${formatHeartRate(zone.min_bpm - 1)}`;
  }

  return `${formatHeartRate(zone.min_bpm)} to ${formatHeartRate(zone.max_bpm)}`;
}

function formatSharePercent(value: number) {
  return Number.isInteger(value)
    ? `${value.toFixed(0)}%`
    : `${value.toFixed(1)}%`;
}

function ActivityRouteMap({
  routePoints,
  segmentGroups,
  canRegenerate,
  isRegenerating,
  onRegenerate,
  onSelectSegment,
  selectedSegmentId,
}: {
  routePoints: ActivityRoutePoint[] | null | undefined;
  segmentGroups: SegmentMatchGroup[];
  canRegenerate: boolean;
  isRegenerating: boolean;
  onRegenerate: () => void;
  onSelectSegment: (segmentId: number) => void;
  selectedSegmentId: number | null;
}) {
  const hasRouteMap = (routePoints?.length ?? 0) >= 2;
  const overlays = segmentGroups
    .flatMap((segmentGroup) =>
      segmentGroup.efforts.map((segmentEffort) => ({
        id: `${segmentEffort.segment_id}-${segmentEffort.effort_index}`,
        color: segmentGroup.tone.mapColor,
        points: segmentOverlayPoints(routePoints, segmentEffort),
        weight: selectedSegmentId === segmentGroup.segmentId ? 8 : 6,
        onClick: () => {
          onSelectSegment(segmentGroup.segmentId);
        },
      })),
    )
    .filter((overlay) => overlay.points.length >= 2);

  return (
    <div className="card bg-base-100 shadow-xl">
      <div className="card-body">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <h2 className="card-title text-xl">Route map</h2>
            <p className="text-sm text-base-content/70">
              The full activity route is drawn from persisted upload-time route
              points. Imported segment matches are highlighted on top of it.
            </p>
          </div>
          <span className="badge badge-outline">
            {segmentGroups.length} matched segment
            {segmentGroups.length === 1 ? "" : "s"}
          </span>
        </div>

        {hasRouteMap ? (
          <>
            <div className="mt-5 overflow-hidden rounded-box border border-base-300 bg-base-200">
              <MapLibreRouteMap
                routePoints={routePoints}
                overlays={overlays}
                ariaLabel="Activity route map"
                emptyMessage="This activity does not have enough stored route points for the map yet."
                showZoomControls
                showLayerPicker
                defaultBasemap="topo"
                basemapOptions={["topo", "street", "satellite"]}
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
              {canRegenerate ? (
                <div className="mt-4">
                  <button
                    type="button"
                    className="btn btn-outline btn-sm"
                    onClick={onRegenerate}
                    disabled={isRegenerating}
                  >
                    {isRegenerating
                      ? "Regenerating..."
                      : "Regenerate route data"}
                  </button>
                </div>
              ) : null}
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
  const [expandedSegmentIds, setExpandedSegmentIds] = useState<number[]>([]);
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
  const updateSegmentMutation = useUpdateSegment();
  const activity = activityQuery.data;
  const canBuildSegment = hasSegmentBuilderRoute(activity?.route_points);
  const segmentBuilderHref = activity
    ? `/segments/builder?activityId=${activity.id}`
    : "/segments/builder";
  const matchedSegmentEfforts = sortMatchedSegmentEfforts(
    activity?.segment_efforts,
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
    setExpandedSegmentIds((current) =>
      current.includes(segmentId)
        ? current.filter((entry) => entry !== segmentId)
        : [...current, segmentId],
    );
  }

  async function toggleSegmentStar(segmentId: number, starred: boolean) {
    try {
      await updateSegmentMutation.updateAsync({ id: segmentId, starred });
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

          {activity.estimated_ftp_watts != null ||
          (activity.heart_rate_zones?.length ?? 0) > 0 ? (
            <div className="rounded-box border border-base-300 bg-base-200 p-4">
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div>
                  <h2 className="text-lg font-semibold text-base-content">
                    Training profile snapshot
                  </h2>
                  <p className="text-sm text-base-content/70">
                    Bike stores the heart rate zone time and FTP snapshot that
                    were active when this ride was imported or regenerated.
                  </p>
                </div>
                <span className="badge badge-outline">
                  {activity.estimated_ftp_watts != null
                    ? `FTP ${formatPower(activity.estimated_ftp_watts)}`
                    : "Heart rate zones"}
                </span>
              </div>

              {(activity.heart_rate_zones?.length ?? 0) > 0 ? (
                <div className="mt-4 grid gap-3 md:grid-cols-2 xl:grid-cols-5">
                  {(activity.heart_rate_zones ?? []).map((zone) => (
                    <div
                      key={zone.zone}
                      className="rounded-lg border border-base-300 bg-base-100 px-3 py-3"
                    >
                      <div className="flex items-center justify-between gap-3">
                        <span className="badge badge-ghost">{zone.label}</span>
                        <span className="text-sm font-medium text-base-content">
                          {formatDuration(zone.duration_seconds)}
                        </span>
                      </div>
                      <p className="mt-2 text-xs text-base-content/60">
                        {formatHeartRateZoneRange(zone)}
                      </p>
                      <progress
                        className="progress progress-primary mt-3 h-2 w-full"
                        value={zone.share_percent}
                        max={100}
                      />
                      <p className="mt-2 text-right text-xs text-base-content/60">
                        {formatSharePercent(zone.share_percent)}
                      </p>
                    </div>
                  ))}
                </div>
              ) : (
                <div className="alert mt-4 bg-base-100 text-sm text-base-content/75">
                  Heart rate zones were not stored on this ride yet. Save your
                  account zones and regenerate the upload to persist them.
                </div>
              )}
            </div>
          ) : null}
        </div>
      </div>

      <ActivityRouteMap
        routePoints={activity.route_points}
        segmentGroups={matchedSegmentGroups}
        canRegenerate={!!activity.can_regenerate}
        isRegenerating={regenerateMutation.isPending}
        onRegenerate={() => {
          void handleRegenerate();
        }}
        onSelectSegment={focusSegmentMatch}
        selectedSegmentId={selectedSegmentId}
      />

      <section
        className="grid gap-4"
        aria-labelledby="matched-segments-heading"
      >
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <h2 id="matched-segments-heading" className="text-xl font-semibold">
              Matched segments
            </h2>
            <p className="mt-1 text-sm text-base-content/70">
              Segment efforts are matched from imported segment routes against
              the upload-time route geometry stored on this activity.
            </p>
          </div>
          <span className="badge badge-outline">
            {matchedSegmentGroups.length} matched segment
            {matchedSegmentGroups.length === 1 ? "" : "s"}
          </span>
        </div>

        {matchedSegmentGroups.length > 0 ? (
          <div className="grid gap-4">
            {matchedSegmentGroups.map((segmentGroup) => {
              const segmentHref = `/segments/${segmentGroup.segmentId}`;
              const segmentAchievement =
                segmentHistoricalAchievements(segmentGroup);
              const isSelected = selectedSegmentId === segmentGroup.segmentId;
              const isStarred = starredSegmentIds.has(segmentGroup.segmentId);
              const isExpanded =
                isSelected ||
                isStarred ||
                expandedSegmentIds.includes(segmentGroup.segmentId);

              return (
                <article
                  id={segmentGroup.anchorId}
                  key={segmentGroup.segmentId}
                  className={`card border border-base-300 bg-base-100 shadow-sm ring-1 transition ${isSelected ? segmentGroup.tone.highlightClassName : "ring-base-300/0"}`}
                >
                  <div className="card-body gap-3">
                    <div className="flex flex-wrap items-start justify-between gap-3">
                      <div className="min-w-0 flex-1">
                        <div className="flex flex-wrap items-center gap-2">
                          <button
                            type="button"
                            className={`btn btn-ghost btn-xs btn-square ${isStarred ? "text-warning" : "text-base-content/45"}`}
                            aria-label={`${isStarred ? "Unstar" : "Star"} ${segmentGroup.segmentTitle}`}
                            aria-pressed={isStarred}
                            disabled={updateSegmentMutation.isPending}
                            onClick={() => {
                              void toggleSegmentStar(
                                segmentGroup.segmentId,
                                !isStarred,
                              );
                            }}
                          >
                            <FontAwesomeIcon
                              icon={faStar}
                              className="h-3.5 w-3.5"
                            />
                          </button>
                          <Link
                            href={segmentHref}
                            className="card-title text-lg transition hover:text-primary"
                          >
                            {segmentGroup.segmentTitle}
                          </Link>
                          <span className="badge badge-ghost badge-sm">
                            {segmentGroup.efforts.length} run
                            {segmentGroup.efforts.length === 1 ? "" : "s"}
                          </span>
                        </div>
                      </div>

                      <button
                        type="button"
                        className="btn btn-ghost btn-sm gap-2"
                        aria-expanded={isExpanded}
                        aria-controls={`${segmentGroup.anchorId}-details`}
                        disabled={isStarred}
                        onClick={() => {
                          toggleSegmentMatch(segmentGroup.segmentId);
                        }}
                      >
                        <span>
                          {isStarred
                            ? "Starred stays open"
                            : isExpanded
                              ? "Hide time & runs"
                              : "Show time & runs"}
                        </span>
                        <FontAwesomeIcon
                          icon={isExpanded ? faChevronUp : faChevronDown}
                          className="h-3.5 w-3.5"
                        />
                      </button>
                    </div>

                    {isExpanded ? (
                      <div
                        id={`${segmentGroup.anchorId}-details`}
                        className="grid gap-3"
                      >
                        <div className="flex flex-wrap gap-2">
                          <span className="badge badge-outline">
                            Best{" "}
                            {formatDuration(
                              segmentGroup.bestEffort.duration_seconds,
                            )}
                          </span>
                          <span className="badge badge-outline">
                            Leaderboard{" "}
                            {formatOverallRank(segmentGroup.bestOverallRank)}
                          </span>
                          <span className="badge badge-outline">
                            Peak HR{" "}
                            {formatHeartRate(segmentGroup.peakHeartRate)}
                          </span>
                          {segmentAchievement ? (
                            <SegmentAchievementBadge
                              achievement={segmentAchievement}
                            />
                          ) : null}
                        </div>

                        <SegmentAttemptsChart
                          segmentGroup={segmentGroup}
                          routePoints={activity.route_points}
                        />
                      </div>
                    ) : null}
                  </div>
                </article>
              );
            })}
          </div>
        ) : (
          <div className="alert mt-5">
            <span>
              No imported segment routes have matched this activity yet. If this
              is an older upload, regenerate it once so the latest route
              geometry and matcher run against the raw file.
            </span>
          </div>
        )}
      </section>

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
