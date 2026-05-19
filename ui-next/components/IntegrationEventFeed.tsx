"use client";

import {
  extractApiMessage,
  formatActivityTimestamp,
} from "@/lib/activityFormatting";
import type { IntegrationEvent } from "@/lib/queries";

type IntegrationEventFeedProps = {
  events: IntegrationEvent[];
  isLoading: boolean;
  error: unknown;
  emptyMessage: string;
  showUserId?: boolean;
  showProvider?: boolean;
};

export default function IntegrationEventFeed({
  events,
  isLoading,
  error,
  emptyMessage,
  showUserId = false,
  showProvider = false,
}: IntegrationEventFeedProps) {
  if (isLoading) {
    return (
      <div className="flex justify-center py-8">
        <span className="loading loading-spinner loading-md" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="alert alert-error/80 text-sm">
        <span>{extractApiMessage(error)}</span>
      </div>
    );
  }

  if (events.length === 0) {
    return (
      <div className="rounded-box border border-dashed border-base-300 bg-base-200/60 px-4 py-6 text-sm text-base-content/65">
        {emptyMessage}
      </div>
    );
  }

  return (
    <div className="overflow-x-auto">
      <table className="table table-zebra w-full">
        <thead>
          <tr>
            <th className="w-40">When</th>
            <th className="w-28">Level</th>
            <th className="w-40">Event</th>
            {showProvider ? <th className="w-28">Provider</th> : null}
            {showUserId ? <th className="w-28">User</th> : null}
            <th className="w-36">Connection</th>
            <th className="min-w-[24rem]">Details</th>
          </tr>
        </thead>
        <tbody>
          {events.map((event) => {
            const payload = event.payload ?? null;
            const payloadMetrics = buildPayloadMetrics(payload);
            const hasPayload = payload != null && Object.keys(payload).length > 0;

            return (
              <tr key={event.id} className="align-top">
                <td className="whitespace-nowrap text-sm text-base-content/60">
                  {formatActivityTimestamp(event.created_at)}
                </td>
                <td className="align-top">
                  <span className={levelBadgeClassName(event.level)}>
                    {event.level}
                  </span>
                </td>
                <td className="align-top">
                  <span className="badge badge-ghost badge-sm uppercase">
                    {formatEventType(event.event_type)}
                  </span>
                </td>
                {showProvider ? (
                  <td className="align-top whitespace-nowrap">
                    <span className="badge badge-outline badge-sm uppercase">
                      {event.provider}
                    </span>
                  </td>
                ) : null}
                {showUserId ? (
                  <td className="align-top whitespace-nowrap text-sm">
                    {event.user_id != null ? (
                      <span className="badge badge-outline badge-sm">
                        User {event.user_id}
                      </span>
                    ) : (
                      <span className="text-base-content/40">-</span>
                    )}
                  </td>
                ) : null}
                <td className="align-top whitespace-nowrap text-sm">
                  {event.connection_id != null ? (
                    <span className="badge badge-outline badge-sm">
                      Connection {event.connection_id}
                    </span>
                  ) : (
                    <span className="text-base-content/40">-</span>
                  )}
                </td>
                <td className="align-top">
                  <div className="min-w-[20rem] space-y-3">
                    <p className="text-sm leading-6 text-base-content/80">
                      {event.message}
                    </p>
                    {payloadMetrics.length > 0 ? (
                      <div className="flex flex-wrap gap-2">
                        {payloadMetrics.map((metric) => (
                          <span
                            key={`${event.id}-${metric.label}`}
                            className="badge badge-ghost badge-sm"
                          >
                            {metric.label}: {metric.value}
                          </span>
                        ))}
                      </div>
                    ) : null}
                    {hasPayload ? (
                      <details>
                        <summary className="cursor-pointer text-xs uppercase tracking-[0.18em] text-base-content/50">
                          Payload
                        </summary>
                        <pre className="mt-2 overflow-x-auto rounded-xl bg-base-200 p-3 text-xs text-base-content/70">
                          {JSON.stringify(payload, null, 2)}
                        </pre>
                      </details>
                    ) : null}
                  </div>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function levelBadgeClassName(level: string) {
  switch (level) {
    case "success":
      return "badge badge-success badge-outline badge-sm uppercase";
    case "warning":
      return "badge badge-warning badge-outline badge-sm uppercase";
    case "error":
      return "badge badge-error badge-outline badge-sm uppercase";
    default:
      return "badge badge-info badge-outline badge-sm uppercase";
  }
}

function formatEventType(value: string) {
  return value
    .split(/[._]/g)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function buildPayloadMetrics(payload: Record<string, unknown> | null) {
  if (!payload) {
    return [];
  }

  return [
    metricFromPayload(payload, "imported_count", "Imported"),
    metricFromPayload(payload, "duplicate_count", "Duplicates"),
    metricFromPayload(payload, "failed_count", "Failed"),
    metricFromPayload(payload, "cancelled_task_count", "Cancelled"),
    metricFromPayload(payload, "activity_id", "Activity"),
    metricFromPayload(payload, "athlete_id", "Athlete"),
  ].filter(
    (metric): metric is { label: string; value: string } => metric != null,
  );
}

function metricFromPayload(
  payload: Record<string, unknown>,
  key: string,
  label: string,
) {
  const value = payload[key];

  if (typeof value === "number") {
    return { label, value: String(value) };
  }

  if (typeof value === "string" && value.trim()) {
    return { label, value };
  }

  return null;
}
