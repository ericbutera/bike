"use client";

import { Pagination, auth, featureFlags } from "@ericbutera/kaleido";
import { faUpload } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import Link from "next/link";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useEffect, useState } from "react";
import {
  extractApiMessage,
  formatActivityTimestamp,
  formatDistance,
  formatDuration,
  formatElevation,
  formatHeartRate,
} from "../lib/activityFormatting";
import { FLAG_ACTIVITY_LIST_FULL_MAPS } from "../lib/featureFlags";
import { type ActivityRoutePoint, useActivities } from "../lib/queries";
import { useUnitPreferences } from "../lib/unitPreferences";
import AuthRequiredCard from "./AuthRequiredCard";
import LeafletRouteMap from "./LeafletRouteMap";

function StreamMetric({ label, value }: { label: string; value: string }) {
  return (
    <div className="stats border border-base-300 bg-base-100 sm:min-w-[9rem]">
      <div className="stat px-3 py-2">
        <div className="stat-title">{label}</div>
        <div className="stat-value text-base">{value}</div>
      </div>
    </div>
  );
}

type ThumbnailPoint = {
  x: number;
  y: number;
};

const ROUTE_THUMBNAIL_MAX_POINTS = 72;
const ROUTE_THUMBNAIL_SIMPLIFY_TOLERANCE_PX = 1.5;

