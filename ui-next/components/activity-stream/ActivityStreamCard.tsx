import Link from "next/link";
import {
  formatActivityTimestamp,
  formatDistance,
  formatDuration,
  formatElevation,
  formatHeartRate,
  type UnitSystem,
} from "../../lib/activityFormatting";
import { type Activity } from "../../lib/queries";
import MetricCard from "../MetricCard";
import ActivityAchievementChip from "./ActivityAchievementChip";
import { activityAchievements } from "./activityAchievements";
import ActivityRoutePreview from "./ActivityRoutePreview";

export default function ActivityStreamCard({
  activity,
  unitSystem,
  showFullRouteMaps,
}: {
  activity: Activity;
  unitSystem: UnitSystem;
  showFullRouteMaps: boolean;
}) {
  const achievements = activityAchievements(activity);
  const activityCardLayoutClassName = showFullRouteMaps
    ? "grid gap-3"
    : "grid gap-3 sm:grid-cols-[8.5rem_minmax(0,1fr)] sm:items-start";

  return (
    <article className="rounded-box border border-base-300 bg-base-100 p-4 shadow-sm sm:p-5">
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
            <MetricCard
              label="Distance"
              value={formatDistance(activity.distance_meters, unitSystem)}
              size="sm"
              tone="base-100"
              shadow={false}
              className="sm:min-w-[9rem]"
            />
            <MetricCard
              label="Moving time"
              value={formatDuration(
                activity.moving_time_seconds ?? activity.total_time_seconds,
              )}
              size="sm"
              tone="base-100"
              shadow={false}
              className="sm:min-w-[9rem]"
            />
            <MetricCard
              label="Elevation gain"
              value={formatElevation(
                activity.elevation_gain_meters,
                unitSystem,
              )}
              size="sm"
              tone="base-100"
              shadow={false}
              className="sm:min-w-[9rem]"
            />
            <MetricCard
              label="Max heart rate"
              value={formatHeartRate(activity.max_heart_rate_bpm)}
              size="sm"
              tone="base-100"
              shadow={false}
              className="sm:min-w-[9rem]"
            />
          </div>
        </div>
      </div>
    </article>
  );
}
