"use client";

import { useEffect, useId, useRef, useState } from "react";
import type {
  ActivityImportTrace,
  ActivityImportTraceNode,
} from "../../lib/queries";
import { LoadingSpinner } from "../ui/QueryState";

type ActivityImportTracePanelProps = {
  trace: ActivityImportTrace | null;
  isLoading: boolean;
  error: Error | null | undefined;
};

export default function ActivityImportTracePanel({
  trace,
  isLoading,
  error,
}: ActivityImportTracePanelProps) {
  const chart = trace
    ? buildStatusMermaidChart(trace.graph.mermaid, trace.nodes)
    : "";

  return (
    <div className="grid gap-6">
      {isLoading ? (
        <div className="flex items-center gap-3 text-sm text-base-content/70">
          <LoadingSpinner size="sm" />
          Loading import trace
        </div>
      ) : null}

      {error ? (
        <div className="alert alert-error">
          <span>{error.message}</span>
        </div>
      ) : null}

      {trace ? (
        <>
          <section>
            <h3 className="text-sm font-semibold uppercase text-base-content/60">
              DAG
            </h3>
            <MermaidFlowchart chart={chart} />
          </section>

          <section>
            <h3 className="text-sm font-semibold uppercase text-base-content/60">
              Integration events
            </h3>
            <div className="mt-3 overflow-x-auto">
              <table className="table table-sm">
                <thead>
                  <tr>
                    <th>Time</th>
                    <th>Type</th>
                    <th>Level</th>
                    <th>Message</th>
                  </tr>
                </thead>
                <tbody>
                  {trace.events.map((event) => (
                    <tr key={event.id}>
                      <td className="whitespace-nowrap">
                        {formatDateTime(event.created_at)}
                      </td>
                      <td className="font-mono text-xs">{event.event_type}</td>
                      <td>
                        <span className={eventLevelClass(event.level)}>
                          {event.level}
                        </span>
                      </td>
                      <td>{event.message}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </section>
        </>
      ) : null}
    </div>
  );
}

function MermaidFlowchart({ chart }: { chart: string }) {
  const id = useId().replace(/[^a-zA-Z0-9_-]/g, "");
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let isCancelled = false;

    async function renderChart() {
      try {
        setError(null);
        if (containerRef.current) {
          containerRef.current.innerHTML = "";
        }

        const mermaid = (await import("mermaid")).default;
        mermaid.initialize({
          startOnLoad: false,
          securityLevel: "strict",
          theme: "base",
          themeVariables: {
            fontFamily: "inherit",
            primaryColor: "#ecfdf5",
            primaryBorderColor: "#10b981",
            primaryTextColor: "#111827",
            lineColor: "#64748b",
            tertiaryColor: "#f8fafc",
          },
        });

        const renderId = `activity-import-${id}-${Date.now()}`;
        const { svg } = await mermaid.render(renderId, chart);
        if (!isCancelled && containerRef.current) {
          containerRef.current.innerHTML = svg;
        }
      } catch (err) {
        if (!isCancelled) {
          setError(
            err instanceof Error
              ? err.message
              : "Failed to render import graph",
          );
        }
      }
    }

    if (chart) {
      void renderChart();
    }

    return () => {
      isCancelled = true;
    };
  }, [chart, id]);

  return (
    <div className="mt-4 rounded-lg border border-base-300 bg-base-100 p-4">
      <div
        ref={containerRef}
        className="max-w-full overflow-auto [&_svg]:max-w-none"
      />
      {error ? (
        <div className="alert alert-error mt-3">
          <span>{error}</span>
        </div>
      ) : null}
    </div>
  );
}

function buildStatusMermaidChart(
  chart: string,
  nodes: ActivityImportTraceNode[],
) {
  const classes = new Map<string, string[]>();
  for (const node of nodes) {
    classes.set(node.status, [...(classes.get(node.status) ?? []), node.id]);
  }

  const lines = [
    chart,
    "classDef completed fill:#ecfdf5,stroke:#10b981,color:#111827;",
    "classDef failed fill:#fef2f2,stroke:#ef4444,color:#111827;",
    "classDef pending fill:#f8fafc,stroke:#cbd5e1,color:#475569;",
  ];

  for (const [status, nodeIds] of classes) {
    if (nodeIds.length > 0) {
      lines.push(`class ${nodeIds.join(",")} ${status};`);
    }
  }

  return lines.join("\n");
}

function formatDateTime(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}

function eventLevelClass(level: string) {
  if (level === "success") {
    return "badge badge-success badge-outline";
  }
  if (level === "error") {
    return "badge badge-error badge-outline";
  }
  if (level === "warning") {
    return "badge badge-warning badge-outline";
  }
  return "badge badge-info badge-outline";
}
