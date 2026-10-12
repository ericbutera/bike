"use client";

import {
  faBan,
  faGear,
  faRotateRight,
} from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { useMemo, useState } from "react";
import toast from "react-hot-toast";
import Pagination from "../ui/Pagination";
import { DataGrid, ListCard, type Column } from "../ui/DataGrid";
import TaskProcessingHistory from "./TaskProcessingHistory";
import ProcessorOverview from "./ProcessorOverview";
import Modal from "../ui/Modal";
import {
  formatTaskDate as formatDate,
  formatTaskDuration as formatDuration,
} from "../../lib/taskTiming";
import {
  useAdminTask,
  useAdminTaskCancel,
  useAdminTaskRerun,
  useAdminTasks,
  useWorkerProcessors,
  type AdminTask,
} from "../../lib/queries";

type TaskFilters = {
  correlationId: string;
  error: string;
  taskType: string;
  status: string;
  fromDate: string;
  toDate: string;
};

type TimedTask = AdminTask & {
  durationMs: number | null;
};

const emptyFilters: TaskFilters = {
  correlationId: "",
  error: "",
  taskType: "",
  status: "completed",
  fromDate: "",
  toDate: "",
};

export default function TasksPageContent({
  initialCorrelationId = "",
}: {
  initialCorrelationId?: string;
}) {
  const processors = useWorkerProcessors();
  const [draft, setDraft] = useState<TaskFilters>({
    ...emptyFilters,
    correlationId: initialCorrelationId,
    status: initialCorrelationId ? "" : emptyFilters.status,
  });
  const [filters, setFilters] = useState<TaskFilters>({
    ...emptyFilters,
    correlationId: initialCorrelationId,
    status: initialCorrelationId ? "" : emptyFilters.status,
  });
  const [page, setPage] = useState(1);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const query = useAdminTasks({
    correlationId: filters.correlationId,
    page,
    perPage: 20,
    taskType: filters.taskType,
    status: filters.status,
    error: filters.error,
    fromDate: dateBoundary(filters.fromDate),
    toDate: dateBoundary(filters.toDate),
  });
  const detailQuery = useAdminTask(selectedId);
  const cancel = useAdminTaskCancel();
  const rerun = useAdminTaskRerun();
  const tasks = useMemo(() => addDurations(query.data ?? []), [query.data]);
  const taskTypes = [
    ...new Set([
      ...(processors.data?.map((processor) => processor.task_type) ?? []),
      ...tasks.map((task) => task.task_type),
      ...(draft.taskType ? [draft.taskType] : []),
    ]),
  ].sort();
  const selectedTask = tasks.find((task) => task.id === selectedId);
  const detail = detailQuery.data;
  const total = query.metadata?.total ?? 0;
  const perPage = query.metadata?.per_page ?? 20;

  function setDraftField(key: keyof TaskFilters, value: string) {
    setDraft((current) => ({ ...current, [key]: value }));
    setFilters((current) => ({ ...current, [key]: value }));
    setPage(1);
  }

  function applyFilters() {
    setFilters({ ...draft });
    setPage(1);
  }

  function clearFilters() {
    setDraft(emptyFilters);
    setFilters(emptyFilters);
    setPage(1);
  }

  async function runAction(action: "cancel" | "rerun", id: number) {
    try {
      if (action === "cancel") {
        await cancel.mutateAsync({ params: { path: { id } } });
      } else {
        await rerun.mutateAsync({ params: { path: { id } } });
      }
      await query.refetch();
      if (selectedId === id) await detailQuery.refetch();
      toast.success(
        action === "cancel" ? "Task canceled" : "Task queued again",
      );
    } catch {
      toast.error(
        action === "cancel" ? "Could not cancel task" : "Could not rerun task",
      );
    }
  }

  const columns: Column<TimedTask>[] = [
    { key: "id", header: "ID", render: (task) => task.id },
    {
      key: "type",
      header: "Type",
      className: "whitespace-nowrap",
      render: (task) => task.task_type,
    },
    { key: "status", header: "Status", render: (task) => task.status },
    {
      key: "duration",
      header: "Duration",
      className: "whitespace-nowrap",
      render: (task) => (
        <span
          title={
            task.durationMs === null
              ? undefined
              : Math.round(task.durationMs / 1000) + "s"
          }
        >
          {formatDuration(task.durationMs)}
        </span>
      ),
    },
    {
      key: "attempts",
      header: "Attempts",
      render: (task) => `${task.attempts}/${task.max_attempts}`,
    },
    {
      key: "error",
      header: "Error",
      className: "max-w-xs truncate",
      render: (task) => (
        <span title={task.error ?? undefined}>{task.error || "—"}</span>
      ),
    },
    {
      key: "created",
      header: "Created",
      className: "whitespace-nowrap",
      render: (task) => formatDate(task.created_at),
    },
    {
      key: "actions",
      header: "",
      className: "text-right",
      render: (task) => (
        <TaskActions
          task={task}
          pending={cancel.isPending || rerun.isPending}
          onAction={runAction}
        />
      ),
    },
  ];

  return (
    <>
      <ProcessorOverview onSelectTask={setSelectedId} />
      <ListCard
        title={
          filters.status === "completed" ? "Completed tasks" : "Task history"
        }
        onSearch={applyFilters}
        onClear={clearFilters}
        filters={
          <>
            <input
              type="search"
              aria-label="Request, trace, or pipeline ID"
              placeholder="Request, trace, or pipeline ID"
              className="input input-sm w-64"
              value={draft.correlationId}
              onChange={(event) =>
                setDraftField("correlationId", event.target.value)
              }
            />
            <input
              type="search"
              aria-label="Filter error text"
              placeholder="Filter error text"
              className="input input-sm w-40"
              value={draft.error}
              onChange={(event) => setDraftField("error", event.target.value)}
            />
            <select
              aria-label="Filter task type"
              className="select select-sm w-40"
              value={draft.taskType}
              onChange={(event) =>
                setDraftField("taskType", event.target.value)
              }
            >
              <option value="">All Types</option>
              {taskTypes.map((type) => (
                <option key={type} value={type}>
                  {type.replaceAll("_", " ")}
                </option>
              ))}
            </select>
            <select
              aria-label="Filter task status"
              className="select select-sm w-40"
              value={draft.status}
              onChange={(event) => setDraftField("status", event.target.value)}
            >
              <option value="">All Statuses</option>
              {(
                [
                  "pending",
                  "processing",
                  "running",
                  "canceled",
                  "completed",
                  "failed",
                ] as const
              ).map((status) => (
                <option key={status} value={status}>
                  {status[0].toUpperCase() + status.slice(1)}
                </option>
              ))}
            </select>
            <input
              type="date"
              aria-label="From date"
              title="From"
              className="input input-sm w-40"
              value={draft.fromDate}
              onChange={(event) =>
                setDraftField("fromDate", event.target.value)
              }
            />
            <input
              type="date"
              aria-label="To date"
              title="To"
              className="input input-sm w-40"
              value={draft.toDate}
              onChange={(event) => setDraftField("toDate", event.target.value)}
            />
          </>
        }
      >
        <DataGrid
          rows={tasks}
          columns={columns}
          isLoading={query.isLoading}
          error={query.error ? "Unable to load tasks." : undefined}
          emptyMessage="No tasks found matching criteria."
          label="Background task history"
          onRowClick={(task) => setSelectedId(task.id)}
        />
        {total > 0 ? (
          <Pagination
            className="mt-4"
            page={page}
            perPage={perPage}
            total={total}
            onPageChange={setPage}
          />
        ) : null}
      </ListCard>
      {selectedId !== null ? (
        <Modal
          title={`Task #${selectedId} — ${selectedTask?.task_type ?? detail?.task_type ?? "Detail"}`}
          onClose={() => setSelectedId(null)}
        >
          {detailQuery.isLoading ? <p className="mt-4">Loading task…</p> : null}
          {detailQuery.error ? (
            <p role="alert" className="mt-4 text-error">
              Unable to load task detail.
            </p>
          ) : null}
          {detail ? (
            <>
              <dl className="mt-4 grid grid-cols-1 items-start gap-4 text-sm sm:grid-cols-2">
                <TaskField label="Status" value={detail.status} />
                <TaskField
                  label="Attempts"
                  value={
                    String(detail.attempts) + "/" + String(detail.max_attempts)
                  }
                />
                <TaskField
                  label="Scheduled"
                  value={formatDate(detail.scheduled_for)}
                />
                <TaskField label="Error" value={detail.error} />
                <TaskField
                  label="Started"
                  value={formatDate(detail.started_at)}
                />
                <TaskField
                  label="Completed"
                  value={formatDate(detail.completed_at)}
                />
                <TaskField
                  label="Created"
                  value={formatDate(detail.created_at)}
                />
                <TaskField
                  label="Updated"
                  value={formatDate(detail.updated_at)}
                />
              </dl>
              <TaskProcessingHistory
                detail={detail}
                onSelectTask={setSelectedId}
              />
              <div className="mt-4 text-sm">
                <strong>Payload:</strong>
                <pre className="mt-1 max-h-48 w-full max-w-full overflow-auto rounded bg-base-200 p-2 text-xs">
                  {formatPayload(detail.payload)}
                </pre>
              </div>
            </>
          ) : null}
        </Modal>
      ) : null}
    </>
  );
}

