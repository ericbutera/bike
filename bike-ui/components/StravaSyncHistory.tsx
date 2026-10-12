"use client";

import Link from "next/link";
import { formatActivityTimestamp } from "@/lib/activityFormatting";
import { useActivity, type IntegrationEvent } from "@/lib/queries";

type ActivityChange = {
  eventId: number;
  date: string;
  activityId: number | null;
  stravaActivityId: number | null;
  activityName: string | null;
  change: "Updated" | "Removed";
};

export function getStravaActivityChanges(events: IntegrationEvent[]) {
  return events.flatMap((event): ActivityChange[] => {
    if (event.provider !== "strava" || event.event_type !== "workflow.stage") {
      return [];
    }
    const payload = event.payload;
    let change: ActivityChange["change"];
    if (payload?.stage === "apply" && payload.outcome === "applied") {
      change = "Updated";
    } else if (payload?.stage === "delete" && payload.outcome === "deleted") {
      change = "Removed";
    } else {
      return [];
    }

    const activityId = positiveId(payload.activity_id);
    const stravaActivityId = positiveId(payload.strava_activity_id);
    if (activityId == null && stravaActivityId == null) {
      return [];
    }

    return [
      {
        eventId: event.id,
        date: event.created_at,
        activityId,
        stravaActivityId,
        activityName:
          typeof payload.activity_name === "string"
            ? payload.activity_name.trim() || null
            : null,
        change,
      },
    ];
  });
}

function positiveId(value: unknown) {
  return typeof value === "number" && Number.isSafeInteger(value) && value > 0
    ? value
    : null;
}

function ChangedActivity({ change }: { change: ActivityChange }) {
  const removed = change.change === "Removed";
  const activityQuery = useActivity(removed ? null : change.activityId);
  const name =
    change.activityName ??
    activityQuery.data?.title ??
    (change.stravaActivityId != null
      ? `Strava activity ${change.stravaActivityId}`
      : `Activity ${change.activityId}`);

  return (
    <>
      {change.change}:{" "}
      {!removed && change.activityId != null && !activityQuery.error ? (
        <Link
          href={`/activities/${change.activityId}`}
          className="link link-primary"
        >
          {name}
        </Link>
      ) : (
        <span>{name}</span>
      )}
    </>
  );
}

export default function StravaSyncHistory({
  changes,
}: {
  changes: ActivityChange[];
}) {
  return (
    <div className="overflow-x-auto">
      <table className="table table-zebra w-full">
        <thead>
          <tr>
            <th className="w-40">Date</th>
            <th>Activities modified</th>
          </tr>
        </thead>
        <tbody>
          {changes.map((change) => (
            <tr key={change.eventId}>
              <td className="whitespace-nowrap text-sm text-base-content/60">
                <time dateTime={change.date}>
                  {formatActivityTimestamp(change.date)}
                </time>
              </td>
              <td className="text-sm">
                <ChangedActivity change={change} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
