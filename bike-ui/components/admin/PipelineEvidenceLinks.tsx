import type { PipelineGraph } from "../../lib/pipelineGraph";

export default function EvidenceLinks({
  run,
  traceId,
  taskId,
}: {
  run: PipelineGraph;
  traceId?: string | null;
  taskId?: number;
}) {
  if (!run.grafana_url) return null;
  const base = run.grafana_url.replace(/\/$/, "");
  const key = traceId || run.request_id || run.run_id;
  const logs = {
    datasource: "loki",
    queries: [
      { refId: "A", expr: `{namespace="bike"} |= ${JSON.stringify(key)}` },
    ],
    range: { from: run.received_at, to: run.ended_at ?? "now" },
  };
  const trace = {
    datasource: "tempo",
    queries: [{ refId: "A", query: traceId, queryType: "traceId" }],
    range: { from: run.received_at, to: "now" },
  };
  return (
    <p className="text-xs space-x-3">
      <a
        href={`${base}/explore?left=${encodeURIComponent(JSON.stringify(logs))}`}
        target="_blank"
        rel="noreferrer"
      >
        Related logs{taskId ? ` · task #${taskId}` : ""}
      </a>
      {traceId ? (
        <a
          href={`${base}/explore?left=${encodeURIComponent(JSON.stringify(trace))}`}
          target="_blank"
          rel="noreferrer"
        >
          Open trace
        </a>
      ) : null}
    </p>
  );
}
