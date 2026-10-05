"use client";

import { type ActivityRoutePoint } from "../../lib/queries";
import Image from "next/image";
import { useSyncExternalStore } from "react";
import { buildActivityRoutePreviewUrl } from "../../lib/routePreview";
import { activityMapImageUrl } from "../../lib/activityMapImages";
import { type RoutePreviewVariant } from "../../lib/routePreview";
import { useBikeTheme } from "../../lib/useBikeTheme";

function subscribeToWindowResize(onChange: () => void) {
  window.addEventListener("resize", onChange);
  return () => window.removeEventListener("resize", onChange);
}

function getDevicePixelRatio(): 1 | 2 {
  return window.devicePixelRatio >= 1.5 ? 2 : 1;
}

function getServerDevicePixelRatio(): 1 {
  return 1;
}

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
  const dpr = useSyncExternalStore(
    subscribeToWindowResize,
    getDevicePixelRatio,
    getServerDevicePixelRatio,
  );
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
  const dimensions =
    variant === "full"
      ? { width: 1000, height: 300 }
      : { width: 288, height: 192 };

  return (
    <div className={wrapperClassName}>
      <Image
        src={src}
        alt={alt}
        width={dimensions.width}
        height={dimensions.height}
        className={imageClassName}
        loading="lazy"
        unoptimized
      />
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
  const dimensions = isFull
    ? { width: 1000, height: 300 }
    : { width: 288, height: 192 };

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
      <Image
        src={src}
        alt={isFull ? `Route map for ${title}` : `Route thumbnail for ${title}`}
        width={dimensions.width}
        height={dimensions.height}
        className={
          isFull ? "h-full w-full object-contain" : "h-24 w-full object-contain"
        }
        loading="lazy"
        unoptimized
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
