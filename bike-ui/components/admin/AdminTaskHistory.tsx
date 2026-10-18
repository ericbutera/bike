"use client";

import { useState } from "react";
import { formatTaskDate } from "@/lib/taskTiming";
import {
  useTaskHistory,
  useTaskInput,
  type TaskExecution,
  type TaskStatus,
} from "@/lib/adminTaskHistory";
import { DataGrid, ListCard, type Column } from "../ui/DataGrid";
import Modal from "../ui/Modal";
import TemporalLink from "../ui/TemporalLink";

const statusLabels: Record<TaskStatus, string> = {
  Running: "Running / queued",
  Completed: "Completed",
  Failed: "Failed",
  Canceled: "Canceled",
  Terminated: "Terminated",
  ContinuedAsNew: "Continued as new",
  TimedOut: "Timed out",
  Unspecified: "Unknown",
};

function time(value?: string | null) {
  return formatTaskDate(value) || "—";
}

const columns: Column<TaskExecution, { open: (row: TaskExecution) => void }>[] =
  [
    {
      key: "task",
      header: "Task",
      render: ({ job }) => (
        <span title={`${job.job_id} / ${job.run_id}`}>
          {job.workflow_type.replaceAll("_", " ")}
        </span>
      ),
    },
    {
      key: "status",
      header: "Status",
      render: ({ status }) => (
        <span className="badge badge-outline whitespace-nowrap">
          {statusLabels[status]}
        </span>
      ),
    },
    {
      key: "started",
      header: "Started",
      render: (row) => time(row.started_at),
    },
    { key: "closed", header: "Finished", render: (row) => time(row.closed_at) },
    {
      key: "parameters",
      header: "Parameters",
      render: (row, params) => (
        <button
          className="btn btn-ghost btn-sm"
          onClick={() => params?.open(row)}
        >
          View parameters
        </button>
      ),
    },
  ];

function TaskParameters({
  execution,
  onClose,
}: {
  execution: TaskExecution;
  onClose: () => void;
}) {
  const input = useTaskInput(execution.job);
  return (
    <Modal title="Task parameters" onClose={onClose}>
      <p className="my-3 font-medium">
        {execution.job.workflow_type.replaceAll("_", " ")}
      </p>
      <p className="mb-3 text-sm">
        {statusLabels[execution.status]} · Started {time(execution.started_at)}
      </p>
      {input.isLoading ? <p>Loading parameters...</p> : null}
      {input.error ? (
        <p role="alert" className="text-error">
          Parameters are unavailable. Try again or open Temporal.
        </p>
      ) : null}
      {input.data ? (
        <pre className="my-4 max-h-96 overflow-auto rounded bg-base-200 p-4 text-sm">
          {JSON.stringify(input.data.parameters, null, 2)}
        </pre>
      ) : null}
      <TemporalLink
        workflowId={execution.job.job_id}
        runId={execution.job.run_id}
        namespace={execution.job.namespace}
      />
    </Modal>
  );
}

export default function AdminTaskHistory() {
  const [status, setStatus] = useState<TaskStatus | "">("");
  const [pages, setPages] = useState<string[]>([]);
  const [selected, setSelected] = useState<TaskExecution>();
  const history = useTaskHistory(status || undefined, pages.at(-1));
  const next = history.data?.next_page_token;

  function refresh() {
    if (pages.length) setPages([]);
    else void history.refetch();
  }

  return (
    <>
      <ListCard
        title="Task history"
        actions={
          <button
            className="btn btn-sm"
            onClick={refresh}
            disabled={history.isFetching}
          >
            Refresh
          </button>
        }
        filters={
          <select
            className="select select-sm select-bordered"
            aria-label="Task status"
            value={status}
            onChange={(event) => {
              setStatus(event.target.value as TaskStatus | "");
              setPages([]);
            }}
          >
            <option value="">All statuses</option>
            {Object.entries(statusLabels).map(([value, label]) => (
              <option key={value} value={value}>
                {label}
              </option>
            ))}
          </select>
        }
      >
        <p className="mb-4 text-sm text-base-content/70">
          Recent Bike and Strava workflows, including child tasks. New runs may
          take a moment to appear; history follows Temporal retention.
        </p>
        <DataGrid
          rows={history.data?.executions ?? []}
          columns={columns}
          params={{ open: setSelected }}
          isLoading={history.isLoading}
          error={
            history.error
              ? "Task history is unavailable. Try again or open Temporal."
              : undefined
          }
          emptyMessage="No tasks match this status."
          label="Task history"
        />
        {pages.length || next ? (
          <div className="mt-4 flex justify-end gap-2">
            {pages.length ? (
              <button
                className="btn btn-sm"
                onClick={() => setPages((current) => current.slice(0, -1))}
                disabled={history.isFetching}
              >
                Newer
              </button>
            ) : null}
            {next ? (
              <button
                className="btn btn-sm"
                onClick={() => setPages((current) => [...current, next])}
                disabled={history.isFetching}
              >
                Older
              </button>
            ) : null}
          </div>
        ) : null}
      </ListCard>
      {selected ? (
        <TaskParameters
          key={selected.job.run_id}
          execution={selected}
          onClose={() => setSelected(undefined)}
        />
      ) : null}
    </>
  );
}
