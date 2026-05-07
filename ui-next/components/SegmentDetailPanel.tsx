"use client";

import { auth } from "@ericbutera/kaleido";
import { faPause, faPlay } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import Link from "next/link";
import { useEffect, useState } from "react";
import {
  extractApiMessage,
  formatActivityTimestamp,
  formatDistance,
  formatDuration,
  formatElevation,
  formatHeartRate,
  formatSpeed,
} from "../lib/activityFormatting";
import {
  type ActivityRoutePoint,
  type SegmentEffort,
  useSegment,
} from "../lib/queries";
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

const EFFORT_TIME_FILTERS = [
  { key: "all", label: "All" },
  { key: "day", label: "Day" },
  { key: "week", label: "Week" },
  { key: "month", label: "Month" },
  { key: "year", label: "Year" },
] as const;

type ChartMetric = "speed" | "heartRate" | "elevation";
type EffortTimeFilter = (typeof EFFORT_TIME_FILTERS)[number]["key"];

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

function formatMetricValue(metric: ChartMetric, value?: number | null) {
  if (metric === "speed") {
    return formatSpeed(value);
  }

  if (metric === "heartRate") {
    return formatHeartRate(value);
  }

  return formatElevation(value);
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

function interpolateSeriesPoint(
  points: Array<{ x: number; y: number }>,
  elapsedSeconds: number,
) {
  if (points.length === 0) {
    return null;
  }

  if (elapsedSeconds <= points[0].x) {
    return points[0];
  }

  for (let index = 1; index < points.length; index += 1) {
    const previous = points[index - 1];
    const current = points[index];

    if (elapsedSeconds <= current.x) {
      const span = Math.max(current.x - previous.x, 1);
      const progress = (elapsedSeconds - previous.x) / span;
      return {
        x: elapsedSeconds,
        y: previous.y + (current.y - previous.y) * progress,
      };
    }
  }

  return points.at(-1) ?? null;
}

function RouteComparisonMap({
  routePoints,
  selectedEfforts,
  playbackSeconds,
  maxDuration,
  isPlaying,
  onTogglePlayback,
  onSeek,
}: {
  routePoints: ActivityRoutePoint[] | null | undefined;
  selectedEfforts: SegmentEffort[];
  playbackSeconds: number;
  maxDuration: number;
  isPlaying: boolean;
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

      return {
        id: effort.id,
        color: EFFORT_COLORS[index % EFFORT_COLORS.length],
        point,
        label: effort.activity_title,
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

  return (
    <div className="card bg-base-100 shadow-xl">
      <div className="card-body">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <h2 className="card-title text-xl">Effort comparison map</h2>
            <p className="text-sm text-base-content/70">
              Play the selected attempts against the same route to see where
              each ride is gaining or losing time.
            </p>
          </div>
          <span className="badge badge-outline">
            {selectedEfforts.length} selected
          </span>
        </div>
        <div className="mt-5 overflow-hidden rounded-box border border-base-300 bg-base-200">
          {hasRouteMap ? (
            <LeafletRouteMap
              routePoints={routePoints}
              movingMarkers={markers.map((marker) => ({
                id: String(marker.id),
                point: marker.point,
                color: marker.color,
              }))}
              ariaLabel="Segment comparison map"
              emptyMessage="Segment route geometry is not available yet."
              className="h-96 w-full"
            />
          ) : (
            <div className="alert">
              Segment route geometry is not available yet.
            </div>
          )}
        </div>

        <div className="mt-5 flex items-center gap-3">
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

        <div className="mt-4 flex flex-wrap gap-2">
          {selectedEfforts.map((effort, index) => (
            <div
              key={effort.id}
              className="badge badge-outline gap-2 px-3 py-3"
            >
              <span
                className="inline-block h-2.5 w-2.5 rounded-full"
                style={{
                  backgroundColor: EFFORT_COLORS[index % EFFORT_COLORS.length],
                }}
              />
              <span>{effort.activity_title}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

function ComparisonChart({
  metric,
  routePoints,
  selectedEfforts,
  playbackSeconds,
  onMetricChange,
}: {
  metric: ChartMetric;
  routePoints: ActivityRoutePoint[] | null | undefined;
  selectedEfforts: SegmentEffort[];
  playbackSeconds: number;
  onMetricChange: (value: ChartMetric) => void;
}) {
  const [hoveredSeconds, setHoveredSeconds] = useState<number | null>(null);
  const series = selectedEfforts.map((effort, index) => ({
    effort,
    color: EFFORT_COLORS[index % EFFORT_COLORS.length],
    points: buildChartSeries(metric, effort),
  }));
  const allPoints = series.flatMap((entry) => entry.points);

  if (allPoints.length < 2) {
    return (
      <div className="alert">
        The selected efforts do not have enough point-level data for a shared
        chart.
      </div>
    );
  }

  const width = 680;
  const height = 172;
  const leftPadding = 44;
  const rightPadding = 44;
  const topPadding = 18;
  const bottomPadding = 28;
  const chartWidth = width - leftPadding - rightPadding;
  const chartHeight = height - topPadding - bottomPadding;
  const maxX = Math.max(...allPoints.map((point) => point.x), 1);
  const elevationPoints = buildElevationBackdropSeries(routePoints, maxX);
  const minY = Math.min(...allPoints.map((point) => point.y));
  const maxY = Math.max(...allPoints.map((point) => point.y));
  const yRange = Math.max(maxY - minY, 1);
  const minElevation = elevationPoints.length > 0
    ? Math.min(...elevationPoints.map((point) => point.y))
    : 0;
  const maxElevation = elevationPoints.length > 0
    ? Math.max(...elevationPoints.map((point) => point.y))
    : 0;
  const elevationRange = Math.max(maxElevation - minElevation, 1);
  const displaySeconds = hoveredSeconds ?? playbackSeconds;
  const toSvgX = (value: number) =>
    leftPadding + (value / maxX) * chartWidth;
  const toSvgY = (value: number) =>
    topPadding + (1 - (value - minY) / yRange) * chartHeight;
  const toElevationSvgY = (value: number) =>
    topPadding + (1 - (value - minElevation) / elevationRange) * chartHeight;
  const hoveredElevationPoint = interpolateSeriesPoint(
    elevationPoints,
    displaySeconds,
  );

  return (
    <div className="card bg-base-100 shadow-xl">
      <div className="card-body">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <h2 className="card-title text-xl">Chart comparison</h2>
            <p className="text-sm text-base-content/70">
              Compare the selected attempts across elapsed time while the map
              dots advance.
            </p>
          </div>
          <div className="join">
            {(["speed", "heartRate"] as ChartMetric[]).map(
              (nextMetric) => (
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
              ),
            )}
          </div>
        </div>

        <div className="mt-5 overflow-hidden rounded-box border border-base-300 bg-base-200 p-3">
          <svg
            viewBox={`0 0 ${width} ${height}`}
            preserveAspectRatio="none"
            className="block h-40 w-full"
            role="img"
            aria-label="Segment comparison chart"
            onMouseLeave={() => {
              setHoveredSeconds(null);
            }}
            onMouseMove={(event) => {
              const rect = event.currentTarget.getBoundingClientRect();
              const x = ((event.clientX - rect.left) / rect.width) * width;
              const clampedX = Math.min(
                Math.max(x, leftPadding),
                width - rightPadding,
              );
              const progress = (clampedX - leftPadding) / chartWidth;
              setHoveredSeconds(progress * maxX);
            }}
          >
            {[0, 0.25, 0.5, 0.75, 1].map((fraction) => {
              const y = topPadding + fraction * chartHeight;
              return (
                <line
                  key={fraction}
                  x1={leftPadding}
                  y1={y}
                  x2={width - rightPadding}
                  y2={y}
                  stroke="#d6d3d1"
                  strokeDasharray="4 6"
                />
              );
            })}

            {elevationPoints.length > 1 ? (
              <g>
                <polygon
                  fill="currentColor"
                  className="text-success/15"
                  points={[
                    `${leftPadding},${height - bottomPadding}`,
                    ...elevationPoints.map(
                      (point) =>
                        `${toSvgX(point.x).toFixed(1)},${toElevationSvgY(point.y).toFixed(1)}`,
                    ),
                    `${width - rightPadding},${height - bottomPadding}`,
                  ].join(" ")}
                />
                <polyline
                  fill="none"
                  stroke="currentColor"
                  className="text-success/35"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  points={elevationPoints
                    .map(
                      (point) =>
                        `${toSvgX(point.x).toFixed(1)},${toElevationSvgY(point.y).toFixed(1)}`,
                    )
                    .join(" ")}
                />
              </g>
            ) : null}

            <text
              x={leftPadding}
              y={12}
              className="fill-current text-[10px] text-base-content/65"
            >
              {metricLabel(metric)}
            </text>
            <text
              x={width - rightPadding}
              y={12}
              textAnchor="end"
              className="fill-current text-[10px] text-base-content/65"
            >
              Elevation
            </text>
            <text
              x={leftPadding}
              y={topPadding - 4}
              className="fill-current text-[10px] text-base-content/60"
            >
              {formatMetricValue(metric, maxY)}
            </text>
            <text
              x={leftPadding}
              y={height - bottomPadding + 14}
              className="fill-current text-[10px] text-base-content/60"
            >
              {formatMetricValue(metric, minY)}
            </text>
            <text
              x={width - rightPadding}
              y={topPadding - 4}
              textAnchor="end"
              className="fill-current text-[10px] text-base-content/60"
            >
              {formatElevation(maxElevation)}
            </text>
            <text
              x={width - rightPadding}
              y={height - bottomPadding + 14}
              textAnchor="end"
              className="fill-current text-[10px] text-base-content/60"
            >
              {formatElevation(minElevation)}
            </text>
            <text
              x={width / 2}
              y={height - 6}
              textAnchor="middle"
              className="fill-current text-[10px] text-base-content/65"
            >
              Elapsed time
            </text>

            {series.map((entry) => {
              const path = entry.points
                .map(
                  (point) =>
                    `${toSvgX(point.x).toFixed(1)},${toSvgY(point.y).toFixed(1)}`,
                )
                .join(" ");
              const currentPoint = interpolateSeriesPoint(
                entry.points,
                displaySeconds,
              );

              return (
                <g key={entry.effort.id}>
                  <polyline
                    fill="none"
                    stroke={entry.color}
                    strokeWidth={metric === "speed" ? "2.5" : "3.5"}
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    points={path}
                  />
                  {currentPoint ? (
                    <circle
                      cx={toSvgX(currentPoint.x)}
                      cy={toSvgY(currentPoint.y)}
                      r="6"
                      fill={entry.color}
                    />
                  ) : null}
                </g>
              );
            })}

            {displaySeconds > 0 ? (
              <line
                x1={toSvgX(displaySeconds)}
                y1={topPadding}
                x2={toSvgX(displaySeconds)}
                y2={height - bottomPadding}
                stroke="#78716c"
                strokeDasharray="4 4"
              />
            ) : null}

            {[0, 0.5, 1].map((fraction) => {
              const value = maxX * fraction;
              return (
                <text
                  key={`x-tick-${fraction}`}
                  x={toSvgX(value)}
                  y={height - bottomPadding + 14}
                  textAnchor={fraction === 0 ? "start" : fraction === 1 ? "end" : "middle"}
                  className="fill-current text-[10px] text-base-content/60"
                >
                  {formatDuration(Math.round(value))}
                </text>
              );
            })}
          </svg>
        </div>

        <div className="mt-4 flex flex-wrap gap-3">
          <div className="card bg-base-200 shadow-sm">
            <div className="card-body p-3 text-sm">
              <div className="font-semibold text-base-content">Hover point</div>
              <div className="mt-1">
                {formatDuration(Math.round(displaySeconds))}
              </div>
              <div className="mt-1 text-base-content/70">
                Elevation {hoveredElevationPoint ? formatElevation(hoveredElevationPoint.y) : "--"}
              </div>
            </div>
          </div>
          {series.map((entry) => {
            const point = interpolateSeriesPoint(entry.points, displaySeconds);
            return (
              <div key={entry.effort.id} className="card bg-base-200 shadow-sm">
                <div className="card-body p-3 text-sm">
                  <div className="font-semibold text-base-content">
                    {entry.effort.activity_title}
                  </div>
                  <div className="mt-1">
                    {point ? formatMetricValue(metric, point.y) : "--"}
                  </div>
                </div>
              </div>
            );
          })}
        </div>
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
  const segmentQuery = useSegment(user ? segmentId : null);
  const [selectedEffortIds, setSelectedEffortIds] = useState<number[]>([]);
  const [playbackSeconds, setPlaybackSeconds] = useState(0);
  const [isPlaying, setIsPlaying] = useState(false);
  const [metric, setMetric] = useState<ChartMetric>("speed");
  const [effortTimeFilter, setEffortTimeFilter] =
    useState<EffortTimeFilter>("all");
  const segment = segmentQuery.data;
  const allEfforts = segment?.efforts ?? [];
  const visibleEfforts = filterEffortsByTimeWindow(
    segment?.efforts,
    effortTimeFilter,
  );
  const currentUserId = user?.id ?? null;
  const currentUserName = user?.name?.trim() || null;
  const overallKom = fastestEffort(allEfforts);
  const currentUserPr = currentUserId != null
    ? fastestEffort(
        allEfforts.filter((effort) => effort.rider_user_id === currentUserId),
      )
    : currentUserName
    ? fastestEffort(
        allEfforts.filter((effort) => effort.rider_name === currentUserName),
      )
    : allEfforts.length > 0 &&
        new Set(allEfforts.map((effort) => effort.rider_name)).size === 1
      ? fastestEffort(allEfforts)
      : null;
  const selectedEfforts = visibleEfforts.filter((effort) =>
    selectedEffortIds.includes(effort.id),
  );
  const maxDuration = selectedEfforts.reduce(
    (max, effort) => Math.max(max, effort.duration_seconds),
    0,
  );

  useEffect(() => {
    const efforts = filterEffortsByTimeWindow(
      segment?.efforts,
      effortTimeFilter,
    );
    if (efforts.length === 0) {
      setSelectedEffortIds([]);
      setPlaybackSeconds(0);
      setIsPlaying(false);
      return;
    }

    setSelectedEffortIds((current) => {
      const valid = current.filter((id) =>
        efforts.some((effort) => effort.id === id),
      );
      if (valid.length > 0) {
        return valid.slice(0, MAX_SELECTED_EFFORTS);
      }

      return efforts
        .slice(0, Math.min(3, efforts.length))
        .map((effort) => effort.id);
    });
  }, [segment?.id, segment?.efforts, effortTimeFilter]);

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

          <div className="stats stats-vertical bg-base-200 shadow lg:stats-horizontal">
            <div className="stat">
              <div className="stat-title">Distance</div>
              <div className="stat-value text-xl">
                {formatDistance(segment.distance_meters)}
              </div>
            </div>
            <div className="stat">
              <div className="stat-title">Overall KOM</div>
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
              <div className="stat-title">Your PR</div>
              <div className="stat-value text-xl">
                {formatDuration(currentUserPr?.duration_seconds ?? null)}
              </div>
              <div className="stat-desc">
                {currentUserPr ? currentUserPr.activity_title : "No PR yet"}
              </div>
            </div>
            <div className="stat">
              <div className="stat-title">Attempts</div>
              <div className="stat-value text-xl">{segment.effort_count}</div>
            </div>
            <div className="stat">
              <div className="stat-title">Source file</div>
              <div className="stat-value text-xl">
                {segment.original_filename ?? "--"}
              </div>
            </div>
          </div>
        </div>
      </div>

      <RouteComparisonMap
        routePoints={segment.route_points}
        selectedEfforts={selectedEfforts}
        playbackSeconds={playbackSeconds}
        maxDuration={maxDuration}
        isPlaying={isPlaying}
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

      <ComparisonChart
        metric={metric}
        routePoints={segment.route_points}
        selectedEfforts={selectedEfforts}
        playbackSeconds={playbackSeconds}
        onMetricChange={(value) => {
          setMetric(value);
        }}
      />

      <div className="card bg-base-200 shadow-sm">
        <div className="card-body">
          <h2 className="card-title text-xl">Efforts</h2>
          <p className="text-sm text-base-content/70">
            Select up to ten attempts, then use time to open the full activity
            detail.
          </p>

          <div className="mt-4 flex flex-wrap items-center justify-between gap-3 rounded-box bg-base-100 p-3">
            <div className="text-sm text-base-content/70">
              {visibleEfforts.length} of {(segment.efforts ?? []).length} efforts
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

          {visibleEfforts.length > 0 ? (
            <div className="mt-5 overflow-x-auto rounded-box bg-base-100">
              <table className="table table-sm">
                <thead>
                  <tr>
                    <th className="w-12"></th>
                    <th>Time</th>
                    <th>Rider</th>
                    <th>Date</th>
                  </tr>
                </thead>
                <tbody>
                  {visibleEfforts.map((effort) => {
                    const checked = selectedEffortIds.includes(effort.id);
                    const isOverallKom = overallKom?.id === effort.id;
                    const isCurrentUserPr = currentUserPr?.id === effort.id;
                    const rowClassName = isCurrentUserPr
                      ? "bg-primary/10"
                      : isOverallKom
                        ? "bg-warning/10"
                        : checked
                          ? "bg-base-200/70"
                          : undefined;

                    return (
                      <tr key={effort.id} className={rowClassName}>
                        <td>
                          <input
                            type="checkbox"
                            className="checkbox checkbox-sm"
                            checked={checked}
                            aria-label={`Select ${effort.activity_title}`}
                            onChange={(event) => {
                              const isChecked = event.target.checked;
                              setSelectedEffortIds((current) => {
                                if (isChecked) {
                                  if (current.includes(effort.id)) {
                                    return current;
                                  }

                                  return [...current, effort.id].slice(
                                    0,
                                    MAX_SELECTED_EFFORTS,
                                  );
                                }

                                return current.filter((id) => id !== effort.id);
                              });
                            }}
                          />
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
                            {isOverallKom ? (
                              <span className="badge badge-warning badge-xs">
                                KOM
                              </span>
                            ) : null}
                            {isCurrentUserPr ? (
                              <span className="badge badge-primary badge-xs">
                                PR
                              </span>
                            ) : null}
                          </div>
                        </td>
                        <td>{effort.rider_name}</td>
                        <td className="whitespace-nowrap text-base-content/65">
                          {formatActivityTimestamp(effort.activity_started_at)}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          ) : (segment.efforts ?? []).length > 0 ? (
            <div className="alert mt-5">
              <span>No efforts match the selected time window.</span>
            </div>
          ) : (
            <div className="alert mt-5">
              <span>
                No activities matched this segment yet. Import the route, then
                regenerate older activities if they were uploaded before route
                points were stored.
              </span>
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
