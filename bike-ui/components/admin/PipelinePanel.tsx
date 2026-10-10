"use client";
import { useState } from "react";
import { usePipelineGraph } from "../../lib/queries";
import {
  pipelineMermaid,
  gatewaySteps,
  type PipelineGraph,
  type PipelineTask,
} from "../../lib/pipelineGraph";
import { formatTaskDuration, millisecondsBetween } from "../../lib/taskTiming";
import MermaidFlowchart from "../ui/MermaidFlowchart";

const duration = (start?: string | null, end?: string | null) =>
  formatTaskDuration(millisecondsBetween(start, end)) || "N/A";
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
  const task = run?.tasks.find((task) => task.id === taskId);
  const gateway =
    selected?.startsWith("gateway_") && run
      ? gatewaySteps(run)[Number(selected.split("_")[1])]
      : null;
  return (
    <section aria-label="Pipeline graph" className="space-y-3">
      {query.isLoading ? <p>Loading pipeline…</p> : null}
      {query.error ? <p role="alert">Unable to load pipeline.</p> : null}
      {run ? (
        <>
          <h4 className="font-semibold">Received → processing → available</h4>
          <p className="text-sm">
            {run.entrypoint} ·{" "}
            {run.available_at
              ? "Available"
              : "Readiness pending or not recorded"}{" "}
            · receipt → current{" "}
            {duration(run.received_at, new Date().toISOString())} · receipt →
            available {duration(run.received_at, run.available_at)} · receipt →
            end {duration(run.received_at, run.ended_at)}
          </p>
          <p className="break-all font-mono text-xs">
            Run {run.run_id} · Request {run.request_id ?? "unknown"} · Trace{" "}
            {run.trace_id ?? "unknown"}
          </p>
          <EvidenceLinks run={run} traceId={run.trace_id} />
          <MermaidFlowchart
            chart={pipelineMermaid(run)}
            onSelect={(node) => {
              setSelected(node);
              setWorkAfter(0);
            }}
          />
          {gateway ? (
            <p>
              {gateway.kind} ·{" "}
              {duration(gateway.started_at, gateway.finished_at)} · receipt →
              result {duration(run.received_at, gateway.finished_at)} · attempts{" "}
              {gateway.attempts ?? "unknown"}
            </p>
          ) : null}
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
          <ul>
            {run.outputs.map((output, index) => (
              <li key={index}>
                {output.kind} #{output.target_id} · {output.status} · revision{" "}
                {output.revision} · published {output.available_at ?? "pending"}
              </li>
            ))}
          </ul>
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
function TaskEvidence({
  run,
  task,
  selected,
}: {
  run: PipelineGraph;
  task: PipelineTask;
  selected: string | null;
}) {
  const now = new Date().toISOString();
  const workIndex = selected?.match(/^work_\d+_(\d+)$/)?.[1];
  const work = workIndex == null ? undefined : task.work[Number(workIndex)];
  const stages = task.imports.flatMap((imported) =>
    imported.stages.map((stage) => ({
      ...stage,
      attemptId: imported.attempt_id,
    })),
  );
  const stage = stages.find(
    (stage) =>
      selected === `stage_${task.id}_${stage.attemptId}_${stage.stage}`,
  );
  const attemptNumber = selected?.match(/^try_\d+_(\d+)$/)?.[1];
  const attempt = task.attempts.find(
    (attempt) => attempt.attempt === Number(attemptNumber),
  );
  const started = work?.started_at ?? attempt?.started_at ?? stage?.started_at;
  const finished =
    work?.finished_at ?? attempt?.finished_at ?? stage?.completed_at;
  const running = attempt?.outcome === "running" || stage?.status === "running";
  return (
    <div className="rounded border border-base-300 p-3">
      <h5>
        Task #{task.id} · {task.task_type} · {task.status}
      </h5>
      <p>
        Receipt → queued {duration(run.received_at, task.created_at)} · receipt
        → first start {duration(run.received_at, task.attempts[0]?.started_at)}
      </p>
      {work || stage || attempt ? (
        <p>
          Selected step ·{" "}
          {duration(started, finished ?? (running ? now : null))} · receipt →
          start {duration(run.received_at, started)} · receipt → result{" "}
          {duration(run.received_at, finished)}
        </p>
      ) : null}
      {work ? (
        <>
          <p>
            {work.work_key} · revision {work.revision} · {work.error}
          </p>
          <EvidenceLinks run={run} traceId={work.trace_id} taskId={task.id} />
        </>
      ) : null}
      {stage ? (
        <p>
          {stage.stage} · {stage.status} · {stage.summary.join(" · ")} ·{" "}
          {stage.error}{" "}
          {stage.reused_attempt_id
            ? ` · reused attempt ${stage.reused_attempt_id}`
            : ""}
        </p>
      ) : null}
      {task.attempts.map((attempt) => (
        <div key={attempt.attempt} className="my-2">
          Attempt {attempt.attempt} · {attempt.outcome} ·{" "}
          {duration(
            attempt.started_at,
            attempt.finished_at ?? (attempt.outcome === "running" ? now : null),
          )}
          <p className="text-xs">
            Heartbeat {attempt.heartbeat_at} · Progress {attempt.progress_at}
          </p>
          <p className="break-all font-mono text-xs">
            {attempt.error} · Trace {attempt.trace_id ?? "unknown"}
          </p>
          <EvidenceLinks
            run={run}
            traceId={attempt.trace_id}
            taskId={task.id}
          />
        </div>
      ))}
      {task.anomalies.map((anomaly, index) => (
        <p key={index} className="text-warning">
          {anomaly.reason} · observed {anomaly.observed} · budget/baseline{" "}
          {anomaly.expected} · policy {anomaly.policy_version} · baseline
          samples {anomaly.baseline_samples} ·{" "}
          {anomaly.resolved_at ? "resolved" : "recorded"}
        </p>
      ))}
      {task.work.map((work) => (
        <p key={work.id} className="text-xs">
          {work.kind} · {work.mode} · {work.reason} · {work.outcome} ·{" "}
          {duration(work.started_at, work.finished_at)} ·{" "}
          {JSON.stringify(work.counts)}
        </p>
      ))}
    </div>
  );
}
function EvidenceLinks({
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
