"use client";

import { auth } from "@ericbutera/kaleido";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useMemo, useState } from "react";
import {
  extractApiMessage,
  formatActivityTimestamp,
  formatCadence,
  formatCalories,
  formatDistance,
  formatDuration,
  formatElevation,
  formatHeartRate,
  formatSpeed,
  formatSport,
} from "../lib/activityFormatting";
import {
  type ActivityChartPoint,
  type ActivityLap,
  type ActivityRoutePoint,
  type ActivitySegmentEffort,
  useActivity,
  useDeleteActivity,
  useRegenerateActivity,
} from "../lib/queries";
import AuthRequiredCard from "./AuthRequiredCard";
import LeafletRouteMap from "./LeafletRouteMap";

type ChartSeriesPoint = {
  x: number;
  y: number;
};

type SignalMetricKey = "heartRate" | "speed" | "elevation";

type SegmentTone = {
  mapColor: string;
  dotClassName: string;
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
  lineClassName: string;
  dotClassName: string;
  strokeWidth?: number;
  areaClassName?: string;
  summary: string;
};

const SEGMENT_TONES: SegmentTone[] = [
  {
    mapColor: "var(--color-primary)",
    dotClassName: "bg-primary",
    buttonClassName: "btn-primary",
    outlineButtonClassName: "btn-outline btn-primary",
    highlightClassName: "ring-primary/25",
  },
  {
    mapColor: "var(--color-secondary)",
    dotClassName: "bg-secondary",
    buttonClassName: "btn-secondary",
    outlineButtonClassName: "btn-outline btn-secondary",
    highlightClassName: "ring-secondary/25",
  },
  {
    mapColor: "var(--color-accent)",
    dotClassName: "bg-accent",
    buttonClassName: "btn-accent",
    outlineButtonClassName: "btn-outline btn-accent",
    highlightClassName: "ring-accent/25",
  },
  {
    mapColor: "var(--color-info)",
    dotClassName: "bg-info",
    buttonClassName: "btn-info",
    outlineButtonClassName: "btn-outline btn-info",
    highlightClassName: "ring-info/25",
  },
  {
    mapColor: "var(--color-warning)",
    dotClassName: "bg-warning",
    buttonClassName: "btn-warning",
    outlineButtonClassName: "btn-outline btn-warning",
    highlightClassName: "ring-warning/25",
  },
];

const DEFAULT_VISIBLE_SIGNAL_KEYS: SignalMetricKey[] = [
  "heartRate",
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

function achievementLabel(rank: number | null | undefined) {
  if (rank === 1) {
    return "Fastest";
  }

  if (rank != null && rank <= 3) {
    return "Top 3";
  }

  return null;
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
  const threshold = Math.max(8, Math.round(firstEffort.duration_seconds * 0.03));

  if (delta <= -threshold) {
    return "faster";
  }

  if (delta >= threshold) {
    return "slower";
  }

  return "steady";
}

function averageHeartRateForSegmentEffort(
  routePoints: ActivityRoutePoint[] | null | undefined,
  segmentEffort: ActivitySegmentEffort,
) {
  const heartRateValues = segmentOverlayPoints(routePoints, segmentEffort)
    .flatMap((point) =>
      point.heart_rate_bpm == null || Number.isNaN(point.heart_rate_bpm)
        ? []
        : [point.heart_rate_bpm],
    );

  if (heartRateValues.length === 0) {
    return null;
  }

  const sum = heartRateValues.reduce((total, value) => total + value, 0);

  return Math.round(sum / heartRateValues.length);
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

  return Array.from(effortsBySegmentId.values()).map((efforts, index) => {
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
      const averageHeartRate = averageHeartRateForSegmentEffort(
        routePoints,
        effort,
      );

      if (averageHeartRate == null) {
        return peak;
      }

      return peak == null || averageHeartRate > peak ? averageHeartRate : peak;
    }, null);
    const hasHighHeartRate =
      peakHeartRate != null &&
      ((activityMaxHeartRate != null && peakHeartRate >= activityMaxHeartRate - 6) ||
        (activityAverageHeartRate != null &&
          peakHeartRate >= activityAverageHeartRate + 10));

    return {
      segmentId: efforts[0].segment_id,
      segmentTitle: efforts[0].segment_title,
      efforts,
      tone: SEGMENT_TONES[index % SEGMENT_TONES.length],
      bestEffort,
      bestOverallRank: ranks[0] ?? null,
      peakHeartRate,
      hasHighHeartRate,
      trendState: describeTrendState(efforts),
      anchorId: buildSegmentAnchorId(efforts[0].segment_id),
    };
  });
}

