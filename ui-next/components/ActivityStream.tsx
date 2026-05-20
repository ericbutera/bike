"use client";

import { Pagination, auth, featureFlags } from "@ericbutera/kaleido";
import {
  faCrown,
  faMedal,
  faTrophy,
  faUpload,
} from "@fortawesome/free-solid-svg-icons";
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
import {
  buildActivityRoutePreviewUrl,
  type RoutePreviewVariant,
} from "../lib/routePreview";
import {
  type Activity,
  type ActivityAchievementHighlight,
  type ActivityRoutePoint,
  useActivities,
} from "../lib/queries";
import { primarySegmentAchievement } from "../lib/segmentAchievements";
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

type StreamAchievement = {
  key: string;
  label: string;
  segmentTitle: string;
  tone: "kom" | "top-10" | "pr";
  priority: number;
};

function streamAchievementBadgeClassName(tone: StreamAchievement["tone"]) {
  switch (tone) {
    case "kom":
      return "border-warning/40 bg-warning/10 text-warning-content";
    case "top-10":
      return "border-info/40 bg-info/10 text-info-content";
    case "pr":
      return "border-primary/40 bg-primary/10 text-primary-content";
  }
}

function streamAchievementIcon(tone: StreamAchievement["tone"]) {
  switch (tone) {
    case "kom":
      return faCrown;
    case "top-10":
      return faTrophy;
    case "pr":
      return faMedal;
  }
}

function personalBestLabel(rank: number) {
  if (rank === 1) {
    return "PR";
  }

  if (rank === 2) {
    return "2nd best";
  }

  return "3rd best";
}

function streamAchievementPriority(achievement: StreamAchievement) {
  switch (achievement.tone) {
    case "kom":
      return 0;
    case "top-10":
      return 10 + achievement.priority;
    case "pr":
      return 20 + achievement.priority;
  }
}

function streamAchievement(
  effort: ActivityAchievementHighlight,
): StreamAchievement[] {
  const primary = primarySegmentAchievement({
    overallRank: effort.overall_rank,
    personalRank: effort.personal_rank,
  });

  if (primary) {
    if (primary.kind === "fastest") {
      return [];
    }

    return [
      {
        key: `${effort.segment_id}-${effort.effort_index}-${primary.shortLabel}`,
        label: primary.longLabel,
        segmentTitle: effort.segment_title,
        tone: primary.kind,
        priority: primary.overallRank ?? effort.personal_rank ?? 0,
      },
    ];
  }

  if (
    effort.personal_rank != null &&
    effort.personal_rank >= 2 &&
    effort.personal_rank <= 3
  ) {
    return [
      {
        key: `${effort.segment_id}-${effort.effort_index}-personal-${effort.personal_rank}`,
        label: personalBestLabel(effort.personal_rank),
        segmentTitle: effort.segment_title,
        tone: "pr",
        priority: effort.personal_rank,
      },
    ];
  }

  return [];
}

function activityAchievements(activity: Activity) {
  return (activity.achievement_highlights ?? activity.segment_efforts ?? [])
    .flatMap((effort) => streamAchievement(effort))
    .sort((left, right) => {
      const leftPriority = streamAchievementPriority(left);
      const rightPriority = streamAchievementPriority(right);

      if (leftPriority !== rightPriority) {
        return leftPriority - rightPriority;
      }

      return left.segmentTitle.localeCompare(right.segmentTitle);
    });
}

function ActivityAchievementChip({
  achievement,
}: {
  achievement: StreamAchievement;
}) {
  return (
    <span
      className={`inline-flex max-w-full items-center gap-1.5 rounded-full border px-3 py-1 text-xs font-medium ${streamAchievementBadgeClassName(achievement.tone)}`}
      title={`${achievement.segmentTitle} · ${achievement.label}`}
    >
      <FontAwesomeIcon
        icon={streamAchievementIcon(achievement.tone)}
        className="h-3 w-3 shrink-0"
      />
      <span className="shrink-0">{achievement.label}</span>
      <span className="truncate">{achievement.segmentTitle}</span>
    </span>
  );
}

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

function ActivityRoutePreview({
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
        {activitiesQuery.data.map((activity) => {
          const achievements = activityAchievements(activity);

          return (
            <article
              key={activity.id}
              className="rounded-box border border-base-300 bg-base-100 p-4 shadow-sm sm:p-5"
            >
              <div className={activityCardLayoutClassName}>
                <ActivityRoutePreview
                  activityId={activity.id}
                  title={activity.title}
                  routePoints={activity.route_points}
                  showFullMap={showFullRouteMaps}
                />

                <div>
                  <div className="grid gap-2 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-start sm:gap-x-4">
                    <div className="min-w-0">
                      <div className="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1">
                        <h3 className="min-w-0 flex-1 text-base font-semibold text-base-content">
                          <Link
                            href={`/activities/${activity.id}`}
                            className="link link-hover link-primary no-underline"
                          >
                            {activity.title}
                          </Link>
                        </h3>
                        {activity.location ? (
                          <span className="hidden text-sm text-base-content/60 sm:inline">
                            {activity.location}
                          </span>
                        ) : null}
                      </div>
                      {activity.location ? (
                        <p className="mt-1 text-sm text-base-content/60 sm:hidden">
                          {activity.location}
                        </p>
                      ) : null}
                    </div>

                    <span className="text-sm text-base-content/70 sm:text-right">
                      {formatActivityTimestamp(activity.started_at)}
                    </span>
                  </div>

                  {achievements.length > 0 ? (
                    <div className="mt-3 flex flex-wrap gap-2">
                      {achievements.map((achievement) => (
                        <ActivityAchievementChip
                          key={achievement.key}
                          achievement={achievement}
                        />
                      ))}
                    </div>
                  ) : null}

                  <div className="mt-3 grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
                    <StreamMetric
                      label="Distance"
                      value={formatDistance(
                        activity.distance_meters,
                        unitSystem,
                      )}
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
          );
        })}
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
