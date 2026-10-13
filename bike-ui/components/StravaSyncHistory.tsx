"use client";

import Link from "next/link";
import { formatActivityTimestamp } from "@/lib/activityFormatting";
import { useActivity, type IntegrationEvent } from "@/lib/queries";

type ActivityChange = {
  kind: "activity";
  eventId: number;
  date: string;
  activityId: number | null;
  stravaActivityId: number | null;
  activityName: string | null;
  change: "Updated" | "Removed";
};

type SyncHistoryEntry =
  | ActivityChange
  | {
      kind: "sync";
      eventId: number;
      date: string;
      summary: string;
    };

const SYNC_MESSAGES: Record<string, string> = {
  queued: "Sync queued.",
  started: "Sync in progress.",
  running: "Sync in progress.",
  completed: "Sync completed.",
  succeeded: "Sync completed.",
  failed: "Sync needs attention.",
  cancelled: "Sync cancelled.",
  paused: "Sync paused.",
  resumed: "Sync in progress.",
};

function syncSummary(event: IntegrationEvent) {
  const type = event.event_type.replaceAll("_", ".");
  if (type.startsWith("sync.")) {
    const message = SYNC_MESSAGES[type.slice(5)] ?? "Sync update.";
    const count = event.payload?.imported_count;
    return (type === "sync.completed" || type === "sync.succeeded") &&
      typeof count === "number" &&
      Number.isSafeInteger(count) &&
      count >= 0
      ? `${message} ${count} ${count === 1 ? "activity" : "activities"} imported.`
      : message;
  }
  const payload = event.payload;
  const collectionStage = ["discover", "children", "complete"].includes(
    String(payload?.stage),
  );
  if (type === "workflow.stage.failed" && collectionStage) {
    return "Sync needs attention.";
  }
  if (payload?.action !== "sync" && payload?.action !== "backfill") {
    return null;
  }
  if (type === "workflow.accepted") return "Sync queued.";
  if (type === "workflow.stage" && collectionStage) {
    return payload.stage === "complete"
      ? "Sync completed."
      : "Sync in progress.";
  }
  return null;
}

function activityChange(event: IntegrationEvent): ActivityChange | null {
  if (event.event_type !== "workflow.stage") return null;
  const payload = event.payload;
  let change: ActivityChange["change"];
  if (payload?.stage === "apply" && payload.outcome === "applied") {
    change = "Updated";
  } else if (payload?.stage === "delete" && payload.outcome === "deleted") {
    change = "Removed";
  } else {
    return null;
  }

  const activityId = positiveId(payload.activity_id);
  const stravaActivityId = positiveId(payload.strava_activity_id);
  if (activityId == null && stravaActivityId == null) {
    return null;
  }

  return {
    kind: "activity",
    eventId: event.id,
    date: event.created_at,
    activityId,
    stravaActivityId,
    activityName:
      typeof payload.activity_name === "string"
        ? payload.activity_name.trim() || null
        : null,
    change,
  };
}

export function getStravaSyncHistory(events: IntegrationEvent[]) {
  const seenSyncs = new Set<string>();
  return events.flatMap((event): SyncHistoryEntry[] => {
    if (event.provider !== "strava") return [];
    const summary = syncSummary(event);
    if (summary) {
      const workflowId = event.payload?.workflow_id;
      if (typeof workflowId === "string") {
        if (seenSyncs.has(workflowId)) return [];
        seenSyncs.add(workflowId);
      }
      return [
        { kind: "sync", eventId: event.id, date: event.created_at, summary },
      ];
    }
    if (
      event.event_type === "webhook.activity_delete" &&
      event.level === "success"
    ) {
      const id = positiveId(event.payload?.activity_id);
      return id == null
        ? []
        : [
            {
              kind: "activity",
              eventId: event.id,
              date: event.created_at,
              activityId: null,
              stravaActivityId: id,
              activityName: null,
              change: "Removed",
            },
          ];
    }
    const change = activityChange(event);
    return change ? [change] : [];
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
  entries,
}: {
  entries: SyncHistoryEntry[];
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
          {entries.map((entry) => (
            <tr key={entry.eventId}>
              <td className="whitespace-nowrap text-sm text-base-content/60">
                <time dateTime={entry.date}>
                  {formatActivityTimestamp(entry.date)}
                </time>
              </td>
              <td className="text-sm">
                {entry.kind === "sync" ? (
                  entry.summary
                ) : (
                  <ChangedActivity change={entry} />
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