function scaleSeriesPoints(
  points: ChartSeriesPoint[],
  maxX: number,
  width: number,
  height: number,
  padding: number,
) {
  const minY = minSeriesValue(points);
  const maxY = maxSeriesValue(points);
  const xRange = Math.max(1, maxX);
  const yRange = Math.max(1, maxY - minY);

  return points.map((point) => ({
    x: padding + (point.x / xRange) * (width - padding * 2),
    y:
      height - padding - ((point.y - minY) / yRange) * (height - padding * 2),
  }));
}

function toPolylinePoints(points: Array<{ x: number; y: number }>) {
  return points.map((point) => `${point.x},${point.y}`).join(" ");
}

function toAreaPoints(
  points: Array<{ x: number; y: number }>,
  height: number,
  padding: number,
) {
  if (points.length === 0) {
    return "";
  }

  const baseline = height - padding;

  return [
    `${points[0].x},${baseline}`,
    ...points.map((point) => `${point.x},${point.y}`),
    `${points[points.length - 1].x},${baseline}`,
  ].join(" ");
}

function SignalChartCard({
  sampleCount,
  series,
  visibleKeys,
  onToggle,
}: {
  sampleCount: number;
  series: SignalSeries[];
  visibleKeys: SignalMetricKey[];
  onToggle: (key: SignalMetricKey) => void;
}) {
  const width = 320;
  const height = 176;
  const padding = 16;
  const availableSeries = series.filter((entry) => entry.points.length > 1);
  const visibleSeries = availableSeries.filter((entry) =>
    visibleKeys.includes(entry.key),
  );
  const maxX = visibleSeries.reduce(
    (maxValue, entry) =>
      Math.max(maxValue, entry.points[entry.points.length - 1]?.x ?? 0),
    0,
  );
  const endLabel = formatElapsedAxisLabel(maxX);
  const scaledSeries = visibleSeries.map((entry) => ({
    ...entry,
    scaledPoints: scaleSeriesPoints(entry.points, maxX, width, height, padding),
  }));

  return (
    <div className="card bg-base-100 shadow-xl">
      <div className="card-body">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <h2 className="card-title text-xl">Ride signals</h2>
            <p className="text-sm leading-6 text-base-content/70">
              Heart rate, speed, and elevation share the same time axis. Toggle
              layers to focus on effort, terrain, or both at once.
            </p>
          </div>
          <span className="badge badge-ghost uppercase">{sampleCount} samples</span>
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
            <svg
              viewBox={`0 0 ${width} ${height}`}
              preserveAspectRatio="none"
              className="mt-5 block h-56 w-full overflow-visible rounded-box border border-base-300 bg-base-200"
              role="img"
              aria-label="Activity signals chart"
            >
              <g className="text-base-content/10">
                {Array.from({ length: 4 }).map((_, index) => {
                  const y = padding + ((height - padding * 2) / 3) * index;

                  return (
                    <line
                      key={`grid-${index}`}
                      x1={padding}
                      y1={y}
                      x2={width - padding}
                      y2={y}
                      stroke="currentColor"
                    />
                  );
                })}
                <line
                  x1={padding}
                  y1={height - padding}
                  x2={width - padding}
                  y2={height - padding}
                  stroke="currentColor"
                />
              </g>

              {scaledSeries
                .filter((entry) => entry.areaClassName)
                .map((entry) => (
                  <g key={`${entry.key}-area`} className={entry.areaClassName}>
                    <polygon
                      fill="currentColor"
                      points={toAreaPoints(entry.scaledPoints, height, padding)}
                    />
                  </g>
                ))}

              {scaledSeries.map((entry) => (
                <g key={entry.key} className={entry.lineClassName}>
                  <polyline
                    fill="none"
                    stroke="currentColor"
                    strokeWidth={entry.strokeWidth ?? 3}
                    strokeLinejoin="round"
                    strokeLinecap="round"
                    points={toPolylinePoints(entry.scaledPoints)}
                  />
                </g>
              ))}
            </svg>

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

            <div className="mt-3 flex items-center justify-between text-sm text-base-content/60">
              <span>0m</span>
              <span>{endLabel}</span>
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

function LapCard({ lap }: { lap: ActivityLap }) {
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
            value={formatDistance(lap.distance_meters)}
          />
          <DetailMetric
            label="Average speed"
            value={formatSpeed(lap.average_speed_mps)}
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
              <LeafletRouteMap
                routePoints={routePoints}
                overlays={overlays}
                ariaLabel="Activity route map"
                emptyMessage="This activity does not have enough stored route points for the map yet."
                className="h-96 w-full"
              />
            </div>

            {segmentGroups.length > 0 ? (
              <div className="card-actions mt-4 gap-2">
                {segmentGroups.map((segmentGroup) => {
                  const isSelected = selectedSegmentId === segmentGroup.segmentId;

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
  const [visibleSignalKeys, setVisibleSignalKeys] = useState<SignalMetricKey[]>(
    DEFAULT_VISIBLE_SIGNAL_KEYS,
  );
  const authApi = auth.useAuthApi();
  const router = useRouter();
  const { user, isLoading: isLoadingUser } = authApi.useCurrentUser();
  const activityQuery = useActivity(user ? activityId : null);
  const regenerateMutation = useRegenerateActivity();
  const deleteMutation = useDeleteActivity();
  const activity = activityQuery.data;
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

  const heartRateSeries = buildSeries(
    activity?.chart_points,
    (point) => point.heart_rate_bpm,
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
        lineClassName: "text-error",
        dotClassName: "bg-error",
        strokeWidth: 3,
        summary: `Peak ${formatHeartRate(maxSeriesValue(heartRateSeries))}`,
      },
      {
        key: "speed",
        label: "Speed",
        points: speedSeries,
        buttonClassName: "btn-info",
        lineClassName: "text-info",
        dotClassName: "bg-info",
        strokeWidth: 2,
        summary: `Top speed ${formatSpeed(maxSeriesValue(speedSeries))}`,
      },
      {
        key: "elevation",
        label: "Elevation",
        points: elevationSeries,
        buttonClassName: "btn-success",
        lineClassName: "text-success/65",
        dotClassName: "bg-success",
        strokeWidth: 2.5,
        areaClassName: "text-success/15",
        summary: `${formatElevation(minSeriesValue(elevationSeries))} to ${formatElevation(maxSeriesValue(elevationSeries))}`,
      },
    ],
    [elevationSeries, heartRateSeries, speedSeries],
  );

  function focusSegmentMatch(segmentId: number) {
    setSelectedSegmentId(segmentId);

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

      return DEFAULT_VISIBLE_SIGNAL_KEYS.filter(
        (entry) => entry === key || current.includes(entry),
      );
    });
  }

  function trendLabel(state: SegmentTrendState | null) {
    switch (state) {
      case "faster":
        return "Trending faster";
      case "slower":
        return "Trending slower";
      case "steady":
        return "Steady pacing";
      default:
        return null;
    }
  }

  function trendBadgeClass(state: SegmentTrendState | null) {
    switch (state) {
      case "faster":
        return "badge-success";
      case "slower":
        return "badge-warning";
      case "steady":
        return "badge-ghost";
      default:
        return "badge-ghost";
    }
  }

  function renderMatchedSegmentInsights(segmentGroup: SegmentMatchGroup) {
    const segmentTrendLabel = trendLabel(segmentGroup.trendState);

    if (!segmentTrendLabel && !segmentGroup.hasHighHeartRate) {
      return (
        <div className="rounded-box border border-base-300 bg-base-100 px-4 py-3 text-sm text-base-content/70">
          Steady pacing through this segment.
        </div>
      );
    }

    return (
      <div className="rounded-box border border-base-300 bg-base-100 px-4 py-3 text-sm text-base-content/70">
        {segmentTrendLabel ?? null}
        {segmentTrendLabel && segmentGroup.hasHighHeartRate ? " · " : null}
        {segmentGroup.hasHighHeartRate && segmentGroup.peakHeartRate != null
          ? `High heart rate at ${formatHeartRate(segmentGroup.peakHeartRate)}`
          : null}
      </div>
    );
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
              <p className="text-sm text-base-content/60">Activity detail</p>
              <h1 className="mt-2 text-4xl font-semibold">{activity.title}</h1>
              <p className="mt-3 text-sm text-base-content/70">
                {formatActivityTimestamp(activity.started_at)}
              </p>
            </div>
            <div className="flex flex-wrap gap-2">
              <span className="badge badge-outline">
                {formatSport(activity.sport)}
              </span>
              {activity.format ? (
                <span className="badge badge-ghost uppercase">
                  {activity.format}
                </span>
              ) : null}
            </div>
          </div>

          <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
            <DetailMetric
              label="Distance"
              value={formatDistance(activity.distance_meters)}
            />
            <DetailMetric
              label="Moving time"
              value={formatDuration(
                activity.moving_time_seconds ?? activity.total_time_seconds,
              )}
            />
            <DetailMetric
              label="Average speed"
              value={formatSpeed(activity.average_speed_mps)}
            />
            <DetailMetric
              label="Max speed"
              value={formatSpeed(activity.max_speed_mps)}
            />
            <DetailMetric
              label="Elevation gain"
              value={formatElevation(activity.elevation_gain_meters)}
            />
            <DetailMetric
              label="Elevation loss"
              value={formatElevation(activity.elevation_loss_meters)}
            />
            <DetailMetric
              label="Average heart rate"
              value={formatHeartRate(activity.average_heart_rate_bpm)}
            />
            <DetailMetric
              label="Max heart rate"
              value={formatHeartRate(activity.max_heart_rate_bpm)}
            />
            <DetailMetric
              label="Average cadence"
              value={formatCadence(activity.average_cadence_rpm)}
            />
            <DetailMetric
              label="Max cadence"
              value={formatCadence(activity.max_cadence_rpm)}
            />
            <DetailMetric
              label="Total time"
              value={formatDuration(activity.total_time_seconds)}
            />
            <DetailMetric
              label="Calories"
              value={formatCalories(activity.calories)}
            />
          </div>
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

      <div className="card bg-base-100 shadow-xl">
        <div className="card-body">
          <div className="flex flex-wrap items-start justify-between gap-4">
            <div>
              <h2 className="card-title text-xl">Matched segments</h2>
              <p className="text-sm text-base-content/70">
                Segment efforts are matched from imported segment routes
                against the upload-time route geometry stored on this
                activity.
              </p>
            </div>
            <span className="badge badge-outline">
              {matchedSegmentGroups.length} matched segment
              {matchedSegmentGroups.length === 1 ? "" : "s"}
            </span>
          </div>

          {matchedSegmentGroups.length > 0 ? (
            <div className="mt-5 grid gap-4">
              {matchedSegmentGroups.map((segmentGroup) => {
                const segmentHref = `/segments/${segmentGroup.segmentId}`;
                const segmentAchievement = achievementLabel(
                  segmentGroup.bestOverallRank,
                );
                const isSelected = selectedSegmentId === segmentGroup.segmentId;
                const segmentTrendLabel = trendLabel(segmentGroup.trendState);

                return (
                  <article
                    id={segmentGroup.anchorId}
                    key={segmentGroup.segmentId}
                    className={`card bg-base-200 shadow-sm ring-1 transition ${isSelected ? segmentGroup.tone.highlightClassName : "ring-base-300/0"}`}
                  >
                    <div className="card-body gap-4">
                      <div className="flex flex-wrap items-start justify-between gap-4">
                        <div>
                          <Link
                            href={segmentHref}
                            className="card-title text-lg transition hover:text-primary"
                          >
                            {segmentGroup.segmentTitle}
                          </Link>
                        </div>

                        <div className="flex flex-wrap gap-2">
                          <span className="badge badge-outline">
                            {segmentGroup.efforts.length} run
                            {segmentGroup.efforts.length === 1 ? "" : "s"}
                          </span>
                          {segmentAchievement ? (
                            <span
                              className={`badge badge-outline ${segmentAchievement === "Fastest" ? "badge-success" : "badge-warning"}`}
                            >
                              {segmentAchievement}
                            </span>
                          ) : null}
                          {segmentTrendLabel ? (
                            <span
                              className={`badge badge-outline ${trendBadgeClass(segmentGroup.trendState)}`}
                            >
                              {segmentTrendLabel}
                            </span>
                          ) : null}
                          {segmentGroup.hasHighHeartRate ? (
                            <span className="badge badge-error badge-outline">
                              High heart rate
                            </span>
                          ) : null}
                        </div>
                      </div>

                      <div className="grid gap-3 md:grid-cols-3">
                        <DetailMetric
                          label="Best time"
                          value={formatDuration(segmentGroup.bestEffort.duration_seconds)}
                        />
                        <DetailMetric
                          label="Leaderboard"
                          value={formatOverallRank(segmentGroup.bestOverallRank)}
                        />
                        <DetailMetric
                          label="Peak segment HR"
                          value={formatHeartRate(segmentGroup.peakHeartRate)}
                        />
                      </div>

                      <ul className="menu rounded-box border border-base-300 bg-base-100 p-2">
                        {segmentGroup.efforts.map((segmentEffort) => {
                          const effortAchievement = achievementLabel(
                            segmentEffort.overall_rank,
                          );

                          return (
                            <li
                              key={`${segmentEffort.segment_id}-${segmentEffort.effort_index}-${segmentEffort.start_route_point_index}-${segmentEffort.end_route_point_index}`}
                            >
                              <Link
                                href={segmentHref}
                                className="flex items-center justify-between gap-3"
                                title={segmentGroup.segmentTitle}
                              >
                                <span className="font-semibold text-base-content">
                                  {formatDuration(segmentEffort.duration_seconds)}
                                </span>
                                <span className="flex flex-wrap items-center justify-end gap-2 text-sm text-base-content/70">
                                  {segmentEffort.overall_rank != null ? (
                                    <span className="badge badge-ghost badge-xs">
                                      #{segmentEffort.overall_rank}
                                    </span>
                                  ) : null}
                                  {effortAchievement ? (
                                    <span
                                      className={`badge badge-xs ${effortAchievement === "Fastest" ? "badge-success" : "badge-warning"}`}
                                    >
                                      {effortAchievement}
                                    </span>
                                  ) : null}
                                </span>
                              </Link>
                            </li>
                          );
                        })}
                      </ul>

                      {renderMatchedSegmentInsights(segmentGroup)}
                    </div>
                  </article>
                );
              })}
            </div>
          ) : (
            <div className="alert mt-5">
              <span>
                No imported segment routes have matched this activity yet. If
                this is an older upload, regenerate it once so the latest
                route geometry and matcher run against the raw file.
              </span>
            </div>
          )}
        </div>
      </div>

      <SignalChartCard
        sampleCount={(activity.chart_points ?? []).length}
        series={signalSeries}
        visibleKeys={visibleSignalKeys}
        onToggle={toggleSignalLayer}
      />

      <div className="grid gap-6 lg:grid-cols-[minmax(0,1.3fr)_minmax(18rem,0.7fr)] lg:items-start">
        <div className="card bg-base-100 shadow-xl">
          <div className="card-body">
            <div className="flex flex-wrap items-start justify-between gap-4">
              <div>
                <h2 className="card-title text-xl">Lap splits</h2>
                <p className="text-sm text-base-content/70">
                  These lap rollups come from the upload-time read side and
                  can be regenerated when the development flag is enabled.
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
                  <LapCard key={`${lap.lap_index}-${lap.title}`} lap={lap} />
                ))}
              </div>
            ) : (
              <div className="alert mt-5">
                <span>This upload did not contain explicit lap data.</span>
              </div>
            )}
          </div>
        </div>

        <aside className="card bg-base-100 shadow-xl">
          <div className="card-body">
            <h2 className="card-title text-xl">Source metadata</h2>
            <dl className="space-y-4 text-sm">
              <div>
                <dt className="font-semibold text-base-content">Source</dt>
                <dd className="mt-1 text-base-content/70">{activity.source}</dd>
              </div>
              <div>
                <dt className="font-semibold text-base-content">
                  Uploaded file
                </dt>
                <dd className="mt-1 text-base-content/70">
                  {activity.original_filename ?? "--"}
                </dd>
              </div>
              <div>
                <dt className="font-semibold text-base-content">Ended at</dt>
                <dd className="mt-1 text-base-content/70">
                  {activity.ended_at
                    ? formatActivityTimestamp(activity.ended_at)
                    : "--"}
                </dd>
              </div>
            </dl>

            {activity.can_regenerate ? (
              <div className="mt-6 border-t border-base-300 pt-5">
                <button
                  type="button"
                  className="btn btn-outline btn-sm"
                  onClick={() => {
                    void handleRegenerate();
                  }}
                  disabled={regenerateMutation.isPending}
                >
                  {regenerateMutation.isPending
                    ? "Regenerating..."
                    : "Regenerate derived data"}
                </button>
                <p className="mt-3 text-sm leading-6 text-base-content/65">
                  Re-runs the same upload-time derivation against the raw file
                  so parser changes, route geometry, and segment matches show
                  up without re-uploading.
                </p>
                {regenerateMutation.isError ? (
                  <div className="alert alert-error mt-3">
                    {extractApiMessage(regenerateMutation.error)}
                  </div>
                ) : null}
              </div>
            ) : null}

            <div className="mt-6 border-t border-base-300 pt-5">
              <button
                type="button"
                className="btn btn-outline btn-error btn-sm"
                onClick={() => {
                  void handleDelete();
                }}
                disabled={deleteMutation.isPending}
              >
                {deleteMutation.isPending ? "Deleting..." : "Delete activity"}
              </button>
              <p className="mt-3 text-sm leading-6 text-base-content/65">
                Removes this activity and clears any derived segment matches
                that depend on it.
              </p>
              {deleteMutation.isError ? (
                <div className="alert alert-error mt-3">
                  {extractApiMessage(deleteMutation.error)}
                </div>
              ) : null}
            </div>
          </div>
        </aside>
      </div>
    </section>
  );
}
