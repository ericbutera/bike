"use client";
import { useState } from "react";
import { useActivityPipelines } from "../../lib/queries";
import PipelinePanel from "./PipelinePanel";
import { LoadingSpinner } from "../ui/QueryState";

export default function ActivityPipelines({
  activityId,
}: {
  activityId: number;
}) {
  const [after, setAfter] = useState<string>();
  const [selected, setSelected] = useState<string>();
  const query = useActivityPipelines(activityId, after);
  const runId = query.data?.run_ids.includes(selected ?? "")
    ? selected
    : query.data?.run_ids[0];
  return (
    <section aria-label="Activity processing pipelines" className="space-y-3">
      {query.isLoading ? (
        <p className="flex items-center gap-2">
          <LoadingSpinner size="sm" />
          Loading worker runs…
        </p>
      ) : null}
      {query.error ? (
        <p role="alert">Unable to load related processing runs.</p>
      ) : null}
      {query.data?.run_ids.length === 0 ? (
        <p>
          No pipeline lineage was recorded for this activity. Historical work
          remains unknown until reprocessing.
        </p>
      ) : null}
      {query.data?.run_ids.length ? (
        <div aria-label="Worker run history" className="flex flex-wrap gap-2">
          {query.data.run_ids.map((id, index) => (
            <button
              type="button"
              key={id}
              aria-pressed={id === runId}
              className={`max-w-full rounded-box border p-3 text-left ${id === runId ? "border-primary bg-primary/5" : "border-base-300"}`}
              onClick={() => setSelected(id)}
            >
              <span className="block text-sm font-semibold">
                Run {index + 1}
              </span>
              <span className="block break-all font-mono text-xs text-base-content/60">
                {id}
              </span>
            </button>
          ))}
        </div>
      ) : null}
      {runId ? <PipelinePanel key={runId} runId={runId} /> : null}
      {query.data?.next_cursor ? (
        <button
          className="btn btn-sm"
          onClick={() => {
            setAfter(query.data!.next_cursor!);
            setSelected(undefined);
          }}
        >
          More runs
        </button>
      ) : null}
      {after ? (
        <button
          type="button"
          className="btn btn-sm"
          onClick={() => {
            setAfter(undefined);
            setSelected(undefined);
          }}
        >
          First runs
        </button>
      ) : null}
    </section>
  );
}
