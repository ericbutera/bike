import { type ActivityRoutePoint } from "./queries";

const CLIMB_MIN_GRADE_PERCENT = 3;
const CLIMB_MIN_GAIN_METERS = 20;
const CLIMB_MIN_DURATION_SECONDS = 60;
const CLIMB_MIN_DISTANCE_METERS = 250;
const CLIMB_MAX_GAP_DISTANCE_METERS = 150;
const CLIMB_MAX_GAP_LOSS_METERS = 10;
const MAX_GRADE_WINDOW_METERS = 60;

export type ActivityClimbCategory = 1 | 2 | 3 | 4 | "HC";

export type ActivityClimb = {
  id: string;
  sequence: number;
  startRoutePointIndex: number;
  endRoutePointIndex: number;
  startDistanceMeters: number;
  endDistanceMeters: number;
  distanceMeters: number;
  elevationGainMeters: number;
  elevationLossMeters: number;
  avgGradePercent: number;
  maxGradePercent: number;
  category: ActivityClimbCategory | null;
  durationSeconds: number | null;
  routePoints: ActivityRoutePoint[];
};

type ClimbCandidate = {
  startIndex: number;
  lastQualifyingEndIndex: number;
  gapDistanceMeters: number;
  gapLossMeters: number;
};

type RouteInterval = {
  startIndex: number;
  endIndex: number;
  deltaDistanceMeters: number;
  deltaElevationMeters: number;
  deltaElapsedSeconds: number;
  gradePercent: number;
};

function finiteNumber(value: number | null | undefined) {
  return typeof value === "number" && Number.isFinite(value);
}

function routeInterval(
  routePoints: ActivityRoutePoint[],
  startIndex: number,
): RouteInterval | null {
  const previous = routePoints[startIndex];
  const current = routePoints[startIndex + 1];

  if (!previous || !current) {
    return null;
  }

  if (
    !finiteNumber(previous.distance_meters) ||
    !finiteNumber(current.distance_meters) ||
    !finiteNumber(previous.elevation_meters) ||
    !finiteNumber(current.elevation_meters)
  ) {
    return null;
  }

  const deltaDistanceMeters =
    (current.distance_meters as number) - (previous.distance_meters as number);
  const deltaElevationMeters =
    (current.elevation_meters as number) -
    (previous.elevation_meters as number);
  const deltaElapsedSeconds =
    current.elapsed_seconds - previous.elapsed_seconds;

  if (deltaDistanceMeters <= 0 || deltaElapsedSeconds <= 0) {
    return null;
  }

  return {
    startIndex,
    endIndex: startIndex + 1,
    deltaDistanceMeters,
    deltaElevationMeters,
    deltaElapsedSeconds,
    gradePercent: (deltaElevationMeters / deltaDistanceMeters) * 100,
  };
}

function isQualifyingClimbInterval(interval: RouteInterval) {
  return (
    interval.deltaElevationMeters > 0 &&
    interval.gradePercent >= CLIMB_MIN_GRADE_PERCENT
  );
}

function climbCategory(
  distanceMeters: number,
  avgGradePercent: number,
): ActivityClimbCategory | null {
  if (
    distanceMeters < CLIMB_MIN_DISTANCE_METERS ||
    avgGradePercent < CLIMB_MIN_GRADE_PERCENT
  ) {
    return null;
  }

  const climbScore = distanceMeters * avgGradePercent;

  if (climbScore >= 64_000) {
    return "HC";
  }

  if (climbScore >= 48_000) {
    return 1;
  }

  if (climbScore >= 32_000) {
    return 2;
  }

  if (climbScore >= 16_000) {
    return 3;
  }

  if (climbScore >= 8_000) {
    return 4;
  }

  return null;
}

function climbMaxGradePercent(points: ActivityRoutePoint[]) {
  let maxGradePercent = 0;
  let hasWindowGrade = false;

  for (let startIndex = 0; startIndex < points.length - 1; startIndex += 1) {
    const start = points[startIndex];

    if (
      !finiteNumber(start.distance_meters) ||
      !finiteNumber(start.elevation_meters)
    ) {
      continue;
    }

    for (let endIndex = startIndex + 1; endIndex < points.length; endIndex += 1) {
      const end = points[endIndex];

      if (
        !finiteNumber(end.distance_meters) ||
        !finiteNumber(end.elevation_meters)
      ) {
        continue;
      }

      const distanceMeters =
        (end.distance_meters as number) - (start.distance_meters as number);

      if (distanceMeters < MAX_GRADE_WINDOW_METERS) {
        continue;
      }

      const gradePercent =
        (((end.elevation_meters as number) -
          (start.elevation_meters as number)) /
          distanceMeters) *
        100;

      if (Number.isFinite(gradePercent)) {
        hasWindowGrade = true;
        maxGradePercent = Math.max(maxGradePercent, gradePercent);
      }

      break;
    }
  }

  if (hasWindowGrade) {
    return Math.max(0, maxGradePercent);
  }

  for (let index = 1; index < points.length; index += 1) {
    const interval = routeInterval(points, index - 1);

    if (interval) {
      maxGradePercent = Math.max(maxGradePercent, interval.gradePercent);
    }
  }

  return Math.max(0, maxGradePercent);
}

