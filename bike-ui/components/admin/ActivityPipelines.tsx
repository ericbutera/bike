"use client";
import { useState } from "react";
import { useActivityPipelines } from "../../lib/queries";
import PipelinePanel from "./PipelinePanel";

export default function ActivityPipelines({
  activityId,
}: {
  activityId: number;
}) {
  const [after, setAfter] = useState<string>();
  const [selected, setSelected] = useState<string>();
  const query = useActivityPipelines(activityId, after);
  const runId = selected ?? query.data?.run_ids[0];
  return (
    <section aria-label="Activity processing pipelines" className="space-y-3">
      <h3 className="font-semibold">All processing runs for this activity</h3>
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
        <select
          aria-label="Activity pipeline"
          className="select select-bordered"
          value={runId}
          onChange={(event) => setSelected(event.target.value)}
        >
          {query.data.run_ids.map((id) => (
            <option key={id}>{id}</option>
          ))}
        </select>
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
    </section>
  );
}
