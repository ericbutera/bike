import {
  formatDistance,
  formatDuration,
  formatElevation,
  formatRelativeEffort,
  type UnitSystem,
} from "../../lib/activityFormatting";
import type { Activity } from "./types";
import { AppCard } from "../ui/Card";

function PrimaryActivityStat({
  label,
  value,
  valueClassName,
}: {
  label: string;
  value: string;
  valueClassName?: string;
}) {
  return (
    <div className="min-w-0">
      <div
        className={`truncate text-3xl font-semibold text-base-content sm:text-4xl ${valueClassName ?? ""}`.trim()}
      >
        {value}
      </div>
      <div className="mt-1 text-sm text-base-content/60">{label}</div>
    </div>
  );
}

export default function ActivityMetricsSummary({
  activity,
  unitSystem,
}: {
  activity: Activity;
  unitSystem: UnitSystem;
}) {
  return (
    <AppCard as="section" aria-label="Activity summary">
      <div className="grid gap-x-6 gap-y-4 sm:grid-cols-2 xl:grid-cols-4">
        <PrimaryActivityStat
          label="Distance"
          value={formatDistance(activity.distance_meters, unitSystem)}
        />
        <PrimaryActivityStat
          label="Moving time"
          value={formatDuration(
            activity.moving_time_seconds ?? activity.total_time_seconds,
          )}
        />
        <PrimaryActivityStat
          label="Elevation"
          value={formatElevation(activity.elevation_gain_meters, unitSystem)}
        />
        <PrimaryActivityStat
          label="Relative effort"
          value={formatRelativeEffort(activity.relative_effort)}
          valueClassName="text-error"
        />
      </div>
    </AppCard>
  );
}
