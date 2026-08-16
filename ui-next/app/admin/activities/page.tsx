"use client";

import { LoadingSpinner } from "@/components/ui/QueryState";
import {
  type AdminActivity,
  type ActivityProcessingGraph,
  type IntegrationEvent,
  useAdminActivities,
  useAdminIntegrationEvents,
  useActivityProcessingGraph,
} from "@/lib/queries";
import { type Column, GenericList, admin } from "@ericbutera/kaleido";
import { faGear } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { Suspense, useEffect, useId, useMemo, useRef, useState } from "react";
import AuthRouter from "../../../components/AuthRouter";

const ADMIN_ACTIVITIES_PAGE_SIZE = 25;
const AdminActivitiesGridSchema = {} as never;

type AdminActivitiesGridParams = {
  page?: number | string;
  per_page?: number | string;
};

export default function AdminActivitiesPage() {
  return (
    <Suspense>
      <AuthRouter>
        <admin.Layout title="Activities">
          <AdminActivitiesContent />
        </admin.Layout>
      </AuthRouter>
    </Suspense>
  );
}

function AdminActivitiesContent() {
  const [selectedActivity, setSelectedActivity] =
    useState<AdminActivity | null>(null);
  const graphQuery = useActivityProcessingGraph({
    enabled: selectedActivity !== null,
  });
  const eventsQuery = useAdminIntegrationEvents({
    provider: "activity_processing",
    importId: selectedActivity?.activity_import_id ?? null,
    limit: 100,
    enabled: !!selectedActivity?.activity_import_id,
  });
  const columns = useMemo<Column<AdminActivity, AdminActivitiesGridParams>[]>(
    () => [
      {
        key: "title",
        header: "Activity",
        className: "min-w-72 whitespace-nowrap",
        render: (activity) => (
          <div className="min-w-0">
            <div className="font-medium text-base-content">
              {activity.title}
            </div>
            <div className="mt-1 text-xs text-base-content/60">
              #{activity.id} · user {activity.user_id}
            </div>
          </div>
        ),
      },
      {
        key: "started_at",
        header: "Started",
        className: "min-w-48 whitespace-nowrap",
        render: (activity) => formatDateTime(activity.started_at),
      },
      {
        key: "source",
        header: "Source",
        className: "min-w-44 whitespace-nowrap",
        render: (activity) => (
          <div className="flex flex-col gap-1">
            <span className="badge badge-outline">{activity.source}</span>
            {activity.format ? (
              <span className="text-xs uppercase text-base-content/55">
                {activity.format}
              </span>
            ) : null}
          </div>
        ),
      },
      {
        key: "activity_import_id",
        header: "Import",
        className: "min-w-32 whitespace-nowrap",
        render: (activity) =>
          activity.activity_import_id ? (
            <div className="flex flex-col gap-1">
              <span className="font-mono text-xs">
                #{activity.activity_import_id}
              </span>
              <span className={importStatusClass(activity.import_status)}>
                {activity.import_status ?? "unknown"}
              </span>
            </div>
          ) : (
            <span className="text-base-content/45">None</span>
          ),
      },
      {
        key: "actions",
        header: "",
        className: "w-12 whitespace-nowrap",
        render: (activity) => (
          <button
            type="button"
            className="btn btn-ghost btn-square btn-sm"
            aria-label={
              activity.activity_import_id
                ? `View import trace for ${activity.title}`
                : `${activity.title} has no linked import trace`
            }
            disabled={!activity.activity_import_id}
            title={
              activity.activity_import_id
                ? "View import DAG and events"
                : "No linked import"
            }
            onClick={() => {
              setSelectedActivity(activity);
            }}
          >
            <FontAwesomeIcon icon={faGear} className="h-4 w-4" />
          </button>
        ),
      },
    ],
    [],
  );

  const useAdminActivitiesGridQuery = (params: AdminActivitiesGridParams) => {
    const page = positiveNumber(params.page, 1);
    const perPage = positiveNumber(params.per_page, ADMIN_ACTIVITIES_PAGE_SIZE);
    const query = useAdminActivities({ page, perPage });

    return {
      data: query.data ?? [],
      isLoading: query.isLoading,
      raw: { metadata: { total: query.metadata?.total ?? 0 } },
    };
  };

  return (
    <div className="grid gap-6 p-6">
      <GenericList
        title={
          <span className="text-xs font-medium uppercase tracking-[0.24em] text-base-content/50">
            Activities
          </span>
        }
        paramsSchema={AdminActivitiesGridSchema}
        useQuery={useAdminActivitiesGridQuery}
        columns={columns}
        emptyMessage="No activities found."
      />

      <ActivityImportTraceModal
        activity={selectedActivity}
        graph={graphQuery.data}
        events={eventsQuery.data ?? []}
        isLoading={graphQuery.isLoading || eventsQuery.isLoading}
        error={graphQuery.error ?? eventsQuery.error}
        onClose={() => {
          setSelectedActivity(null);
        }}
      />
    </div>
  );
}

