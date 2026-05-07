"use client";

import { Pagination, auth } from "@ericbutera/kaleido";
import Link from "next/link";
import { useState } from "react";
import {
  extractApiMessage,
  formatActivityTimestamp,
  formatDistance,
  formatDuration,
  formatElevation,
  formatHeartRate,
} from "../lib/activityFormatting";
import { type ActivityRoutePoint, useActivities } from "../lib/queries";
import { useUnitPreferences } from "../lib/unitPreferences";
import AuthRequiredCard from "./AuthRequiredCard";

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
  const latitudes = points.map((point) => point.latitude);
  const longitudes = points.map((point) => point.longitude);
  const minLatitude = Math.min(...latitudes);
  const maxLatitude = Math.max(...latitudes);
  const minLongitude = Math.min(...longitudes);
  const maxLongitude = Math.max(...longitudes);
  const latitudeRange = Math.max(maxLatitude - minLatitude, 0.000001);
  const longitudeRange = Math.max(maxLongitude - minLongitude, 0.000001);
  const polylinePoints = points
    .map((point) => {
      const x =
        padding +
        ((point.longitude - minLongitude) / longitudeRange) *
          (width - padding * 2);
      const y =
        height -
        padding -
        ((point.latitude - minLatitude) / latitudeRange) *
          (height - padding * 2);

      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
  const startPoint = polylinePoints.split(" ")[0] ?? "";
  const endPoint = polylinePoints.split(" ").at(-1) ?? "";
  const [startX = 0, startY = 0] = startPoint.split(",").map(Number);
  const [endX = 0, endY = 0] = endPoint.split(",").map(Number);

  return (
    <div className="overflow-hidden rounded-box border border-base-300 bg-base-200">
      <svg
        viewBox={`0 0 ${width} ${height}`}
        className="h-24 w-full"
        role="img"
        aria-label={`Route thumbnail for ${title}`}
      >
        <rect width={width} height={height} fill="transparent" />
        <polyline
          fill="none"
          stroke="#0f766e"
          strokeWidth="4"
          strokeLinecap="round"
          strokeLinejoin="round"
          points={polylinePoints}
        />
        <circle cx={startX} cy={startY} r="4" fill="#1d4ed8" />
        <circle cx={endX} cy={endY} r="4.5" fill="#dc2626" />
      </svg>
    </div>
  );
}

export default function ActivityStream() {
  const authApi = auth.useAuthApi();
  const { user, isLoading: isLoadingUser } = authApi.useCurrentUser();
  const { unitSystem } = useUnitPreferences();
  const [page, setPage] = useState(1);
  const perPage = 10;
  const activitiesQuery = useActivities({ enabled: !!user, page, perPage });

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
            <div className="grid gap-3 sm:grid-cols-[8.5rem_minmax(0,1fr)] sm:items-start">
              <ActivityRouteThumbnail
                title={activity.title}
                routePoints={activity.route_points}
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
          onPageChange={setPage}
        />
      ) : null}
    </section>
  );
}
