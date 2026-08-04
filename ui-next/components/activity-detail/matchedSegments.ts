import { useMemo } from "react";
import {
  type Activity,
  type ActivityRoutePoint,
  type ActivitySegmentEffort,
} from "../../lib/queries";

export type SegmentTone = {
  mapColor: string;
  dotClassName: string;
  chartClassName: string;
  buttonClassName: string;
  outlineButtonClassName: string;
  highlightClassName: string;
};

export type SegmentTrendState = "faster" | "slower" | "steady";

export type MatchedSegmentGroup = {
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

function clampRouteIndex(index: number, routeLength: number) {
  if (routeLength <= 0) {
    return 0;
  }

  return Math.max(0, Math.min(index, routeLength - 1));
}

export function segmentOverlayPoints(
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

export function buildSegmentAnchorId(segmentId: number) {
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
): MatchedSegmentGroup[] {
  const effortsBySegmentId = new Map<number, ActivitySegmentEffort[]>();

  for (const effort of segmentEfforts ?? []) {
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

export function useMatchedSegmentGroups(activity: Activity | null | undefined) {
  const matchedSegmentEfforts = useMemo(
    () => sortMatchedSegmentEfforts(activity?.segment_efforts),
    [activity?.segment_efforts],
  );

  return useMemo(
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
}
