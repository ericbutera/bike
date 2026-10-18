"use client";

import { formatActivityTimestamp } from "@/lib/activityFormatting";
import type { IntegrationEvent } from "@/lib/queries";
import { useState } from "react";
import { faMagnifyingGlass } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import Modal from "./ui/Modal";
import StravaIcon from "./ui/StravaIcon";
import StravaEventHistory from "./StravaEventHistory";
import TemporalLink from "./ui/TemporalLink";
import StatusBadge from "./ui/StatusBadge";
import { CenteredLoading, ErrorCard } from "./ui/QueryState";

type IntegrationEventFeedProps = {
  events: IntegrationEvent[];
  isLoading: boolean;
  error: unknown;
  emptyMessage: string;
  audience?: "account" | "admin";
  showUserId?: boolean;
  showProvider?: boolean;
};

export default function IntegrationEventFeed({
  events,
  isLoading,
  error,
  emptyMessage,
  audience = "account",
  showUserId = false,
  showProvider = false,
}: IntegrationEventFeedProps) {
  if (isLoading) {
    return <CenteredLoading />;
  }

  if (error) {
    return (
      <ErrorCard
        error={error}
        fallback={
          audience === "account"
            ? "Unable to load Strava event history."
            : "Unable to load integration events."
        }
      />
    );
  }

  if (events.length === 0) {
    return (
      <div className="rounded-box border border-dashed border-base-300 bg-base-200/60 px-4 py-6 text-sm text-base-content/65">
        {emptyMessage}
      </div>
    );
  }

  if (audience === "account") {
    return <StravaEventHistory events={events} />;
  }

  return (
    <div className="overflow-x-auto">
      <table className="table table-zebra w-full">
        <thead>
          <tr>
            <th className="w-28">Level</th>
            <th className="w-40">Event</th>
            {showProvider ? <th className="w-28">Provider</th> : null}
            {showUserId ? <th className="w-28">User</th> : null}
            <th className="w-36">Connection</th>
            <th className="w-40">When</th>
            <th className="w-12">
              <span className="sr-only">Inspect</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {events.map((event) => {
            return (
              <tr key={event.id} className="align-top">
                <td className="align-top">
                  <StatusBadge status={event.level} />
                </td>
                <td className="align-top">
                  <span className="text-xs uppercase">
                    {formatEventType(event.event_type)}
                  </span>
                </td>
                {showProvider ? (
                  <td className="align-top whitespace-nowrap">
                    {event.provider === "strava" ? (
                      <StravaIcon />
                    ) : (
                      event.provider
                    )}
                  </td>
                ) : null}
                {showUserId ? (
                  <td className="align-top whitespace-nowrap text-sm">
                    {event.user_id != null ? (
                      <span>User {event.user_id}</span>
                    ) : (
                      <span className="text-base-content/40">-</span>
                    )}
                  </td>
                ) : null}
                <td className="align-top whitespace-nowrap text-sm">
                  {event.connection_id != null ? (
                    <span>Connection {event.connection_id}</span>
                  ) : (
                    <span className="text-base-content/40">-</span>
                  )}
                </td>
                <td className="whitespace-nowrap text-sm text-base-content/60">
                  {formatActivityTimestamp(event.created_at)}
                </td>
                <td className="align-top">
                  <IntegrationEventInspector event={event} />
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function IntegrationEventInspector({ event }: { event: IntegrationEvent }) {
  const [open, setOpen] = useState(false);
  const payload = event.payload ?? null;
  const payloadMetrics = buildPayloadMetrics(payload);
  const hasPayload = payload != null && Object.keys(payload).length > 0;
  return (
    <>
      <button
        type="button"
        className="btn btn-ghost btn-xs btn-square"
        aria-label={`Inspect event ${event.id}`}
        title="Inspect event"
        onClick={() => setOpen(true)}
      >
        <FontAwesomeIcon icon={faMagnifyingGlass} className="h-3 w-3" />
      </button>
      {open ? (
        <Modal
          title={`Integration event #${event.id}`}
          onClose={() => setOpen(false)}
        >
          <div className="space-y-3">
            <p className="text-sm leading-6 text-base-content/80">
              {event.message}
            </p>
            {typeof payload?.temporal_workflow_id === "string" ? (
              <TemporalLink
                workflowId={payload.temporal_workflow_id}
                namespace={String(payload.temporal_namespace ?? "bike")}
              />
            ) : null}
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
        </Modal>
      ) : null}
    </>
  );
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
    metricFromPayload(payload, "import_id", "Import"),
    metricFromPayload(payload, "strava_activity_id", "Strava activity"),
    metricFromPayload(payload, "athlete_id", "Athlete"),
    metricFromPayload(payload, "workflow_id", "Workflow"),
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
