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
    <div className="space-y-3">
      {events.map((event) => {
        const payload = event.payload ?? null;
        const payloadMetrics = buildPayloadMetrics(payload);

        return (
          <article
            key={event.id}
            className="rounded-box border border-base-300 bg-base-100 px-4 py-4 shadow-sm"
          >
            <div className="flex flex-wrap items-start justify-between gap-3">
              <div className="space-y-2">
                <div className="flex flex-wrap items-center gap-2 text-xs uppercase tracking-[0.18em] text-base-content/45">
                  <span className={levelBadgeClassName(event.level)}>
                    {event.level}
                  </span>
                  <span className="badge badge-ghost badge-sm uppercase">
                    {formatEventType(event.event_type)}
                  </span>
                  {showProvider ? (
                    <span className="badge badge-outline badge-sm uppercase">
                      {event.provider}
                    </span>
                  ) : null}
                  {showUserId && event.user_id ? (
                    <span className="badge badge-outline badge-sm">
                      User {event.user_id}
                    </span>
                  ) : null}
                  {event.connection_id ? (
                    <span className="badge badge-outline badge-sm">
                      Connection {event.connection_id}
                    </span>
                  ) : null}
                </div>
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
              </div>

              <div className="text-sm text-base-content/60">
                {formatActivityTimestamp(event.created_at)}
              </div>
            </div>

            {payload && Object.keys(payload).length > 0 ? (
              <details className="mt-3">
                <summary className="cursor-pointer text-xs uppercase tracking-[0.18em] text-base-content/50">
                  Payload
                </summary>
                <pre className="mt-2 overflow-x-auto rounded-xl bg-base-200 p-3 text-xs text-base-content/70">
                  {JSON.stringify(payload, null, 2)}
                </pre>
              </details>
            ) : null}
          </article>
        );
      })}
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