function ActivityImportTraceModal({
  activity,
  graph,
  events,
  isLoading,
  error,
  onClose,
}: {
  activity: AdminActivity | null;
  graph: ActivityProcessingGraph | null;
  events: IntegrationEvent[];
  isLoading: boolean;
  error: Error | null;
  onClose: () => void;
}) {
  const isOpen = !!activity;
  const nodes =
    activity && graph ? buildTraceNodes(activity, graph, events) : [];
  const chart = graph ? buildStatusMermaidChart(graph.mermaid, nodes) : "";

  if (!isOpen) {
    return null;
  }

  return (
    <dialog className="modal modal-open">
      <div className="modal-box max-w-5xl">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h2 className="text-lg font-semibold">Import trace</h2>
            {activity?.activity_import_id ? (
              <p className="mt-1 text-sm text-base-content/65">
                Activity #{activity.id} · import #{activity.activity_import_id}
                {activity.import_version
                  ? ` · version ${activity.import_version}`
                  : ""}
              </p>
            ) : null}
          </div>
          <button
            type="button"
            className="btn btn-ghost btn-sm"
            onClick={onClose}
          >
            Close
          </button>
        </div>

        {isLoading ? (
          <div className="mt-8 flex items-center gap-3 text-sm text-base-content/70">
            <LoadingSpinner size="sm" />
            Loading import trace
          </div>
        ) : null}

        {error ? (
          <div className="alert alert-error mt-6">
            <span>{error.message}</span>
          </div>
        ) : null}

        {activity && graph ? (
          <div className="mt-6 grid gap-6">
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
                    {events.map((event) => (
                      <tr key={event.id}>
                        <td className="whitespace-nowrap">
                          {formatDateTime(event.created_at)}
                        </td>
                        <td className="font-mono text-xs">
                          {event.event_type}
                        </td>
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
          </div>
        ) : null}
      </div>
      <form method="dialog" className="modal-backdrop">
        <button type="button" onClick={onClose}>
          close
        </button>
      </form>
    </dialog>
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

    void renderChart();

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

function positiveNumber(value: number | string | undefined, fallback: number) {
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

function buildTraceNodes(
  activity: AdminActivity,
  graph: ActivityProcessingGraph,
  events: IntegrationEvent[],
) {
  const ranks = new Map(graph.nodes.map((node, index) => [node.stage, index]));
  const currentStage = activity.import_processing_stage ?? "";
  const currentRank =
    currentStage === "complete"
      ? Number.POSITIVE_INFINITY
      : (ranks.get(currentStage) ?? -1);

  return graph.nodes.map((node, index) => {
    const completedAt = events.find((event) => {
      const payload = event.payload ?? {};
      return (
        event.event_type === "stage_completed" &&
        typeof payload === "object" &&
        "stage" in payload &&
        payload.stage === node.stage
      );
    })?.created_at;

    return {
      ...node,
      status:
        activity.import_status === "failed" && currentStage === node.stage
          ? "failed"
          : index <= currentRank
            ? "completed"
            : "pending",
      completed_at: completedAt ?? null,
    };
  });
}

function buildStatusMermaidChart(
  chart: string,
  nodes: ReturnType<typeof buildTraceNodes>,
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
    lines.push(`class ${nodeIds.join(",")} ${status};`);
  }

  return lines.join("\n");
}

function formatDateTime(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}

function importStatusClass(status: string | null | undefined) {
  if (status === "processed") {
    return "badge badge-success badge-outline";
  }
  if (status === "failed") {
    return "badge badge-error badge-outline";
  }
  if (status === "duplicate") {
    return "badge badge-warning badge-outline";
  }
  return "badge badge-info badge-outline";
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
