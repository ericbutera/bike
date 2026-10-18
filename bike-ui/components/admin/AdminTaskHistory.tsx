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
import Pagination from "../ui/Pagination";
import RefreshButton from "../ui/RefreshButton";
import StatusBadge from "../ui/StatusBadge";

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
        <StatusBadge status={status} label={statusLabels[status]} />
      ),
    },
    {
      key: "started",
      header: "Started",
      className: "whitespace-nowrap",
      render: (row) => time(row.started_at),
    },
    {
      key: "closed",
      header: "Finished",
      className: "whitespace-nowrap",
      render: (row) => time(row.closed_at),
    },
    {
      key: "parameters",
      header: "Parameters",
      render: (row, params) => (
        <button
          className="btn btn-ghost btn-sm"
          onClick={() => params?.open(row)}
        >
          Details
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
  const [userId, setUserId] = useState("");
  const validUserId =
    !userId || (Number.isInteger(Number(userId)) && Number(userId) > 0);
  const [pages, setPages] = useState<string[]>([]);
  const [page, setPage] = useState(1);
  const [selected, setSelected] = useState<TaskExecution>();
  const history = useTaskHistory(
    status || undefined,
    pages[page - 2],
    userId ? Number(userId) : undefined,
    validUserId,
  );
  const next = history.data?.next_page_token;

  function refresh() {
    if (page > 1) {
      setPages([]);
      setPage(1);
    } else {
      setPages([]);
      if (validUserId) void history.refetch();
    }
  }

  return (
    <>
      <ListCard
        title="Task history"
        onSearch={refresh}
        onClear={() => {
          setStatus("");
          setUserId("");
          setPages([]);
          setPage(1);
        }}
        filterActions={
          <RefreshButton
            onClick={refresh}
            disabled={history.isFetching || !validUserId}
          />
        }
        filters={
          <>
            <input
              aria-label="Task user ID"
              placeholder="Any user"
              type="number"
              min={1}
              step={1}
              aria-invalid={!validUserId}
              className="input input-sm input-bordered w-40"
              value={userId}
              onChange={(event) => {
                setUserId(event.target.value);
                setPages([]);
                setPage(1);
              }}
            />
            <select
              className="select select-sm select-bordered w-40"
              aria-label="Task status"
              value={status}
              onChange={(event) => {
                setStatus(event.target.value as TaskStatus | "");
                setPages([]);
                setPage(1);
              }}
            >
              <option value="">All statuses</option>
              {Object.entries(statusLabels).map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </select>
          </>
        }
      >
        <p className="mb-4 text-sm text-base-content/70">
          Recent Bike and Strava workflows, including child tasks. New runs may
          take a moment to appear; history follows Temporal retention.
        </p>
        {!validUserId ? (
          <p role="alert" className="text-error">
            User ID must be a positive integer.
          </p>
        ) : (
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
            emptyMessage="No tasks match these filters."
            label="Task history"
          />
        )}
        {validUserId && !history.error ? (
          <Pagination
            className="mt-4"
            page={page}
            cursor={{ knownPages: pages.length + 1, hasNextPage: !!next }}
            disabled={history.isFetching || history.isLoading}
            onPageChange={(value) => {
              if (value > pages.length + 1 && next)
                setPages((current) => [...current, next]);
              setPage(value);
            }}
          />
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
