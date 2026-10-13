"use client";
import { useState } from "react";
import { usePipelineGraph } from "../../lib/queries";
import { pipelineMermaid } from "../../lib/pipelineGraph";
import {
  formatTaskDate,
  formatTaskInterval as duration,
} from "../../lib/taskTiming";
import MermaidFlowchart from "../ui/MermaidFlowchart";
import { ProcessingFacts, ProcessingStatus } from "../ui/ProcessingDetails";

import EvidenceLinks from "./PipelineEvidenceLinks";
import TaskEvidence from "./PipelineTaskEvidence";
export default function PipelinePanel({ runId }: { runId: string }) {
  const [cursor, setCursor] = useState(0);
  const [selected, setSelected] = useState<string | null>(null);
  const [workAfter, setWorkAfter] = useState(0);
  const [outputOffset, setOutputOffset] = useState(0);
  const taskId = Number(selected?.match(/^(?:task|try|work|stage)_(\d+)/)?.[1]);
  const query = usePipelineGraph(runId, cursor, {
    work_task: Number.isFinite(taskId) ? taskId : undefined,
    after_work: workAfter,
    output_offset: outputOffset,
  });
  const run = query.data;
  const task =
    run?.tasks.find((task) => task.id === taskId) ??
    (selected == null ? run?.tasks[0] : undefined);
  return (
    <section aria-label="Pipeline graph" className="space-y-3">
      {query.isLoading ? <p>Loading pipeline…</p> : null}
      {query.error ? <p role="alert">Unable to load pipeline.</p> : null}
      {run ? (
        <>
          <h4 className="font-semibold">Run overview</h4>
          <ol aria-label="Run progress" className="grid gap-3 md:grid-cols-3">
            {[
              {
                label: "1. Received",
                time: run.received_at,
                detail: run.entrypoint.replaceAll("_", " "),
              },
              {
                label: "2. Accepted for processing",
                time: run.accepted_at,
                detail: run.ended_at
                  ? `Work ended ${formatTaskDate(run.ended_at)}`
                  : "Work has not ended",
              },
              {
                label: "3. Outputs available",
                time: run.available_at,
                detail: run.available_at
                  ? "Publication confirmed"
                  : "Readiness pending or not recorded",
              },
            ].map((step) => (
              <li
                key={step.label}
                className="rounded-box border border-base-300 bg-base-200/40 p-4"
              >
                <h5 className="font-semibold">{step.label}</h5>
                <p className="mt-2 text-sm">
                  {formatTaskDate(step.time) || "Not recorded"}
                </p>
                <p className="mt-1 text-xs text-base-content/60">
                  {step.detail}
                </p>
              </li>
            ))}
          </ol>
          <ProcessingFacts
            facts={[
              {
                label: "Receipt to availability",
                value: duration(run.received_at, run.available_at),
              },
              {
                label: "Receipt to end of work",
                value: duration(run.received_at, run.ended_at),
              },
            ]}
          />
          <details className="rounded-box border border-base-300 p-3">
            <summary className="cursor-pointer text-sm font-medium">
              Run identifiers
            </summary>
            <div className="mt-3">
              <ProcessingFacts
                facts={[
                  { label: "Run ID", value: run.run_id },
                  { label: "Request ID", value: run.request_id },
                  { label: "Trace ID", value: run.trace_id },
                ]}
              />
            </div>
          </details>
          <EvidenceLinks run={run} traceId={run.trace_id} />
          <div className="flex flex-wrap gap-2" aria-label="Worker tasks">
            {run.tasks.map((item) => (
              <button
                type="button"
                key={item.id}
                aria-pressed={item.id === task?.id}
                className={`rounded-box border p-3 text-left ${item.id === task?.id ? "border-primary bg-primary/5" : "border-base-300"}`}
                onClick={() => {
                  setSelected(`task_${item.id}`);
                  setWorkAfter(0);
                }}
              >
                <span className="block text-sm font-semibold">
                  Task #{item.id}: {item.task_type.replaceAll("_", " ")}
                </span>
                <ProcessingStatus status={item.status} />
              </button>
            ))}
          </div>
          <details className="rounded-box border border-base-300 p-3">
            <summary className="cursor-pointer text-sm font-medium">
              Complete task dependency graph
            </summary>
            <div className="mt-3">
              <MermaidFlowchart
                chart={pipelineMermaid(run)}
                onSelect={(node) => {
                  setSelected(node);
                  setWorkAfter(0);
                }}
              />
            </div>
          </details>
          {task ? (
            <TaskEvidence run={run} task={task} selected={selected} />
          ) : (
            <p className="text-sm">
              Select a task, attempt, work unit or import stage to inspect its
              timings and evidence.
            </p>
          )}
          {task?.next_work_cursor != null ? (
            <button
              className="btn btn-sm"
              onClick={() => setWorkAfter(task.next_work_cursor!)}
            >
              More work evidence
            </button>
          ) : null}
          {workAfter > 0 ? (
            <button className="btn btn-sm" onClick={() => setWorkAfter(0)}>
              First work evidence
            </button>
          ) : null}
          <section aria-label="Published outputs" className="space-y-3">
            <h5 className="font-semibold">Outputs</h5>
            {!run.outputs.length ? (
              <p className="text-sm text-base-content/60">
                No output evidence was recorded on this page.
              </p>
            ) : null}
            <div className="grid gap-3 sm:grid-cols-2">
              {run.outputs.map((output, index) => (
                <article
                  key={index}
                  className="rounded-box border border-base-300 p-4"
                >
                  <div className="mb-3 flex items-center justify-between gap-2">
                    <h6 className="text-sm font-semibold">
                      {output.kind.replaceAll("_", " ")} #{output.target_id}
                    </h6>
                    <ProcessingStatus status={output.status} />
                  </div>
                  <ProcessingFacts
                    facts={[
                      { label: "Revision", value: output.revision },
                      {
                        label: "Published",
                        value:
                          formatTaskDate(output.available_at) ||
                          "Pending or not recorded",
                      },
                    ]}
                  />
                </article>
              ))}
            </div>
          </section>
          {run.next_output_offset != null ? (
            <button
              className="btn btn-sm"
              onClick={() => setOutputOffset(run.next_output_offset!)}
            >
              More outputs
            </button>
          ) : null}
          {outputOffset > 0 ? (
            <button className="btn btn-sm" onClick={() => setOutputOffset(0)}>
              First outputs
            </button>
          ) : null}
          {run.next_task_cursor != null ? (
            <button
              className="btn btn-sm"
              onClick={() => {
                setCursor(run.next_task_cursor!);
                setSelected(null);
              }}
            >
              Next tasks
            </button>
          ) : null}
          {cursor > 0 ? (
            <button className="btn btn-sm" onClick={() => setCursor(0)}>
              First tasks
            </button>
          ) : null}
        </>
      ) : null}
    </section>
  );
}
