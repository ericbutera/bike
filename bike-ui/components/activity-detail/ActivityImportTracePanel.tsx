"use client";

import { useEffect, useId, useRef, useState } from "react";
import toast from "react-hot-toast";
import {
  type ActivityImportTrace,
  type ActivityImportTraceNode,
  useActivityImportReplayPlan,
  useReplayActivityImport,
} from "../../lib/queries";
import { LoadingSpinner } from "../ui/QueryState";

export default function ActivityImportTracePanel({
  trace,
  isLoading,
  error,
  canReplay = false,
}: {
  trace: ActivityImportTrace | null;
  isLoading: boolean;
  error: Error | null | undefined;
  canReplay?: boolean;
}) {
  const [attemptId, setAttemptId] = useState<number | null>(null);
  const [selectedStage, setSelectedStage] = useState<string | null>(null);
  const attempt =
    trace?.attempts?.find((item) => item.id === attemptId) ??
    trace?.attempts?.[0];
  const nodes = attempt?.nodes ?? trace?.nodes ?? [];
  const selected =
    nodes.find((node) => node.id === selectedStage) ??
    nodes.find((node) => node.status === "failed") ??
    nodes[0];
  const active =
    trace?.attempts?.some((item) =>
      ["queued", "running"].includes(item.status),
    ) ?? trace?.import.status === "processing";
  const attemptError = attempt ? attempt.error : trace?.import.processing_error;

  return (
    <div className="grid gap-6">
      {isLoading ? (
        <div className="flex items-center gap-3">
          <LoadingSpinner size="sm" />
          Loading import trace
        </div>
      ) : null}
      {error ? <div className="alert alert-error">{error.message}</div> : null}
      {trace ? (
        <>
          <div className="flex flex-wrap items-center justify-between gap-4">
            <div>
              <h3 className="font-semibold">Processing DAG</h3>
              <p className="text-sm text-base-content/65">
                Select a stage to inspect its result or replay from there.
              </p>
            </div>
            {trace.attempts?.length ? (
              <label className="form-control">
                Attempt
                <select
                  aria-label="Attempt"
                  className="select select-bordered"
                  value={attempt?.id}
                  onChange={(event) => setAttemptId(Number(event.target.value))}
                >
                  {trace.attempts.map((item) => (
                    <option key={item.id} value={item.id}>
                      #{item.id} · {item.status} ·{" "}
                      {new Date(item.created_at).toLocaleString()}
                    </option>
                  ))}
                </select>
              </label>
            ) : (
              <p className="text-sm">
                Historical import: stage summaries are unavailable until replay.
              </p>
            )}
          </div>
          {attempt ? (
            <p className="text-sm text-base-content/70">
              {attempt.source.filename} · {attempt.source.format?.toUpperCase()}{" "}
              · {attempt.source.quality}
              <br />
              Requested {stageLabel(nodes, attempt.requested_stage)} · Started
              at {stageLabel(nodes, attempt.start_stage)}
              {attempt.reused_attempt_id
                ? ` · Reused earlier results from #${attempt.reused_attempt_id}`
                : ""}
            </p>
          ) : null}
          {attemptError ? (
            <div className="alert alert-error">{attemptError}</div>
          ) : null}
          <MermaidFlowchart
            chart={buildStatusMermaidChart(trace.graph.mermaid, nodes)}
            onSelect={setSelectedStage}
          />
          <div
            className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3"
            aria-label="Stage results"
          >
            {nodes.map((node) => (
              <button
                key={node.id}
                type="button"
                aria-pressed={selected?.id === node.id}
                className={`rounded-box border p-4 text-left ${selected?.id === node.id ? "border-primary bg-primary/5" : "border-base-300"}`}
                onClick={() => setSelectedStage(node.id)}
              >
                <div className="flex items-center justify-between gap-2">
                  <strong>{node.label}</strong>
                  <span className="badge badge-outline">{node.status}</span>
                </div>
                <ul className="mt-2 space-y-1 text-sm text-base-content/70">
                  {node.summary?.map((fact) => (
                    <li key={fact}>{fact}</li>
                  ))}
                </ul>
                {node.reused_attempt_id ? (
                  <p className="mt-2 text-xs">
                    Reused from attempt #{node.reused_attempt_id}
                  </p>
                ) : null}
                {node.error ? (
                  <p className="mt-2 text-sm text-error">{node.error}</p>
                ) : null}
                {node.completed_at ? (
                  <p className="mt-2 text-xs text-base-content/60">
                    {new Date(node.completed_at).toLocaleString()}
                  </p>
                ) : null}
              </button>
            ))}
          </div>
          {canReplay && selected ? (
            <ReplayControls
              importId={trace.import.id}
              stage={selected.id}
              label={selected.label}
              active={active}
              onQueued={() => setAttemptId(null)}
            />
          ) : null}
        </>
      ) : null}
    </div>
  );
}

