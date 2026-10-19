import {
  formatActivityTimestamp,
  formatCadence,
  formatCalories,
  formatDuration,
  formatHeartRate,
  formatSpeed,
  type UnitSystem,
} from "../../lib/activityFormatting";
import type { Activity } from "./types";
import { AppCard, CardHeader } from "../ui/Card";

function MetricRow({
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
      <th scope="row" className="font-medium text-base-content">
        {label}
      </th>
      <td className="tabular-nums">{average}</td>
      <td className="tabular-nums">{maximum}</td>
    </tr>
  );
}

function DetailRow({ label, value }: { label: string; value: string }) {
  return (
    <>
      <dt className="text-base-content/60">{label}</dt>
      <dd className="min-w-0 whitespace-pre-wrap break-words font-medium text-base-content">
        {value}
      </dd>
    </>
  );
}

export default function ActivityDataCard({
  activity,
  unitSystem,
}: {
  activity: Activity;
  unitSystem: UnitSystem;
}) {
  const hasHeartRate =
    activity.average_heart_rate_bpm != null ||
    activity.max_heart_rate_bpm != null;
  const hasCadence =
    activity.average_cadence_rpm != null || activity.max_cadence_rpm != null;

  return (
    <AppCard as="section" aria-label="Activity data" bodyClassName="gap-3">
      <CardHeader title="Activity data" />
      <div className="grid min-w-0 gap-4 md:grid-cols-2 md:gap-6">
        <div className="min-w-0 overflow-x-auto">
          <table className="table table-sm" aria-label="Activity measurements">
            <thead>
              <tr>
                <th scope="col">
                  <span className="sr-only">Metric</span>
                </th>
                <th scope="col">Avg</th>
                <th scope="col">Max</th>
              </tr>
            </thead>
            <tbody>
              <MetricRow
                label="Speed"
                average={formatSpeed(activity.average_speed_mps, unitSystem)}
                maximum={formatSpeed(activity.max_speed_mps, unitSystem)}
              />
              {hasHeartRate ? (
                <MetricRow
                  label="Heart rate"
                  average={formatHeartRate(activity.average_heart_rate_bpm)}
                  maximum={formatHeartRate(activity.max_heart_rate_bpm)}
                />
              ) : null}
              {hasCadence ? (
                <MetricRow
                  label="Cadence"
                  average={formatCadence(activity.average_cadence_rpm)}
                  maximum={formatCadence(activity.max_cadence_rpm)}
                />
              ) : null}
            </tbody>
          </table>
        </div>
        <dl className="grid min-w-0 content-start grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-1 border-t border-base-300 pt-3 text-sm md:border-l md:border-t-0 md:pl-6 md:pt-0">
          <DetailRow
            label="Elapsed time"
            value={formatDuration(activity.total_time_seconds)}
          />
          {activity.gear ? (
            <DetailRow
              label="Gear"
              value={activity.gear.name ?? activity.gear.id}
            />
          ) : null}
          {activity.calories != null ? (
            <DetailRow
              label="Calories"
              value={formatCalories(activity.calories)}
            />
          ) : null}
          {activity.private_notes ? (
            <DetailRow label="Private notes" value={activity.private_notes} />
          ) : null}
          <DetailRow
            label="Started"
            value={formatActivityTimestamp(activity.started_at)}
          />
          <DetailRow
            label="Ended"
            value={
              activity.ended_at
                ? formatActivityTimestamp(activity.ended_at)
                : "--"
            }
          />
        </dl>
      </div>
      {activity.description ? (
        <p className="whitespace-pre-wrap text-sm text-base-content/80">
          {activity.description}
        </p>
      ) : null}
    </AppCard>
  );
}
