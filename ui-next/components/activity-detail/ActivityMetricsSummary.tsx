"use client";

import {
  formatActivityTimestamp,
  formatCadence,
  formatCalories,
  formatDistance,
  formatDuration,
  formatElevation,
  formatHeartRate,
  formatRelativeEffort,
  formatSpeed,
  formatSport,
  type UnitSystem,
} from "../../lib/activityFormatting";
import type { Activity } from "../../lib/queries";
import InfoTooltip from "../ui/InfoTooltip";

const ACTIVITY_DATA_HELP_TEXT =
  "Secondary fields are grouped into a tighter stats list so the summary stays readable.";

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

function SecondaryMetricRow({
  label,
  average,
  maximum,
}: {
  label: string;
  average: string;
  maximum: string;
}) {
  return (
    <tr>
      <th className="font-medium text-base-content">{label}</th>
      <td>{average}</td>
      <td>{maximum}</td>
    </tr>
  );
}

function DenseDetailRow({ label, value }: { label: string; value: string }) {
  return (
    <>
      <dt className="text-base-content/55">{label}</dt>
      <dd className="font-medium text-base-content">{value}</dd>
    </>
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
    <>
      <div className="grid gap-x-6 gap-y-4 border-b border-base-300 pb-5 sm:grid-cols-2 xl:grid-cols-4">
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

      <div className="space-y-3">
        <div>
          <div className="flex items-center gap-2">
            <h2 className="text-xs font-medium uppercase tracking-[0.24em] text-base-content/50">
              Activity data
            </h2>
            <InfoTooltip
              label="Activity data details"
              tip={ACTIVITY_DATA_HELP_TEXT}
            />
          </div>
        </div>

        <div className="grid gap-3 lg:grid-cols-[minmax(0,1.15fr)_minmax(0,0.85fr)] lg:items-start">
          <div className="overflow-x-auto rounded-box border border-base-300 bg-base-100">
            <table className="table table-sm">
              <thead>
                <tr>
                  <th>Metric</th>
                  <th>Avg</th>
                  <th>Max</th>
                </tr>
              </thead>
              <tbody>
                <SecondaryMetricRow
                  label="Speed"
                  average={formatSpeed(activity.average_speed_mps, unitSystem)}
                  maximum={formatSpeed(activity.max_speed_mps, unitSystem)}
                />
                <SecondaryMetricRow
                  label="Heart rate"
                  average={formatHeartRate(activity.average_heart_rate_bpm)}
                  maximum={formatHeartRate(activity.max_heart_rate_bpm)}
                />
                <SecondaryMetricRow
                  label="Cadence"
                  average={formatCadence(activity.average_cadence_rpm)}
                  maximum={formatCadence(activity.max_cadence_rpm)}
                />
              </tbody>
            </table>
          </div>

          <dl className="grid gap-x-4 gap-y-2 rounded-box border border-base-300 bg-base-100 px-4 py-3 text-sm sm:grid-cols-[auto_1fr]">
            <DenseDetailRow label="Sport" value={formatSport(activity.sport)} />
            <DenseDetailRow
              label="Format"
              value={activity.format?.toUpperCase() ?? "--"}
            />
            <DenseDetailRow label="Source" value={activity.source} />
            <DenseDetailRow
              label="Uploaded file"
              value={activity.original_filename ?? "--"}
            />
            <DenseDetailRow
              label="Started"
              value={formatActivityTimestamp(activity.started_at)}
            />
            <DenseDetailRow
              label="Ended"
              value={
                activity.ended_at
                  ? formatActivityTimestamp(activity.ended_at)
                  : "--"
              }
            />
            <DenseDetailRow
              label="Calories"
              value={formatCalories(activity.calories)}
            />
          </dl>
        </div>
      </div>
    </>
  );
}