function ReplayControls({
  importId,
  stage,
  label,
  active,
  onQueued,
}: {
  importId: number;
  stage: string;
  label: string;
  active: boolean;
  onQueued: () => void;
}) {
  const plan = useActivityImportReplayPlan(importId, stage, !active);
  const mutation = useReplayActivityImport();
  const [queuedAttempt, setQueuedAttempt] = useState<number | null>(null);
  async function replay() {
    try {
      const result = await mutation.replay(importId, stage, plan.data);
      setQueuedAttempt(result.attempt_id ?? null);
      onQueued();
      toast.success("Replay queued");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Replay failed");
    }
  }
  return (
    <section className="rounded-box border border-base-300 p-4">
      <h4 className="font-semibold">Replay from {label}</h4>
      <p className="mt-2 text-sm">
        This creates a new attempt and runs this stage and every downstream
        stage to completion.
      </p>
      {active ? (
        <p className="mt-2 text-sm" role="status">
          An attempt is already queued or running.
        </p>
      ) : null}
      {plan.data?.reason ? (
        <p className="mt-2 text-sm text-warning">{plan.data.reason}</p>
      ) : null}
      {plan.data?.reused_attempt_id ? (
        <p className="mt-2 text-sm">
          Earlier results will be reused from attempt #
          {plan.data.reused_attempt_id}.
        </p>
      ) : null}
      {plan.error ? (
        <p className="mt-2 text-error">{plan.error.message}</p>
      ) : null}
      {queuedAttempt ? (
        <p className="mt-2 text-sm" role="status">
          Queued attempt #{queuedAttempt}. Results update automatically.
        </p>
      ) : null}
      <button
        type="button"
        className="btn btn-primary mt-4"
        disabled={active || plan.isLoading || !plan.data || mutation.isPending}
        onClick={() => void replay()}
      >
        {mutation.isPending
          ? "Queueing replay…"
          : plan.data?.start_stage !== stage && plan.data
            ? "Replay from Raw stored"
            : `Replay from ${label}`}
      </button>
    </section>
  );
}

function MermaidFlowchart({
  chart,
  onSelect,
}: {
  chart: string;
  onSelect: (stage: string) => void;
}) {
  const id = useId().replace(/[^a-zA-Z0-9_-]/g, "");
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    async function render() {
      try {
        setError(null);
        const mermaid = (await import("mermaid")).default;
        mermaid.initialize({
          startOnLoad: false,
          securityLevel: "strict",
          theme: "base",
        });
        const { svg } = await mermaid.render(
          `activity-import-${id}-${Date.now()}`,
          chart,
        );
        if (!cancelled && containerRef.current)
          containerRef.current.innerHTML = svg;
      } catch (error) {
        if (!cancelled)
          setError(
            error instanceof Error
              ? error.message
              : "Failed to render import graph",
          );
      }
    }
    if (chart) void render();
    return () => {
      cancelled = true;
    };
  }, [chart, id]);
  return (
    <div className="overflow-x-auto rounded-box border border-base-300 p-4">
      <div
        ref={containerRef}
        onClick={(event) => {
          const node = (event.target as Element).closest("g.node");
          const stage = node?.id.match(/flowchart-(.+)-\d+$/)?.[1];
          if (stage) onSelect(stage);
        }}
      />
      {error ? <p className="text-error">{error}</p> : null}
    </div>
  );
}

function buildStatusMermaidChart(
  chart: string,
  nodes: ActivityImportTraceNode[],
) {
  for (const node of nodes) {
    const fact = node.summary?.[0]?.replace(/[<>&"\[\]]/g, "");
    if (fact)
      chart = chart.replace(
        `${node.id}["${node.label}"]`,
        `${node.id}["${node.label}<br/>${fact}"]`,
      );
  }
  const colors: Record<string, string> = {
    completed: "#ecfdf5",
    reused: "#eff6ff",
    failed: "#fef2f2",
    running: "#fef3c7",
  };
  return [
    chart,
    ...nodes.map(
      (node) =>
        `style ${node.id} fill:${colors[node.status] ?? "#f8fafc"},stroke:#64748b,color:#111827;`,
    ),
  ].join("\n");
}

function stageLabel(nodes: ActivityImportTraceNode[], stage: string) {
  return nodes.find((node) => node.id === stage)?.label ?? stage;
}
