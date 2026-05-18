import { type ActivityRoutePoint } from "./queries";

export type RoutePreviewVariant = "thumbnail" | "full";

export type RoutePreviewCoordinate = Pick<
  ActivityRoutePoint,
  "latitude" | "longitude"
>;

type ThumbnailPoint = {
  x: number;
  y: number;
};

type RoutePreviewDimensions = {
  width: number;
  height: number;
  padding: number;
};

export const ROUTE_PREVIEW_STYLE_VERSION = "4";

const ROUTE_PREVIEW_DIMENSIONS: Record<
  RoutePreviewVariant,
  RoutePreviewDimensions
> = {
  thumbnail: {
    width: 288,
    height: 192,
    padding: 28,
  },
  full: {
    width: 1000,
    height: 300,
    padding: 40,
  },
};

export type RoutePreviewGeometry = {
  width: number;
  height: number;
  pathData: string;
  startPoint: ThumbnailPoint;
  endPoint: ThumbnailPoint;
};

export function resolveRoutePreviewVariant(
  value: string | null | undefined,
): RoutePreviewVariant {
  return value === "thumbnail" ? "thumbnail" : "full";
}

export function serializeRoutePreviewCoordinates(
  routePoints: readonly RoutePreviewCoordinate[] | null | undefined,
): string {
  return (routePoints ?? [])
    .map(
      (point) => `${point.latitude.toFixed(5)},${point.longitude.toFixed(5)}`,
    )
    .join(";");
}

export function parseRoutePreviewCoordinates(
  raw: string | null | undefined,
): RoutePreviewCoordinate[] {
  if (!raw) {
    return [];
  }

  return raw
    .split(";")
    .map((entry) => {
      const [latitude, longitude] = entry.split(",");
      const parsedLatitude = Number(latitude);
      const parsedLongitude = Number(longitude);

      if (
        !Number.isFinite(parsedLatitude) ||
        !Number.isFinite(parsedLongitude)
      ) {
        return null;
      }

      return {
        latitude: parsedLatitude,
        longitude: parsedLongitude,
      } satisfies RoutePreviewCoordinate;
    })
    .filter((point): point is RoutePreviewCoordinate => point !== null);
}

export function buildActivityRoutePreviewUrl(options: {
  activityId?: number | null;
  routePoints?: readonly RoutePreviewCoordinate[] | null | undefined;
  variant: RoutePreviewVariant;
}): string | null {
  if (Number.isFinite(options.activityId) && (options.activityId ?? 0) > 0) {
    const query = new URLSearchParams({
      activityId: String(options.activityId),
    });

    return `/activity-previews/${options.variant}?${query.toString()}`;
  }

  const query = new URLSearchParams({
    v: ROUTE_PREVIEW_STYLE_VERSION,
    variant: options.variant,
  });

  const serializedPoints = serializeRoutePreviewCoordinates(
    options.routePoints,
  );
  if (serializedPoints.length === 0) {
    return null;
  }

  query.set("points", serializedPoints);

  return `/activity-previews?${query.toString()}`;
}

export function buildRoutePreviewGeometry(
  routePoints: readonly RoutePreviewCoordinate[],
  variant: RoutePreviewVariant,
): RoutePreviewGeometry | null {
  if (routePoints.length < 2) {
    return null;
  }

  const dimensions = ROUTE_PREVIEW_DIMENSIONS[variant];
  const thumbnailPoints = projectRouteThumbnailPoints(
    routePoints,
    dimensions.width,
    dimensions.height,
    dimensions.padding,
  );

  if (thumbnailPoints.length < 2) {
    return null;
  }

  return {
    width: dimensions.width,
    height: dimensions.height,
    pathData: buildBoundedRouteThumbnailPath(thumbnailPoints),
    startPoint: thumbnailPoints[0],
    endPoint: thumbnailPoints.at(-1) ?? thumbnailPoints[0],
  };
}

function projectRouteThumbnailPoints(
  routePoints: readonly RoutePreviewCoordinate[],
  width: number,
  height: number,
  padding: number,
): ThumbnailPoint[] {
  const latitudes = routePoints.map((point) => point.latitude);
  const longitudes = routePoints.map((point) => point.longitude);
  const minLatitude = Math.min(...latitudes);
  const maxLatitude = Math.max(...latitudes);
  const minLongitude = Math.min(...longitudes);
  const maxLongitude = Math.max(...longitudes);
  const latitudeRange = Math.max(maxLatitude - minLatitude, 0.000001);
  const longitudeRange = Math.max(maxLongitude - minLongitude, 0.000001);
  const usableWidth = width - padding * 2;
  const usableHeight = height - padding * 2;
  const scale = Math.min(
    usableWidth / longitudeRange,
    usableHeight / latitudeRange,
  );
  const projectedWidth = longitudeRange * scale;
  const projectedHeight = latitudeRange * scale;
  const xOffset = (width - projectedWidth) / 2;
  const yOffset = (height - projectedHeight) / 2;

  return routePoints.map((point) => ({
    x:
      xOffset +
      ((point.longitude - minLongitude) / longitudeRange) * projectedWidth,
    y:
      height -
      yOffset -
      ((point.latitude - minLatitude) / latitudeRange) * projectedHeight,
  }));
}

function buildBoundedRouteThumbnailPath(points: ThumbnailPoint[]) {
  if (points.length === 0) {
    return "";
  }

  if (points.length === 1) {
    return `M ${points[0].x.toFixed(1)} ${points[0].y.toFixed(1)}`;
  }

  if (points.length === 2) {
    return `M ${points[0].x.toFixed(1)} ${points[0].y.toFixed(1)} L ${points[1].x.toFixed(1)} ${points[1].y.toFixed(1)}`;
  }

  const commands = [`M ${points[0].x.toFixed(1)} ${points[0].y.toFixed(1)}`];

  for (let index = 1; index < points.length; index += 1) {
    const point = points[index];

    commands.push(`L ${point.x.toFixed(1)} ${point.y.toFixed(1)}`);
  }

  return commands.join(" ");
}
