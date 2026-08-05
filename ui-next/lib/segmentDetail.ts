import { formatSpeed, type UnitSystem } from "./activityFormatting";
import type { ActivityRoutePoint, SegmentEffort } from "./queries";

export const EFFORT_COLORS = [
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

export const EFFORTS_PER_PAGE = 25;
export const EMPTY_EFFORTS: SegmentEffort[] = [];
export const EMPTY_EFFORT_IDS: number[] = [];
export const AUTO_PLAYBACK_MIN_SECONDS = 25;
export const PLAYBACK_TARGET_MAX_SECONDS = 120;
export const PLAYBACK_TARGET_MIN_SECONDS = 15;
export const PLAYBACK_END_EPSILON = 0.0001;
export const ATHLETE_PANEL_ROW_ANIMATION_MS = 220;

const WEB_MERCATOR_METERS_PER_PIXEL_AT_ZOOM_0 = 156543.03392;
const FOLLOW_PAIR_VIEWPORT_PADDING_FACTOR = 1.8;
const FOLLOW_PAIR_VIEWPORT_TARGET_SPAN_PX = 220;
const FOLLOW_PAIR_MIN_ZOOM = 12;

export const PLAYBACK_PACE_OPTIONS = [
  { key: "detail", label: "Slow", multiplier: 1.5 },
  { key: "auto", label: "Auto", multiplier: 1 },
  { key: "fast", label: "Fast", multiplier: 0.65 },
] as const;

export const RACE_PLAYBACK_SPEED_OPTIONS = [
  { value: 0.1, label: "0.10x" },
  { value: 0.25, label: "0.25x" },
  { value: 0.5, label: "0.5x" },
  { value: 0.75, label: "0.75x" },
  { value: 1, label: "1x" },
  { value: 1.25, label: "1.25x" },
  { value: 1.5, label: "1.5x" },
  { value: 2, label: "2x" },
  { value: 3, label: "3x" },
  { value: 4, label: "4x" },
] as const;

export const EFFORT_TIME_FILTERS = [
  { key: "all", label: "All" },
  { key: "day", label: "Day" },
  { key: "week", label: "Week" },
  { key: "month", label: "Month" },
  { key: "year", label: "Year" },
] as const;

export type EffortTimeFilter = (typeof EFFORT_TIME_FILTERS)[number]["key"];
export type PlaybackPace = (typeof PLAYBACK_PACE_OPTIONS)[number]["key"];
export type RacePlaybackSpeed =
  (typeof RACE_PLAYBACK_SPEED_OPTIONS)[number]["value"];

export type SelectedEffortRow = {
  effort: SegmentEffort;
  color: string;
  markerLabel: string;
};

export type SegmentEffortDayAttemptSummary = {
  attemptNumber: number;
  attemptCount: number;
  isFastestOfDay: boolean;
};

export type LiveComparisonRow = SelectedEffortRow & {
  currentPoint: ActivityRoutePoint | null;
  gapSeconds: number | null;
  speedDeltaMps: number | null;
  isReference: boolean;
  progress: number | null;
  isFinished: boolean;
};

export type GapChartRow = {
  progress: number;
  distanceMeters: number;
  elevation?: number | null;
  [key: string]: number | null | undefined;
};

function firstRouteParamValue(value: string | string[] | undefined) {
  return Array.isArray(value) ? value[0] : value;
}

export function parseSelectedEffortIdsParam(
  value: string | string[] | undefined,
) {
  const raw = firstRouteParamValue(value);

  if (!raw) {
    return [] as number[];
  }

  return raw
    .split(",")
    .map((entry) => Number(entry.trim()))
    .filter((entry) => Number.isFinite(entry) && entry > 0);
}

export function parseOptionalPositiveNumberParam(
  value: string | string[] | undefined,
) {
  const numericValue = Number(firstRouteParamValue(value));

  return Number.isFinite(numericValue) && numericValue > 0
    ? numericValue
    : null;
}

export function parsePlaybackPaceParam(
  value: string | string[] | undefined,
): PlaybackPace | undefined {
  const nextValue = firstRouteParamValue(value);

  return nextValue === "detail" || nextValue === "auto" || nextValue === "fast"
    ? nextValue
    : undefined;
}

export function parseRacePlaybackSpeedParam(
  value: string | string[] | undefined,
): RacePlaybackSpeed | undefined {
  const numericValue = Number(firstRouteParamValue(value));

  return RACE_PLAYBACK_SPEED_OPTIONS.some(
    (option) => option.value === numericValue,
  )
    ? (numericValue as RacePlaybackSpeed)
    : undefined;
}

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

export function filterEffortsByTimeWindow(
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

export function fastestEffort(efforts: SegmentEffort[] | null | undefined) {
  return (efforts ?? []).reduce<SegmentEffort | null>((best, effort) => {
    if (!best || effort.duration_seconds < best.duration_seconds) {
      return effort;
    }

    return best;
  }, null);
}

export function overallEffortRanks(
  efforts: SegmentEffort[] | null | undefined,
) {
  const ranked = [...(efforts ?? [])].sort(
    (left, right) =>
      left.duration_seconds - right.duration_seconds || left.id - right.id,
  );

  return new Map(ranked.map((effort, index) => [effort.id, index + 1]));
}

function effortLocalDayKey(value: string) {
  const date = new Date(value);

  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return [
    date.getFullYear(),
    String(date.getMonth() + 1).padStart(2, "0"),
    String(date.getDate()).padStart(2, "0"),
  ].join("-");
}

export function segmentEffortDayAttemptSummaries(
  efforts: SegmentEffort[] | null | undefined,
) {
  const effortsByRiderDay = new Map<string, SegmentEffort[]>();

  for (const effort of efforts ?? []) {
    const key = `${effort.rider_user_id}:${effortLocalDayKey(effort.activity_started_at)}`;
    const existing = effortsByRiderDay.get(key);

    if (existing) {
      existing.push(effort);
      continue;
    }

    effortsByRiderDay.set(key, [effort]);
  }

  const summaries = new Map<number, SegmentEffortDayAttemptSummary>();

  for (const group of effortsByRiderDay.values()) {
    const orderedAttempts = [...group].sort(
      (left, right) =>
        new Date(left.activity_started_at).getTime() -
          new Date(right.activity_started_at).getTime() ||
        left.start_elapsed_seconds - right.start_elapsed_seconds ||
        left.effort_index - right.effort_index ||
        left.id - right.id,
    );
    const fastestAttempt = [...group].sort(
      (left, right) =>
        left.duration_seconds - right.duration_seconds || left.id - right.id,
    )[0];

    orderedAttempts.forEach((effort, index) => {
      summaries.set(effort.id, {
        attemptNumber: index + 1,
        attemptCount: group.length,
        isFastestOfDay: group.length > 1 && effort.id === fastestAttempt.id,
      });
    });
  }

  return summaries;
}

export function areEffortIdListsEqual(left: number[], right: number[]) {
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

export function interpolateRoutePoint(
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

export function interpolateRoutePointByProgress(
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

export function effortProgressAtElapsed(
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

export function comparisonMarkerPoint(
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

export function buildLeaderPairFollowViewport(
  leaderPoint: ActivityRoutePoint | null,
  runnerUpPoint: ActivityRoutePoint | null,
  maxZoom: number,
) {
  if (!leaderPoint) {
    return null;
  }

  if (!runnerUpPoint) {
    return {
      point: leaderPoint,
      zoom: maxZoom,
    };
  }

  const centerLatitude = (leaderPoint.latitude + runnerUpPoint.latitude) / 2;
  const centerLongitude = (leaderPoint.longitude + runnerUpPoint.longitude) / 2;
  const latitudeRadians = (centerLatitude * Math.PI) / 180;
  const longitudeScale = Math.max(Math.cos(latitudeRadians), 0.01);
  const latitudeSpreadMeters =
    Math.abs(leaderPoint.latitude - runnerUpPoint.latitude) * 111_320;
  const longitudeSpreadMeters =
    Math.abs(leaderPoint.longitude - runnerUpPoint.longitude) *
    111_320 *
    longitudeScale;
  const spreadMeters = Math.max(latitudeSpreadMeters, longitudeSpreadMeters);

  if (spreadMeters <= 1) {
    return {
      point: leaderPoint,
      zoom: maxZoom,
    };
  }

  const paddedSpreadMeters = spreadMeters * FOLLOW_PAIR_VIEWPORT_PADDING_FACTOR;
  const metersPerPixelAtZoom0 =
    WEB_MERCATOR_METERS_PER_PIXEL_AT_ZOOM_0 * longitudeScale;
  const computedZoom = Math.log2(
    (metersPerPixelAtZoom0 * FOLLOW_PAIR_VIEWPORT_TARGET_SPAN_PX) /
      paddedSpreadMeters,
  );
  const zoom = Number.isFinite(computedZoom)
    ? Math.min(maxZoom, Math.max(FOLLOW_PAIR_MIN_ZOOM, computedZoom))
    : maxZoom;

  return {
    point: {
      ...leaderPoint,
      latitude: centerLatitude,
      longitude: centerLongitude,
    },
    zoom,
  };
}

export function buildLeaderGroupFollowViewport(
  points: ActivityRoutePoint[],
  maxZoom: number,
) {
  const firstPoint = points[0];

  if (!firstPoint) {
    return null;
  }

  if (points.length === 1) {
    return {
      point: firstPoint,
      zoom: maxZoom,
    };
  }

  const latitudeValues = points.map((point) => point.latitude);
  const longitudeValues = points.map((point) => point.longitude);
  const minLatitude = Math.min(...latitudeValues);
  const maxLatitude = Math.max(...latitudeValues);
  const minLongitude = Math.min(...longitudeValues);
  const maxLongitude = Math.max(...longitudeValues);
  const centerLatitude = (minLatitude + maxLatitude) / 2;
  const centerLongitude = (minLongitude + maxLongitude) / 2;
  const latitudeRadians = (centerLatitude * Math.PI) / 180;
  const longitudeScale = Math.max(Math.cos(latitudeRadians), 0.01);
  const latitudeSpreadMeters = Math.abs(maxLatitude - minLatitude) * 111_320;
  const longitudeSpreadMeters =
    Math.abs(maxLongitude - minLongitude) * 111_320 * longitudeScale;
  const spreadMeters = Math.max(latitudeSpreadMeters, longitudeSpreadMeters);

  if (spreadMeters <= 1) {
    return {
      point: firstPoint,
      zoom: maxZoom,
    };
  }

  const paddedSpreadMeters = spreadMeters * FOLLOW_PAIR_VIEWPORT_PADDING_FACTOR;
  const metersPerPixelAtZoom0 =
    WEB_MERCATOR_METERS_PER_PIXEL_AT_ZOOM_0 * longitudeScale;
  const computedZoom = Math.log2(
    (metersPerPixelAtZoom0 * FOLLOW_PAIR_VIEWPORT_TARGET_SPAN_PX) /
      paddedSpreadMeters,
  );
  const zoom = Number.isFinite(computedZoom)
    ? Math.min(maxZoom, Math.max(FOLLOW_PAIR_MIN_ZOOM, computedZoom))
    : maxZoom;

  return {
    point: {
      ...firstPoint,
      latitude: centerLatitude,
      longitude: centerLongitude,
    },
    zoom,
  };
}

export function resolveRouteDistanceMeters(
  routePoints: ActivityRoutePoint[] | null | undefined,
  fallbackDistanceMeters: number | null | undefined,
) {
  return (
    distanceRange(routePoints)?.totalDistance ?? fallbackDistanceMeters ?? null
  );
}

export function resolveRouteNetElevationMeters(
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

export function formatGradePercent(value: number | null | undefined) {
  if (value == null || Number.isNaN(value)) {
    return "-- grade";
  }

  const rounded =
    Math.abs(value) >= 10 ? Math.round(value) : Number(value.toFixed(1));

  return `${rounded > 0 ? "+" : ""}${rounded}% grade`;
}

export function formatSignedSecondsDelta(value: number | null | undefined) {
  if (value == null || Number.isNaN(value)) {
    return "--";
  }

  const rounded = Math.round(value);

  if (rounded === 0) {
    return "0s";
  }

  return `${rounded > 0 ? "+" : ""}${rounded}s`;
}

export function formatSignedSpeedDelta(
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

export function playbackTargetSeconds(
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

export function effortSeriesDataKey(effortId: number) {
  return `effort_${effortId}`;
}

export function buildGapChartRowAtProgress(
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

export function buildGapChartRows(
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

export function buildPlaybackGapMarker(
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

export function buildLiveComparisonRows(
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

export function sortLiveComparisonRowsByLeader(
  comparisonRows: LiveComparisonRow[],
) {
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
}
