import type { components } from "../../lib/openapi/react-query/api";
import PipelinePanel from "./PipelinePanel";
import { useState } from "react";
import {
  formatTaskDate,
  formatTaskDuration,
  millisecondsBetween,
} from "../../lib/taskTiming";

type Detail = components["schemas"]["TaskDetailResponse"];
type Pipeline = components["schemas"]["TaskPipelineResponse"];

export default function TaskProcessingHistory({
  detail,
  onSelectTask,
}: {
  detail: Detail;
  onSelectTask: (id: number) => void;
}) {
  const now = new Date().toISOString();
  const attempts = detail.attempt_history ?? [];
  const pipelines = detail.pipelines ?? [];
  return (
    <section className="mt-6 space-y-4" aria-label="Processing history">
      <h4 className="font-semibold">Processing history</h4>
      {pipelines.length === 0 ? (
        <p className="text-sm text-base-content/70">
          Original receipt and task lineage were not recorded for this task.
        </p>
      ) : null}
      {pipelines.map((pipeline) => (
        <div key={pipeline.run_id} className="space-y-3">
          <PipelineClock
            key={pipeline.run_id}
            pipeline={pipeline}
            startedAt={attempts[0]?.started_at}
            finishedAt={detail.completed_at}
            now={now}
            onSelectTask={onSelectTask}
          />
          <PipelineDisclosure runId={pipeline.run_id} />
        </div>
      ))}
      {attempts.length === 0 ? (
        <p className="text-sm text-base-content/70">
          No execution attempts have been recorded.
        </p>
      ) : (
        <div className="overflow-x-auto">
          <table className="table table-sm">
            <caption className="text-left text-sm font-semibold">
              Execution attempts
            </caption>
            <thead>
              <tr>
                <th>Attempt</th>
                <th>Outcome</th>
                <th>Started</th>
                <th>Duration</th>
                <th>Last heartbeat</th>
                <th>Error / trace</th>
              </tr>
            </thead>
            <tbody>
              {attempts.map((attempt) => (
                <tr key={attempt.attempt}>
                  <td>{attempt.attempt}</td>
                  <td>{attempt.outcome}</td>
                  <td>{formatTaskDate(attempt.started_at)}</td>
                  <td>
                    {formatTaskDuration(
                      millisecondsBetween(
                        attempt.started_at,
                        attempt.finished_at ?? now,
                      ),
                    )}
                  </td>
                  <td>{formatTaskDate(attempt.heartbeat_at)}</td>
                  <td className="max-w-xs break-words">
                    {attempt.error ? <p>{attempt.error}</p> : null}
                    {attempt.trace_id ? (
                      <p className="font-mono text-xs">
                        Trace: {attempt.trace_id}
                      </p>
                    ) : null}
                    {attempt.span_id ? (
                      <p className="font-mono text-xs">
                        Span: {attempt.span_id}
                      </p>
                    ) : null}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

function PipelineDisclosure({ runId }: { runId: string }) {
  const [open, setOpen] = useState(false);
  return (
    <details onToggle={(event) => setOpen(event.currentTarget.open)}>
      <summary>View complete pipeline</summary>
      {open ? <PipelinePanel runId={runId} /> : null}
    </details>
  );
}

function PipelineClock({
  pipeline,
  startedAt,
  finishedAt,
  now,
  onSelectTask,
}: {
  pipeline: Pipeline;
  startedAt?: string | null;
  finishedAt?: string | null;
  now: string;
  onSelectTask: (id: number) => void;
}) {
  const fromReceipt = (end?: string | null) =>
    formatTaskDuration(
      millisecondsBetween(pipeline.pipeline_started_at, end),
    ) || "Not recorded";
  return (
    <div className="rounded border border-base-300 p-3 text-sm">
      <p className="break-all font-mono">Pipeline: {pipeline.run_id}</p>
      <p>Entry point: {pipeline.entrypoint}</p>
      {pipeline.request_id ? (
        <p className="break-all">Request: {pipeline.request_id}</p>
      ) : null}
      {pipeline.trace_id ? (
        <p className="break-all">Trace: {pipeline.trace_id}</p>
      ) : null}
      <dl className="mt-2 grid gap-2 sm:grid-cols-2">
        <div>
          <dt>Original receipt</dt>
          <dd>{formatTaskDate(pipeline.pipeline_started_at)}</dd>
        </div>
        <div>
          <dt>Accepted</dt>
          <dd>{formatTaskDate(pipeline.accepted_at)}</dd>
        </div>
        <div>
          <dt>Receipt to task start</dt>
          <dd>{fromReceipt(startedAt)}</dd>
        </div>
        <div>
          <dt>Receipt to task finish</dt>
          <dd>{fromReceipt(finishedAt)}</dd>
        </div>
        <div>
          <dt>Receipt to now</dt>
          <dd>{fromReceipt(now)}</dd>
        </div>
        <div>
          <dt>Receipt to available</dt>
          <dd>{fromReceipt(pipeline.available_at)}</dd>
        </div>
      </dl>
      {pipeline.parent_task_id != null ? (
        <button
          type="button"
          className="btn btn-link btn-sm px-0"
          onClick={() => onSelectTask(pipeline.parent_task_id!)}
        >
          Parent task #{pipeline.parent_task_id}
        </button>
      ) : null}
    </div>
  );
}
