import { type ActivityRoutePoint } from "../../lib/queries";
import {
  buildActivityRoutePreviewUrl,
  type RoutePreviewVariant,
} from "../../lib/routePreview";

function ActivityRouteImage({
  activityId,
  title,
  routePoints,
  variant,
}: {
  activityId: number;
  title: string;
  routePoints: ActivityRoutePoint[] | null | undefined;
  variant: RoutePreviewVariant;
}) {
  const src = buildActivityRoutePreviewUrl({
    activityId,
    routePoints,
    variant,
  });
  const alt =
    variant === "full"
      ? `Route map for ${title}`
      : `Route thumbnail for ${title}`;
  const wrapperClassName =
    variant === "full"
      ? "grid h-[300px] w-full place-items-center overflow-hidden rounded-box border border-base-300 bg-base-200"
      : "grid place-items-center overflow-hidden rounded-box border border-base-300 bg-base-200 p-1.5";
  const imageClassName =
    variant === "full"
      ? "h-full w-full object-contain"
      : "h-24 w-full object-contain";

  if (!src) {
    return null;
  }

  return (
    <div className={wrapperClassName}>
      <img
        src={src}
        alt={alt}
        className={imageClassName}
        loading="lazy"
        decoding="async"
      />
    </div>
  );
}

export default function ActivityRoutePreview({
  activityId,
  title,
  routePoints,
  showFullMap,
}: {
  activityId: number;
  title: string;
  routePoints: ActivityRoutePoint[] | null | undefined;
  showFullMap: boolean;
}) {
  const points = routePoints ?? [];
  const emptyStateClassName = showFullMap
    ? "flex h-[300px] items-center justify-center rounded-box border border-base-300 bg-base-200 text-sm text-base-content/60"
    : "flex h-24 items-center justify-center rounded-box border border-base-300 bg-base-200 text-sm text-base-content/60";

  if (points.length < 2) {
    return <div className={emptyStateClassName}>No route</div>;
  }

  if (!showFullMap) {
    return (
      <ActivityRouteImage
        activityId={activityId}
        title={title}
        routePoints={routePoints}
        variant="thumbnail"
      />
    );
  }

  return (
    <ActivityRouteImage
      activityId={activityId}
      title={title}
      routePoints={points}
      variant="full"
    />
  );
}