function TaskField({ label, value }: { label: string; value?: string | null }) {
  return (
    <div>
      <dt className="font-semibold">{label}:</dt>
      <dd className="whitespace-pre-wrap">{value || "—"}</dd>
    </div>
  );
}

function dateBoundary(value: string) {
  if (!value) return undefined;
  return new Date(value).toISOString();
}

function formatPayload(value: unknown) {
  if (value == null) return "—";
  try {
    return JSON.stringify(
      typeof value === "string" ? JSON.parse(value) : value,
      null,
      2,
    );
  } catch {
    return String(value);
  }
}

function durationMs(task: AdminTask) {
  if (!task.started_at) return null;
  const started = new Date(task.started_at).getTime();
  const finished = new Date(
    task.completed_at ??
      (task.status === "processing"
        ? new Date().toISOString()
        : task.updated_at),
  ).getTime();
  return Number.isFinite(started) &&
    Number.isFinite(finished) &&
    finished >= started
    ? finished - started
    : null;
}

function addDurations(tasks: AdminTask[]): TimedTask[] {
  return tasks.map((task) => ({ ...task, durationMs: durationMs(task) }));
}

function TaskActions({
  task,
  pending,
  onAction,
}: {
  task: AdminTask;
  pending: boolean;
  onAction: (action: "cancel" | "rerun", id: number) => Promise<void>;
}) {
  const canCancel = task.status === "processing" || task.status === "running";
  return (
    <div onClick={(event) => event.stopPropagation()}>
      <details className="dropdown dropdown-end">
        <summary
          className="btn btn-ghost btn-xs list-none"
          aria-label={"Task " + task.id + " actions"}
        >
          <FontAwesomeIcon icon={faGear} />
        </summary>
        <ul className="dropdown-content menu z-20 mt-2 w-40 rounded-box border border-base-300 bg-base-100 p-2 shadow-lg">
          <li>
            <button
              type="button"
              disabled={pending}
              onClick={() => void onAction("rerun", task.id)}
            >
              <FontAwesomeIcon icon={faRotateRight} /> Rerun
            </button>
          </li>
          {canCancel ? (
            <li>
              <button
                type="button"
                disabled={pending}
                onClick={() => void onAction("cancel", task.id)}
              >
                <FontAwesomeIcon icon={faBan} /> Cancel
              </button>
            </li>
          ) : null}
        </ul>
      </details>
    </div>
  );
}
