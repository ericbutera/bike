"use client";
import { useState } from "react";
import { useWorkerProcessors } from "../../lib/queries";
import type { components } from "../../lib/openapi/react-query/api";
import { DataGrid, ListCard, type Column } from "../ui/DataGrid";

type Distribution = components["schemas"]["Distribution"];
type Processor = components["schemas"]["ProcessorSummary"];
const columns: Column<Processor>[] = [
  {
    key: "processor",
    header: "Processor",
    className: "whitespace-nowrap",
    render: (processor) => processor.task_type,
  },
  {
    key: "eligible",
    header: "Eligible / scheduled",
    className: "whitespace-nowrap",
    render: (processor) => `${processor.queued} / ${processor.scheduled}`,
  },
  {
    key: "running",
    header: "Running / retrying / failed",
    className: "whitespace-nowrap",
    render: (processor) =>
      `${processor.running} / ${processor.retrying} / ${processor.failed}`,
  },
  {
    key: "attempt",
    header: "Attempt p50 / p90 · n",
    className: "whitespace-nowrap",
    render: (processor) => <Percentiles distribution={processor.attempt} />,
  },
  {
    key: "wait",
    header: "Eligible wait p50 / p90 · n",
    className: "whitespace-nowrap",
    render: (processor) => (
      <Percentiles distribution={processor.eligible_wait} />
    ),
  },
  {
    key: "completion",
    header: "Logical completion p50 / p90 · n",
    className: "whitespace-nowrap",
    render: (processor) => (
      <Percentiles distribution={processor.logical_completion} />
    ),
  },
];

export default function ProcessorOverview() {
  const [hours, setHours] = useState(24);
  const [outcome, setOutcome] = useState("completed");
  const [search, setSearch] = useState("");
  const query = useWorkerProcessors(hours, outcome);
  const term = search.trim().toLowerCase();
  const rows = (query.data ?? []).filter((processor) =>
    processor.task_type.toLowerCase().includes(term),
  );
  return (
    <ListCard
      title="All registered processors"
      onSearch={() => {
        void query.refetch();
      }}
      onClear={() => {
        setHours(24);
        setOutcome("completed");
        setSearch("");
      }}
      filters={
        <>
          <select
            aria-label="Processor window"
            className="select select-sm select-bordered w-40"
            value={hours}
            onChange={(event) => setHours(Number(event.target.value))}
          >
            {[1, 24, 168].map((hours) => (
              <option key={hours} value={hours}>
                {hours} hours
              </option>
            ))}
          </select>
          <select
            aria-label="Attempt outcome"
            className="select select-sm select-bordered w-40"
            value={outcome}
            onChange={(event) => setOutcome(event.target.value)}
          >
            {["completed", "retrying", "failed", "interrupted", "canceled"].map(
              (value) => (
                <option key={value}>{value}</option>
              ),
            )}
          </select>
          <input
            type="search"
            aria-label="Search processors"
            placeholder="Search processors"
            className="input input-sm input-bordered w-56"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
          />
        </>
      }
    >
      <DataGrid
        rows={rows}
        columns={columns}
        isLoading={query.isLoading}
        error={query.error ? "Unable to load processor statistics." : undefined}
        emptyMessage="No processors match these filters."
        label="Registered processors"
      />
      <p className="text-sm text-base-content/70">
        Percentiles cover the full selected window. Logical completion includes
        waits and retries of successful tasks. N/A means no samples.
      </p>
    </ListCard>
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