function projectRouteThumbnailPoints(
  routePoints: ActivityRoutePoint[],
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

function distanceBetweenPoints(a: ThumbnailPoint, b: ThumbnailPoint) {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

function perpendicularDistanceToSegment(
  point: ThumbnailPoint,
  start: ThumbnailPoint,
  end: ThumbnailPoint,
) {
  const segmentLength = distanceBetweenPoints(start, end);

  if (segmentLength === 0) {
    return distanceBetweenPoints(point, start);
  }

  return (
    Math.abs(
      (end.y - start.y) * point.x -
        (end.x - start.x) * point.y +
        end.x * start.y -
        end.y * start.x,
    ) / segmentLength
  );
}

function limitRouteThumbnailPoints(
  points: ThumbnailPoint[],
  maxPoints: number,
) {
  if (points.length <= maxPoints) {
    return points;
  }

  const selectedIndexes = new Set<number>();

  for (let index = 0; index < maxPoints; index += 1) {
    selectedIndexes.add(
      Math.round((index * (points.length - 1)) / Math.max(maxPoints - 1, 1)),
    );
  }

  return Array.from(selectedIndexes)
    .sort((left, right) => left - right)
    .map((index) => points[index]);
}

function simplifyRouteThumbnailPoints(
  points: ThumbnailPoint[],
  tolerance: number,
  maxPoints: number,
) {
  if (points.length <= 2) {
    return points;
  }

  const keep = Array.from({ length: points.length }, () => false);
  keep[0] = true;
  keep[points.length - 1] = true;

  const stack: Array<[number, number]> = [[0, points.length - 1]];

  while (stack.length > 0) {
    const [startIndex, endIndex] = stack.pop()!;

    if (endIndex - startIndex <= 1) {
      continue;
    }

    let maxDistance = 0;
    let indexToKeep = -1;

    for (let index = startIndex + 1; index < endIndex; index += 1) {
      const distance = perpendicularDistanceToSegment(
        points[index],
        points[startIndex],
        points[endIndex],
      );

      if (distance > maxDistance) {
        maxDistance = distance;
        indexToKeep = index;
      }
    }

    if (indexToKeep !== -1 && maxDistance >= tolerance) {
      keep[indexToKeep] = true;
      stack.push([startIndex, indexToKeep], [indexToKeep, endIndex]);
    }
  }

  return limitRouteThumbnailPoints(
    points.filter((_, index) => keep[index]),
    maxPoints,
  );
}

function buildSmoothRouteThumbnailPath(points: ThumbnailPoint[]) {
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

  for (let index = 0; index < points.length - 1; index += 1) {
    const previous = points[index - 1] ?? points[index];
    const current = points[index];
    const next = points[index + 1];
    const following = points[index + 2] ?? next;
    const control1 = {
      x: current.x + (next.x - previous.x) / 6,
      y: current.y + (next.y - previous.y) / 6,
    };
    const control2 = {
      x: next.x - (following.x - current.x) / 6,
      y: next.y - (following.y - current.y) / 6,
    };

    commands.push(
      `C ${control1.x.toFixed(1)} ${control1.y.toFixed(1)}, ${control2.x.toFixed(1)} ${control2.y.toFixed(1)}, ${next.x.toFixed(1)} ${next.y.toFixed(1)}`,
    );
  }

  return commands.join(" ");
}

function ActivityRouteThumbnail({
  title,
  routePoints,
}: {
  title: string;
  routePoints: ActivityRoutePoint[] | null | undefined;
}) {
  const points = routePoints ?? [];

  if (points.length < 2) {
    return (
      <div className="flex h-24 items-center justify-center rounded-box border border-base-300 bg-base-200 text-sm text-base-content/60">
        No route
      </div>
    );
  }

  const width = 144;
  const height = 96;
  const padding = 10;
  const thumbnailPoints = simplifyRouteThumbnailPoints(
    projectRouteThumbnailPoints(points, width, height, padding),
    ROUTE_THUMBNAIL_SIMPLIFY_TOLERANCE_PX,
    ROUTE_THUMBNAIL_MAX_POINTS,
  );
  const pathData = buildSmoothRouteThumbnailPath(thumbnailPoints);
  const startPoint = thumbnailPoints[0];
  const endPoint = thumbnailPoints.at(-1) ?? thumbnailPoints[0];

  return (
    <div className="overflow-hidden rounded-box border border-base-300 bg-base-200">
      <svg
        viewBox={`0 0 ${width} ${height}`}
        className="h-24 w-full"
        role="img"
        aria-label={`Route thumbnail for ${title}`}
      >
        <rect width={width} height={height} fill="transparent" />
        <path
          d={pathData}
          fill="none"
          stroke="#0f766e"
          strokeWidth="4"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
        <circle cx={startPoint.x} cy={startPoint.y} r="4" fill="#1d4ed8" />
        <circle cx={endPoint.x} cy={endPoint.y} r="4.5" fill="#dc2626" />
      </svg>
    </div>
  );
}

function ActivityRoutePreview({
  title,
  routePoints,
  showFullMap,
}: {
  title: string;
  routePoints: ActivityRoutePoint[] | null | undefined;
  showFullMap: boolean;
}) {
  const points = routePoints ?? [];

  if (!showFullMap) {
    return <ActivityRouteThumbnail title={title} routePoints={routePoints} />;
  }

  if (points.length < 2) {
    return (
      <div className="flex h-24 items-center justify-center rounded-box border border-base-300 bg-base-200 text-sm text-base-content/60">
        No route
      </div>
    );
  }

  return (
    <LeafletRouteMap
      routePoints={points}
      ariaLabel={`Route map for ${title}`}
      emptyMessage="No route"
      className="h-24 w-full overflow-hidden rounded-box border border-base-300 bg-base-200"
      showBaseTiles={false}
      interactive={false}
    />
  );
}

export default function ActivityStream() {
  const authApi = auth.useAuthApi();
  const { user, isLoading: isLoadingUser } = authApi.useCurrentUser();
  const { unitSystem } = useUnitPreferences();
  const showFullRouteMaps = featureFlags.useFeatureFlag(
    FLAG_ACTIVITY_LIST_FULL_MAPS,
  );
  const activityCardLayoutClassName = showFullRouteMaps
    ? "grid gap-3"
    : "grid gap-3 sm:grid-cols-[8.5rem_minmax(0,1fr)] sm:items-start";
  const pathname = usePathname();
  const router = useRouter();
  const searchParams = useSearchParams();
  const currentUrlPage = parsePageParam(searchParams.get("page"));
  const [page, setPage] = useState(currentUrlPage);
  const perPage = 10;
  const activitiesQuery = useActivities({ enabled: !!user, page, perPage });

  useEffect(() => {
    setPage(currentUrlPage);
  }, [currentUrlPage]);

  const handlePageChange = (nextPage: number) => {
    const normalizedPage = Math.max(1, nextPage);
    const nextSearchParams = new URLSearchParams(searchParams.toString());

    setPage(normalizedPage);
    if (normalizedPage === 1) {
      nextSearchParams.delete("page");
    } else {
      nextSearchParams.set("page", String(normalizedPage));
    }

    const query = nextSearchParams.toString();
    router.replace(query ? `${pathname}?${query}` : pathname, {
      scroll: false,
    });
  };

  if (isLoadingUser) {
    return (
      <section className="rounded-box border border-base-300 bg-base-100 shadow-sm">
        <div className="flex items-center justify-center py-10">
          <span className="loading loading-spinner loading-md" />
        </div>
      </section>
    );
  }

  if (!user) {
    return (
      <AuthRequiredCard
        eyebrow="Activity stream"
        title="Recent activity feed"
        description="Sign in to see your latest uploads normalized into a Garmin or Strava style stream."
        loginLabel="Sign in to view activities"
      />
    );
  }

  return (
    <section className="space-y-4">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <h2 className="text-3xl font-semibold text-base-content">
          Recent activities
        </h2>
        {activitiesQuery.isFetching ? (
          <span className="loading loading-spinner loading-sm" />
        ) : null}

        <Link href="/upload" className="btn btn-ghost btn-sm">
          <FontAwesomeIcon icon={faUpload} className="h-8 w-8" />
          Upload Activity
        </Link>
      </div>

      {activitiesQuery.isError ? (
        <div className="alert alert-error">
          {extractApiMessage(activitiesQuery.error)}
        </div>
      ) : null}

      {!activitiesQuery.isError && activitiesQuery.data.length === 0 ? (
        <div className="alert bg-base-100 shadow-sm">
          <span>
            No activities yet. Upload a GPX, TCX, or FIT file below to seed your
            stream.
          </span>
        </div>
      ) : null}

      <div className="space-y-3">
        {activitiesQuery.data.map((activity) => (
          <article
            key={activity.id}
            className="rounded-box border border-base-300 bg-base-100 p-4 shadow-sm sm:p-5"
          >
            <div className={activityCardLayoutClassName}>
              <ActivityRoutePreview
                title={activity.title}
                routePoints={activity.route_points}
                showFullMap={showFullRouteMaps}
              />

              <div>
                <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
                  <h3 className="min-w-0 flex-1 text-base font-semibold text-base-content">
                    <Link
                      href={`/activities/${activity.id}`}
                      className="link link-hover link-primary no-underline"
                    >
                      {activity.title}
                    </Link>
                  </h3>
                  {activity.location ? (
                    <span className="text-sm text-base-content/60">
                      {activity.location}
                    </span>
                  ) : null}
                  <span className="text-sm text-base-content/70">
                    {formatActivityTimestamp(activity.started_at)}
                  </span>
                </div>

                <div className="mt-3 grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
                  <StreamMetric
                    label="Distance"
                    value={formatDistance(activity.distance_meters, unitSystem)}
                  />
                  <StreamMetric
                    label="Moving time"
                    value={formatDuration(
                      activity.moving_time_seconds ??
                        activity.total_time_seconds,
                    )}
                  />
                  <StreamMetric
                    label="Elevation gain"
                    value={formatElevation(
                      activity.elevation_gain_meters,
                      unitSystem,
                    )}
                  />
                  <StreamMetric
                    label="Max heart rate"
                    value={formatHeartRate(activity.max_heart_rate_bpm)}
                  />
                </div>
              </div>
            </div>
          </article>
        ))}
      </div>

      {(activitiesQuery.metadata?.total ?? 0) > perPage ? (
        <Pagination
          page={activitiesQuery.metadata.page}
          perPage={activitiesQuery.metadata.per_page}
          total={activitiesQuery.metadata.total}
          onPageChange={handlePageChange}
        />
      ) : null}
    </section>
  );
}

function parsePageParam(rawPage: string | null): number {
  const parsedPage = Number(rawPage);

  if (Number.isFinite(parsedPage) && parsedPage >= 1) {
    return Math.floor(parsedPage);
  }

  return 1;
}
