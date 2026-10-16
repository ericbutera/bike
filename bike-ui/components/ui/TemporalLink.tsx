"use client";

import { config } from "../../lib/config";

export default function TemporalLink({
  workflowId,
  namespace = "bike",
}: {
  workflowId?: string;
  namespace?: string;
}) {
  const baseUrl = config.TEMPORAL_UI_URL;
  if (!baseUrl) return null;
  const workflowsUrl = `${baseUrl.replace(/\/$/, "")}/namespaces/${encodeURIComponent(namespace)}/workflows`;
  return (
    <a
      className="link link-primary"
      target="_blank"
      rel="noreferrer"
      href={
        workflowId
          ? `${workflowsUrl}/${encodeURIComponent(workflowId)}`
          : workflowsUrl
      }
    >
      {workflowId ? "View job in Temporal" : "Open Temporal workflows"}
    </a>
  );
}
