"use client";

import {
  faBan,
  faGear,
  faRotateRight,
} from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { useMemo, useState, type FormEvent } from "react";
import toast from "react-hot-toast";
import Pagination from "../ui/Pagination";
import {
  useAdminTask,
  useAdminTaskCancel,
  useAdminTaskRerun,
  useAdminTasks,
  type AdminTask,
} from "../../lib/queries";

type TaskFilters = {
  error: string;
  taskType: string;
  status: string;
  fromDate: string;
  toDate: string;
};

type TimedTask = AdminTask & {
  durationMs: number | null;
  slowerBaselineMs: number | null;
};

const emptyFilters: TaskFilters = {
  error: "",
  taskType: "",
  status: "",
  fromDate: "",
  toDate: "",
};

const taskTypes = [
  "email_registration",
  "email_password_reset",
  "email_notification",
  "resize_image",
  "zip_import",
  "process_activity_import",
  "reprocess_activity_import",
  "reprocess_user_activity_imports",
  "activity_archive_import",
  "strava_sync",
  "rebuild_fitness_freshness",
  "rebuild_segment_analytics",
  "regenerate_segment_efforts",
  "regenerate_user_segments",
  "backfill_user_xc_training",
];

export default function TasksPageContent() {
  const [draft, setDraft] = useState<TaskFilters>(emptyFilters);
  const [filters, setFilters] = useState<TaskFilters>(emptyFilters);
  const [page, setPage] = useState(1);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const query = useAdminTasks({
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
  const selectedTask = tasks.find((task) => task.id === selectedId);
  const detail = detailQuery.data;
  const total = query.metadata?.total ?? 0;
  const perPage = query.metadata?.per_page ?? 20;

  function setDraftField(key: keyof TaskFilters, value: string) {
    setDraft((current) => ({ ...current, [key]: value }));
    setFilters((current) => ({ ...current, [key]: value }));
    setPage(1);
  }

  function applyFilters(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
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

  return (
    <div className="card bg-base-100 shadow-xl">
      <div className="card-body">
        <div className="mb-4 flex flex-col gap-4">
          <h2 className="card-title m-0 text-2xl font-bold">
            Background Tasks
          </h2>
          <form
            onSubmit={applyFilters}
            className="flex w-full flex-wrap items-center justify-end gap-2 rounded-lg bg-base-200/50 p-2"
          >
            <div className="flex flex-1 flex-wrap justify-end gap-2">
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
                onChange={(event) =>
                  setDraftField("status", event.target.value)
                }
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
                onChange={(event) =>
                  setDraftField("toDate", event.target.value)
                }
              />
            </div>
            <div className="join">
              <button
                className="join-item btn btn-sm btn-neutral"
                type="submit"
              >
                Search
              </button>
              <button
                className="join-item btn btn-sm btn-ghost border-base-300"
                type="button"
                onClick={clearFilters}
              >
                Clear
              </button>
            </div>
          </form>
        </div>

        {query.isLoading ? (
          <p className="p-6 text-sm text-base-content/70">Loading tasks…</p>
        ) : null}
        {query.error ? (
          <p role="alert" className="p-6 text-sm text-error">
            Unable to load tasks.
          </p>
        ) : null}
        {!query.isLoading && !query.error && tasks.length === 0 ? (
          <p className="p-6 text-sm text-base-content/70">
            No tasks found matching criteria.
          </p>
        ) : null}
        {tasks.length > 0 ? (
          <div className="overflow-x-auto">
            <table className="table table-zebra w-full min-w-[820px]">
              <thead>
                <tr>
                  <th>ID</th>
                  <th>Type</th>
                  <th>Status</th>
                  <th>Duration</th>
                  <th>Trend</th>
                  <th>Attempts</th>
                  <th>Error</th>
                  <th>Created</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {tasks.map((task) => {
                  const canCancel =
                    task.status === "processing" || task.status === "running";
                  const actionPending = cancel.isPending || rerun.isPending;
                  return (
                    <tr
                      key={task.id}
                      className="cursor-pointer"
                      onClick={() => setSelectedId(task.id)}
                    >
                      <td>{task.id}</td>
                      <td>{task.task_type}</td>
                      <td>{task.status}</td>
                      <td
                        title={
                          task.durationMs === null
                            ? undefined
                            : String(Math.round(task.durationMs / 1000)) + "s"
                        }
                      >
                        {formatDuration(task.durationMs)}
                      </td>
                      <td>
                        {task.slowerBaselineMs === null ? (
                          <span className="text-base-content/50">-</span>
                        ) : (
                          <span
                            className="badge badge-warning badge-sm whitespace-nowrap"
                            title={
                              "Slower than visible baseline " +
                              formatDuration(task.slowerBaselineMs)
                            }
                          >
                            Slower
                          </span>
                        )}
                      </td>
                      <td>
                        {task.attempts}/{task.max_attempts}
                      </td>
                      <td
                        className="max-w-xs truncate"
                        title={task.error ?? ""}
                      >
                        {task.error}
                      </td>
                      <td>{formatDate(task.created_at)}</td>
                      <td
                        className="text-right"
                        onClick={(event) => event.stopPropagation()}
                      >
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
                                disabled={actionPending}
                                onClick={() => void runAction("rerun", task.id)}
                              >
                                <FontAwesomeIcon icon={faRotateRight} /> Rerun
                              </button>
                            </li>
                            {canCancel ? (
                              <li>
                                <button
                                  type="button"
                                  disabled={actionPending}
                                  onClick={() =>
                                    void runAction("cancel", task.id)
                                  }
                                >
                                  <FontAwesomeIcon icon={faBan} /> Cancel
                                </button>
                              </li>
                            ) : null}
                          </ul>
                        </details>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        ) : null}
        {total > 0 ? (
          <div className="mt-4">
            <Pagination
              page={page}
              perPage={perPage}
              total={total}
              onPageChange={setPage}
            />
          </div>
        ) : null}
      </div>

      {selectedId !== null ? (
        <div
          className="modal modal-open"
          role="dialog"
          aria-modal="true"
          aria-label={"Task " + selectedId + " detail"}
          onClick={() => setSelectedId(null)}
        >
          <div
            className="modal-box max-w-3xl"
            onClick={(event) => event.stopPropagation()}
          >
            <h3 className="text-lg font-bold">
              Task #{selectedId} —{" "}
              {selectedTask?.task_type ?? detail?.task_type}
            </h3>
            {detailQuery.isLoading ? (
              <p className="mt-4">Loading task…</p>
            ) : null}
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
                      String(detail.attempts) +
                      "/" +
                      String(detail.max_attempts)
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
                <div className="mt-4 text-sm">
                  <strong>Payload:</strong>
                  <pre className="mt-1 max-h-48 w-full max-w-full overflow-auto rounded bg-base-200 p-2 text-xs">
                    {formatPayload(detail.payload)}
                  </pre>
                </div>
              </>
            ) : null}
            <div className="modal-action">
              <button
                className="btn"
                type="button"
                onClick={() => setSelectedId(null)}
              >
                Close
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </div>
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

function formatDate(value?: string | null) {
  if (!value) return "";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
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

function formatDuration(value: number | null) {
  if (value === null) return "";
  const seconds = Math.max(0, Math.round(value / 1000));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (hours) return String(hours) + "h " + String(minutes) + "m";
  if (minutes) return String(minutes) + "m " + String(seconds % 60) + "s";
  return String(seconds) + "s";
}

function addDurations(tasks: AdminTask[]): TimedTask[] {
  const timed = tasks.map((task) => ({
    ...task,
    durationMs: durationMs(task),
    slowerBaselineMs: null as number | null,
  }));
  const byType = new Map<string, TimedTask[]>();
  for (const task of timed) {
    if (task.status !== "completed" || task.durationMs === null) continue;
    const group = byType.get(task.task_type) ?? [];
    group.push(task);
    byType.set(task.task_type, group);
  }
  for (const group of byType.values()) {
    group.sort(
      (a, b) =>
        new Date(a.created_at).getTime() - new Date(b.created_at).getTime(),
    );
    for (let index = 2; index < group.length; index += 1) {
      const baseline =
        group
          .slice(0, index)
          .reduce((sum, task) => sum + (task.durationMs ?? 0), 0) / index;
      if (baseline > 0 && (group[index].durationMs ?? 0) > baseline * 1.5)
        group[index].slowerBaselineMs = baseline;
    }
  }
  return timed;
}
