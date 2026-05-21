import { type ActivityRoutePoint } from "./queries";

export type SegmentBuilderSelection = {
  startIndex: number;
  endIndex: number;
};

function routePointCount(routePoints: ActivityRoutePoint[] | null | undefined) {
  return routePoints?.length ?? 0;
}

function normalizeDistanceMeters(
  distanceMeters: number | null | undefined,
  startDistanceMeters: number | null | undefined,
) {
  if (distanceMeters == null) {
    return null;
  }

  if (startDistanceMeters == null) {
    return distanceMeters;
  }

  return Math.max(0, distanceMeters - startDistanceMeters);
}

export function hasSegmentBuilderRoute(
  routePoints: ActivityRoutePoint[] | null | undefined,
) {
  return routePointCount(routePoints) >= 2;
}

export function buildInitialSegmentSelection(
  routePoints: ActivityRoutePoint[] | null | undefined,
): SegmentBuilderSelection {
  const totalPoints = routePointCount(routePoints);

  if (totalPoints < 2) {
    return { startIndex: 0, endIndex: 0 };
  }

  return {
    startIndex: 0,
    endIndex: totalPoints - 1,
  };
}

export function clampStartIndex(
  routePoints: ActivityRoutePoint[] | null | undefined,
  nextStartIndex: number,
  endIndex: number,
) {
  const totalPoints = routePointCount(routePoints);

  if (totalPoints < 2) {
    return 0;
  }

  const maxStartIndex = Math.max(0, Math.min(endIndex - 1, totalPoints - 2));

  return Math.max(0, Math.min(nextStartIndex, maxStartIndex));
}

export function clampEndIndex(
  routePoints: ActivityRoutePoint[] | null | undefined,
  startIndex: number,
  nextEndIndex: number,
) {
  const totalPoints = routePointCount(routePoints);

  if (totalPoints < 2) {
    return 0;
  }

  const minEndIndex = Math.min(totalPoints - 1, Math.max(startIndex + 1, 1));

  return Math.max(minEndIndex, Math.min(nextEndIndex, totalPoints - 1));
}

export function shiftStartIndex(
  routePoints: ActivityRoutePoint[] | null | undefined,
  selection: SegmentBuilderSelection,
  delta: number,
): SegmentBuilderSelection {
  return {
    startIndex: clampStartIndex(
      routePoints,
      selection.startIndex + delta,
      selection.endIndex,
    ),
    endIndex: selection.endIndex,
  };
}

export function shiftEndIndex(
  routePoints: ActivityRoutePoint[] | null | undefined,
  selection: SegmentBuilderSelection,
  delta: number,
): SegmentBuilderSelection {
  return {
    startIndex: selection.startIndex,
    endIndex: clampEndIndex(
      routePoints,
      selection.startIndex,
      selection.endIndex + delta,
    ),
  };
}

export function sliceSegmentRoutePoints(
  routePoints: ActivityRoutePoint[] | null | undefined,
  selection: SegmentBuilderSelection,
) {
  if (!hasSegmentBuilderRoute(routePoints)) {
    return [] as ActivityRoutePoint[];
  }

  const points = routePoints ?? [];
  const startIndex = clampStartIndex(
    points,
    selection.startIndex,
    selection.endIndex,
  );
  const endIndex = clampEndIndex(points, startIndex, selection.endIndex);
  const startPoint = points[startIndex];

  return points.slice(startIndex, endIndex + 1).map((point) => ({
    ...point,
    elapsed_seconds: point.elapsed_seconds - startPoint.elapsed_seconds,
    distance_meters: normalizeDistanceMeters(
      point.distance_meters,
      startPoint.distance_meters,
    ),
  }));
}

export function segmentSelectionDurationSeconds(
  routePoints: ActivityRoutePoint[] | null | undefined,
  selection: SegmentBuilderSelection,
) {
  if (!hasSegmentBuilderRoute(routePoints)) {
    return null;
  }

  const points = routePoints ?? [];
  const startIndex = clampStartIndex(
    points,
    selection.startIndex,
    selection.endIndex,
  );
  const endIndex = clampEndIndex(points, startIndex, selection.endIndex);

  return Math.max(
    0,
    points[endIndex].elapsed_seconds - points[startIndex].elapsed_seconds,
  );
}

export function segmentSelectionDistanceMeters(
  routePoints: ActivityRoutePoint[] | null | undefined,
  selection: SegmentBuilderSelection,
) {
  return (
    sliceSegmentRoutePoints(routePoints, selection).at(-1)?.distance_meters ??
    null
  );
}
