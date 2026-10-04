"use client";

import { type ActivityRoutePoint } from "../../lib/queries";
import { buildActivityRoutePreviewUrl } from "../../lib/routePreview";
import { useEffect, useState } from "react";
import { activityMapImageUrl } from "../../lib/activityMapImages";
import { type RoutePreviewVariant } from "../../lib/routePreview";
import { useBikeTheme } from "../../lib/useBikeTheme";

function ActivityRouteImage({
  activityId,
  title,
  variant,
}: {
  activityId: number;
  title: string;
  variant: RoutePreviewVariant;
}) {
  const theme = useBikeTheme();
  const [dpr, setDpr] = useState<1 | 2>(1);
  const [ready, setReady] = useState(false);
  useEffect(() => {
    setDpr(window.devicePixelRatio >= 1.5 ? 2 : 1);
    setReady(true);
  }, []);
  const src = activityMapImageUrl({ activityId, variant, theme, dpr });
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

  return (
    <div className={wrapperClassName}>
      {ready ? (
        <img
          src={src}
          alt={alt}
          className={imageClassName}
          loading="lazy"
          decoding="async"
        />
      ) : null}
    </div>
  );
}

function LegacyActivityRouteImage({
  activityId,
  title,
  variant,
}: {
  activityId: number;
  title: string;
  variant: RoutePreviewVariant;
}) {
  const src = buildActivityRoutePreviewUrl({
    activityId,
    variant,
  });
  const isFull = variant === "full";

  if (!src) {
    return null;
  }

  return (
    <div
      className={
        isFull
          ? "grid h-[300px] w-full place-items-center overflow-hidden rounded-box border border-base-300 bg-base-200"
          : "grid place-items-center overflow-hidden rounded-box border border-base-300 bg-base-200 p-1.5"
      }
    >
      <img
        src={src}
        alt={isFull ? `Route map for ${title}` : `Route thumbnail for ${title}`}
        className={
          isFull ? "h-full w-full object-contain" : "h-24 w-full object-contain"
        }
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
  showEnhancedMaps = true,
}: {
  activityId: number;
  title: string;
  routePoints: ActivityRoutePoint[] | null | undefined;
  showFullMap: boolean;
  showEnhancedMaps?: boolean;
}) {
  const points = routePoints ?? [];
  const emptyStateClassName = showFullMap
    ? "flex h-[300px] items-center justify-center rounded-box border border-base-300 bg-base-200 text-sm text-base-content/60"
    : "flex h-24 items-center justify-center rounded-box border border-base-300 bg-base-200 text-sm text-base-content/60";

  if (points.length < 2) {
    return <div className={emptyStateClassName}>No route</div>;
  }

  const variant = showFullMap ? "full" : "thumbnail";

  if (!showEnhancedMaps) {
    return (
      <LegacyActivityRouteImage
        activityId={activityId}
        title={title}
        variant={variant}
      />
    );
  }

  return (
    <ActivityRouteImage
      activityId={activityId}
      title={title}
      variant={variant}
    />
  );
}