function climbElevationTotals(points: ActivityRoutePoint[]) {
  let elevationGainMeters = 0;
  let elevationLossMeters = 0;

  for (let index = 1; index < points.length; index += 1) {
    const previous = points[index - 1];
    const current = points[index];

    if (
      !finiteNumber(previous.elevation_meters) ||
      !finiteNumber(current.elevation_meters)
    ) {
      continue;
    }

    const deltaElevationMeters =
      (current.elevation_meters as number) -
      (previous.elevation_meters as number);

    if (deltaElevationMeters >= 0) {
      elevationGainMeters += deltaElevationMeters;
    } else {
      elevationLossMeters += Math.abs(deltaElevationMeters);
    }
  }

  return { elevationGainMeters, elevationLossMeters };
}

function buildClimbFromCandidate(
  routePoints: ActivityRoutePoint[],
  candidate: ClimbCandidate,
  sequence: number,
): ActivityClimb | null {
  const startPoint = routePoints[candidate.startIndex];
  const endPoint = routePoints[candidate.lastQualifyingEndIndex];

  if (
    !startPoint ||
    !endPoint ||
    !finiteNumber(startPoint.distance_meters) ||
    !finiteNumber(endPoint.distance_meters)
  ) {
    return null;
  }

  const distanceMeters =
    (endPoint.distance_meters as number) -
    (startPoint.distance_meters as number);
  const durationSeconds = Math.max(
    0,
    endPoint.elapsed_seconds - startPoint.elapsed_seconds,
  );
  const climbRoutePoints = routePoints.slice(
    candidate.startIndex,
    candidate.lastQualifyingEndIndex + 1,
  );
  const { elevationGainMeters, elevationLossMeters } =
    climbElevationTotals(climbRoutePoints);
  const avgGradePercent =
    distanceMeters > 0 ? (elevationGainMeters / distanceMeters) * 100 : 0;

  if (
    durationSeconds < CLIMB_MIN_DURATION_SECONDS ||
    distanceMeters < CLIMB_MIN_DISTANCE_METERS ||
    elevationGainMeters < CLIMB_MIN_GAIN_METERS ||
    avgGradePercent < CLIMB_MIN_GRADE_PERCENT
  ) {
    return null;
  }

  return {
    id: `climb-${sequence}`,
    sequence,
    startRoutePointIndex: candidate.startIndex,
    endRoutePointIndex: candidate.lastQualifyingEndIndex,
    startDistanceMeters: startPoint.distance_meters as number,
    endDistanceMeters: endPoint.distance_meters as number,
    distanceMeters,
    elevationGainMeters,
    elevationLossMeters,
    avgGradePercent,
    maxGradePercent: climbMaxGradePercent(climbRoutePoints),
    category: climbCategory(distanceMeters, avgGradePercent),
    durationSeconds,
    routePoints: climbRoutePoints,
  };
}

function canKeepClimbGap(candidate: ClimbCandidate, interval: RouteInterval) {
  const nextGapDistanceMeters =
    candidate.gapDistanceMeters + interval.deltaDistanceMeters;
  const nextGapLossMeters =
    candidate.gapLossMeters + Math.max(0, -interval.deltaElevationMeters);

  return (
    nextGapDistanceMeters <= CLIMB_MAX_GAP_DISTANCE_METERS &&
    nextGapLossMeters <= CLIMB_MAX_GAP_LOSS_METERS
  );
}

export function buildActivityClimbs(
  routePoints: ActivityRoutePoint[] | null | undefined,
) {
  const points = routePoints ?? [];

  if (points.length < 2) {
    return [] as ActivityClimb[];
  }

  const climbs: ActivityClimb[] = [];
  let candidate: ClimbCandidate | null = null;

  const finalizeCandidate = () => {
    if (!candidate) {
      return;
    }

    const nextClimb = buildClimbFromCandidate(
      points,
      candidate,
      climbs.length + 1,
    );

    if (nextClimb) {
      climbs.push(nextClimb);
    }

    candidate = null;
  };

  for (let index = 0; index < points.length - 1; index += 1) {
    const interval = routeInterval(points, index);

    if (!interval) {
      finalizeCandidate();
      continue;
    }

    if (isQualifyingClimbInterval(interval)) {
      if (!candidate) {
        candidate = {
          startIndex: interval.startIndex,
          lastQualifyingEndIndex: interval.endIndex,
          gapDistanceMeters: 0,
          gapLossMeters: 0,
        };
      } else {
        candidate.lastQualifyingEndIndex = interval.endIndex;
        candidate.gapDistanceMeters = 0;
        candidate.gapLossMeters = 0;
      }

      continue;
    }

    if (!candidate) {
      continue;
    }

    if (canKeepClimbGap(candidate, interval)) {
      candidate.gapDistanceMeters += interval.deltaDistanceMeters;
      candidate.gapLossMeters += Math.max(0, -interval.deltaElevationMeters);
      continue;
    }

    finalizeCandidate();
  }

  finalizeCandidate();

  return climbs;
}
