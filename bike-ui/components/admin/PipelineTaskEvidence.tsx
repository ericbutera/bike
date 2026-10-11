import type { PipelineGraph, PipelineTask } from "../../lib/pipelineGraph";
import {
  formatTaskDate,
  formatTaskInterval as duration,
} from "../../lib/taskTiming";
import { ProcessingFacts, ProcessingStatus } from "../ui/ProcessingDetails";
import EvidenceLinks from "./PipelineEvidenceLinks";

export default function TaskEvidence({
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
      <div className="mb-4 flex items-center justify-between gap-2">
        <h5 className="font-semibold">
          Task #{task.id}: {task.task_type.replaceAll("_", " ")}
        </h5>
        <ProcessingStatus status={task.status} />
      </div>
      <ProcessingFacts
        facts={[
          {
            label: "Receipt to queue",
            value: duration(run.received_at, task.created_at),
          },
          {
            label: "Receipt to first start",
            value: duration(run.received_at, task.attempts[0]?.started_at),
          },
          {
            label: "Parent task",
            value: task.parent_task_id
              ? `#${task.parent_task_id}`
              : "No parent task",
          },
        ]}
      />
      {work || stage || attempt ? (
        <section
          aria-label="Selected step timing"
          className="my-4 border-t border-base-300 pt-4"
        >
          <ProcessingFacts
            facts={[
              {
                label: "Step duration",
                value: duration(started, finished ?? (running ? now : null)),
              },
              {
                label: "Receipt to step start",
                value: duration(run.received_at, started),
              },
              {
                label: "Receipt to step result",
                value: duration(run.received_at, finished),
              },
            ]}
          />
        </section>
      ) : null}
      {work ? (
        <>
          <ProcessingFacts
            facts={[
              { label: "Work key", value: work.work_key },
              { label: "Revision", value: work.revision },
            ]}
          />
          {work.error ? (
            <p role="alert" className="text-error">
              {work.error}
            </p>
          ) : null}
          <EvidenceLinks run={run} traceId={work.trace_id} taskId={task.id} />
        </>
      ) : null}
      {stage ? (
        <section className="my-4 space-y-2">
          <h6 className="font-semibold">{stage.stage.replaceAll("_", " ")}</h6>
          <ProcessingStatus status={stage.status} />
          <ul className="space-y-1 text-sm">
            {stage.summary.map((fact) => (
              <li key={fact}>{fact}</li>
            ))}
          </ul>
          {stage.error ? (
            <p role="alert" className="text-error">
              {stage.error}
            </p>
          ) : null}
          {stage.reused_attempt_id ? (
            <p className="text-sm">
              Reused from attempt #{stage.reused_attempt_id}
            </p>
          ) : null}
        </section>
      ) : null}
      {task.attempts.map((attempt) => (
        <section
          key={attempt.attempt}
          className="my-4 rounded-box border border-base-300 p-3"
        >
          <div className="mb-3 flex items-center justify-between gap-2">
            <h6 className="font-semibold">Worker attempt {attempt.attempt}</h6>
            <ProcessingStatus status={attempt.outcome} />
          </div>
          <ProcessingFacts
            facts={[
              {
                label: "Duration",
                value: duration(
                  attempt.started_at,
                  attempt.finished_at ??
                    (attempt.outcome === "running" ? now : null),
                ),
              },
              {
                label: "Started",
                value: formatTaskDate(attempt.started_at) || null,
              },
              {
                label: "Heartbeat",
                value: formatTaskDate(attempt.heartbeat_at) || null,
              },
              {
                label: "Progress",
                value: formatTaskDate(attempt.progress_at) || null,
              },
            ]}
          />
          {attempt.error ? (
            <p className="mt-2 text-sm text-error" role="alert">
              {attempt.error}
            </p>
          ) : null}
          <EvidenceLinks
            run={run}
            traceId={attempt.trace_id}
            taskId={task.id}
          />
        </section>
      ))}
      {task.anomalies.map((anomaly, index) => (
        <section
          key={index}
          className="my-4 rounded-box border border-warning p-3"
        >
          <h6 className="mb-3 font-semibold text-warning">{anomaly.reason}</h6>
          <ProcessingFacts
            facts={[
              { label: "Observed", value: anomaly.observed },
              { label: "Expected", value: anomaly.expected },
              { label: "Policy", value: anomaly.policy_version },
              { label: "Baseline samples", value: anomaly.baseline_samples },
              {
                label: "Resolution",
                value: formatTaskDate(anomaly.resolved_at) || "Unresolved",
              },
            ]}
          />
        </section>
      ))}
      {task.work.map((work) => (
        <section
          key={work.id}
          className="my-4 rounded-box border border-base-300 p-3"
        >
          <div className="mb-3 flex items-center justify-between gap-2">
            <h6 className="font-semibold">{work.kind.replaceAll("_", " ")}</h6>
            <ProcessingStatus status={work.outcome} />
          </div>
          <ProcessingFacts
            facts={[
              { label: "Mode", value: work.mode },
              { label: "Reason", value: work.reason },
              {
                label: "Duration",
                value: duration(work.started_at, work.finished_at),
              },
              ...Object.entries(work.counts ?? {}).map(([key, value]) => ({
                label: key.replaceAll("_", " "),
                value: String(value),
              })),
            ]}
          />
        </section>
      ))}
    </div>
  );
}
