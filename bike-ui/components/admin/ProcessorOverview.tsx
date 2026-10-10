"use client";
import { useState } from "react";
import { useWorkerProcessors } from "../../lib/queries";
import type { components } from "../../lib/openapi/react-query/api";

type Distribution = components["schemas"]["Distribution"];
export default function ProcessorOverview() {
  const [hours, setHours] = useState(24);
  const [outcome, setOutcome] = useState("completed");
  const query = useWorkerProcessors(hours, outcome);
  return (
    <section aria-label="Processor overview" className="space-y-3">
      <div className="flex flex-wrap items-center gap-3">
        <h3 className="font-semibold">All registered processors</h3>
        <label>
          Window{" "}
          <select
            aria-label="Processor window"
            className="select select-sm"
            value={hours}
            onChange={(event) => setHours(Number(event.target.value))}
          >
            {[1, 24, 168].map((hours) => (
              <option key={hours} value={hours}>
                {hours} hours
              </option>
            ))}
          </select>
        </label>
        <label>
          Attempt outcome{" "}
          <select
            aria-label="Attempt outcome"
            className="select select-sm"
            value={outcome}
            onChange={(event) => setOutcome(event.target.value)}
          >
            {["completed", "retrying", "failed", "interrupted", "canceled"].map(
              (value) => (
                <option key={value}>{value}</option>
              ),
            )}
          </select>
        </label>
      </div>
      <p className="text-sm text-base-content/70">
        Percentiles cover the full selected window. Logical completion includes
        waits and retries of successful tasks. N/A means no samples.
      </p>
      {query.error ? (
        <p role="alert">Unable to load processor statistics.</p>
      ) : null}
      <div className="overflow-x-auto">
        <table className="table table-sm">
          <thead>
            <tr>
              <th>Processor</th>
              <th>Eligible / scheduled</th>
              <th>Running / retrying / failed</th>
              <th>Attempt p50 / p90 · n</th>
              <th>Eligible wait p50 / p90 · n</th>
              <th>Logical completion p50 / p90 · n</th>
            </tr>
          </thead>
          <tbody>
            {query.data?.map((processor) => (
              <tr key={processor.task_type}>
                <td>{processor.task_type}</td>
                <td>
                  {processor.queued} / {processor.scheduled}
                </td>
                <td>
                  {processor.running} / {processor.retrying} /{" "}
                  {processor.failed}
                </td>
                <td>
                  <Percentiles distribution={processor.attempt} />
                </td>
                <td>
                  <Percentiles distribution={processor.eligible_wait} />
                </td>
                <td>
                  <Percentiles distribution={processor.logical_completion} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}
function Percentiles({ distribution }: { distribution: Distribution }) {
  const seconds = (value: number | null | undefined) =>
    value == null ? "N/A" : `${value.toFixed(2)}s`;
  return (
    <>
      {seconds(distribution.p50_seconds)} / {seconds(distribution.p90_seconds)}{" "}
      · {distribution.samples}
    </>
  );
}
