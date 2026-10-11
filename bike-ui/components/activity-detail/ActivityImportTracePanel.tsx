"use client";

import { useState } from "react";
import Link from "next/link";
import ProcessingGraph from "../ui/ProcessingGraph";
import { ProcessingFacts, ProcessingStatus } from "../ui/ProcessingDetails";
import {
  formatTaskDate,
  formatTaskDuration,
  millisecondsBetween,
} from "../../lib/taskTiming";
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
  const active = Boolean(
    trace?.attempts?.some((item) =>
      ["queued", "running"].includes(item.status),
    ) || trace?.import.status === "processing",
  );
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
              <h3 className="text-lg font-semibold">Import stages</h3>
              <p className="text-sm text-base-content/65">
                Follow the connections from top to bottom. Select a stage for
                its result and recovery action.
              </p>
            </div>
            {!trace.attempts?.length ? (
              <p className="text-sm">
                Historical import: stage summaries are unavailable until replay.
              </p>
            ) : null}
          </div>
          {trace.attempts?.length ? (
            <section aria-label="Import attempts" className="space-y-3">
              <h4 className="font-semibold">Attempt history</h4>
              <div className="flex flex-wrap gap-2">
                {trace.attempts.map((item, index) => (
                  <button
                    key={item.id}
                    type="button"
                    aria-label={`Attempt #${item.id} ${item.status}`}
                    aria-pressed={attempt?.id === item.id}
                    className={`rounded-box border p-3 text-left ${attempt?.id === item.id ? "border-primary bg-primary/5" : "border-base-300"}`}
                    onClick={() => {
                      setAttemptId(item.id);
                      setSelectedStage(null);
                    }}
                  >
                    <div className="flex items-center gap-2">
                      <strong className="text-sm">Attempt #{item.id}</strong>
                      <ProcessingStatus status={item.status} />
                      {index === 0 ? (
                        <span className="text-xs text-base-content/60">
                          Latest
                        </span>
                      ) : null}
                    </div>
                    <p className="mt-1 text-xs text-base-content/60">
                      {formatTaskDate(item.created_at)}
                    </p>
                  </button>
                ))}
              </div>
            </section>
          ) : null}
          {attempt ? (
            <details className="rounded-box border border-base-300 p-4">
              <summary className="cursor-pointer text-sm font-medium">
                Source and timing for attempt #{attempt.id}
              </summary>
              <div className="mt-4">
                <ProcessingFacts
                  facts={[
                    {
                      label: "Retained source",
                      value: attempt.source.filename,
                    },
                    {
                      label: "Source format and quality",
                      value:
                        [
                          attempt.source.format?.toUpperCase(),
                          attempt.source.quality?.replaceAll("_", " "),
                        ]
                          .filter(Boolean)
                          .join(" · ") || null,
                    },
                    {
                      label: "Requested start",
                      value: stageLabel(nodes, attempt.requested_stage),
                    },
                    {
                      label: "Actual start",
                      value: stageLabel(nodes, attempt.start_stage),
                    },
                    {
                      label: "Started",
                      value: formatTaskDate(attempt.started_at) || null,
                    },
                    {
                      label: "Finished",
                      value: formatTaskDate(attempt.finished_at) || null,
                    },
                    ...(attempt.reused_attempt_id
                      ? [
                          {
                            label: "Earlier results",
                            value: `Reused from attempt #${attempt.reused_attempt_id}`,
                          },
                        ]
                      : []),
                  ]}
                />
              </div>
            </details>
          ) : null}
          {attemptError ? (
            <div className="alert alert-error">{attemptError}</div>
          ) : null}
          <div className="grid items-start gap-4 xl:grid-cols-[minmax(0,1fr)_20rem]">
            <ProcessingGraph
              nodes={nodes}
              edges={trace.graph.edges}
              selected={selected?.id}
              onSelect={setSelectedStage}
            />
            {selected ? (
              <section
                aria-label="Selected stage"
                className="space-y-4 rounded-box border border-base-300 p-4 xl:sticky xl:top-4"
              >
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <h4 className="font-semibold">{selected.label}</h4>
                  <ProcessingStatus status={selected.status} />
                </div>
                <ul className="space-y-2 text-sm">
                  {selected.summary?.map((fact) => (
                    <li key={fact}>{fact}</li>
                  ))}
                </ul>
                {!selected.summary?.length ? (
                  <p className="text-sm text-base-content/60">
                    No summary was recorded for this stage.
                  </p>
                ) : null}
                {selected.error ? (
                  <p className="text-sm text-error" role="alert">
                    {selected.error}
                  </p>
                ) : null}
                {selected.reused_attempt_id ? (
                  <p className="text-sm">
                    Reused from attempt #{selected.reused_attempt_id}
                  </p>
                ) : null}
                <ProcessingFacts
                  facts={[
                    {
                      label: "Started",
                      value: formatTaskDate(selected.started_at) || null,
                    },
                    {
                      label: "Finished",
                      value: formatTaskDate(selected.completed_at) || null,
                    },
                    {
                      label: "Duration",
                      value:
                        formatTaskDuration(
                          millisecondsBetween(
                            selected.started_at,
                            selected.completed_at,
                          ),
                        ) || null,
                    },
                    {
                      label: "Depends on",
                      value:
                        trace.graph.edges
                          .filter((edge) => edge.to === selected.id)
                          .map((edge) => stageLabel(nodes, edge.from))
                          .join(", ") || "No earlier stage",
                    },
                  ]}
                />
                {canReplay ? (
                  <ReplayControls
                    importId={trace.import.id}
                    stage={selected.id}
                    label={selected.label}
                    nodes={nodes}
                    active={active}
                    onQueued={() => {
                      setAttemptId(null);
                    }}
                  />
                ) : (
                  <Link
                    className="link link-primary text-sm"
                    href={`/admin/imports/${trace.import.id}`}
                  >
                    Open activity processing and replay
                  </Link>
                )}
              </section>
            ) : null}
          </div>
        </>
      ) : null}
    </div>
  );
}

function ReplayControls({
  importId,
  stage,
  label,
  nodes,
  active,
  onQueued,
}: {
  importId: number;
  stage: string;
  label: string;
  nodes: ActivityImportTraceNode[];
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
    <section className="border-t border-base-300 pt-4">
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
            ? `Replay from ${stageLabel(nodes, plan.data.start_stage)}`
            : `Replay from ${label}`}
      </button>
    </section>
  );
}

function stageLabel(nodes: ActivityImportTraceNode[], stage: string) {
  return nodes.find((node) => node.id === stage)?.label ?? stage;
}
