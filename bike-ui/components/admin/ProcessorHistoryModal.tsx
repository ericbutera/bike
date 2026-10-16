"use client";

import { useAdminTasks, type AdminTask } from "../../lib/queries";
import { processorDescription } from "../../lib/processorDescriptions";
import { formatTaskDate, formatTaskInterval } from "../../lib/taskTiming";
import { DataGrid, type Column } from "../ui/DataGrid";
import InfoTooltip from "../ui/InfoTooltip";
import Modal from "../ui/Modal";

export default function ProcessorHistoryModal({
  taskType,
  onClose,
  onSelectTask,
}: {
  taskType: string;
  onClose: () => void;
  onSelectTask: (id: number) => void;
}) {
  const query = useAdminTasks({ taskType, page: 1, perPage: 10 });
  const columns: Column<AdminTask>[] = [
    {
      key: "id",
      header: "Task",
      render: (task) => (
        <button
          type="button"
          className="link link-primary whitespace-nowrap"
          onClick={(event) => {
            event.stopPropagation();
            onSelectTask(task.id);
          }}
        >
          #{task.id}
        </button>
      ),
    },
    { key: "status", header: "Status", render: (task) => task.status },
    {
      key: "attempts",
      header: "Attempts",
      render: (task) => `${task.attempts}/${task.max_attempts}`,
    },
    {
      key: "duration",
      header: "Runtime",
      className: "whitespace-nowrap",
      render: (task) =>
        formatTaskInterval(
          task.started_at,
          task.completed_at ??
            (["processing", "running"].includes(task.status)
              ? new Date().toISOString()
              : task.updated_at),
        ),
    },
    ...(["created_at", "started_at", "completed_at"] as const).map(
      (key, index): Column<AdminTask> => ({
        key,
        header: ["Queued", "Started", "Finished"][index],
        className: "whitespace-nowrap",
        render: (task) => formatTaskDate(task[key]) || "—",
      }),
    ),
    {
      key: "error",
      header: "Error",
      className: "max-w-xs truncate",
      render: (task) => (
        <span title={task.error ?? undefined}>{task.error || "—"}</span>
      ),
    },
  ];

  return (
    <Modal
      title={
        <>
          <span className="break-all">{taskType}</span>
          <InfoTooltip
            label={`${taskType} description`}
            tip={processorDescription(taskType)}
            icon="help"
            position="bottom"
          />
        </>
      }
      onClose={onClose}
      className="max-w-6xl"
    >
      <h4 className="mt-4 font-semibold">Last 10 runs</h4>
      <p className="mb-4 text-sm text-base-content/70">
        Newest queued tasks first, across all statuses and dates. Select a task
        to inspect its attempts, payload, and recorded errors.
      </p>
      <DataGrid
        rows={query.data ?? []}
        columns={columns}
        isLoading={query.isLoading}
        error={query.error ? "Unable to load processor runs." : undefined}
        emptyMessage="No runs have been recorded for this processor."
        label={`${taskType} recent runs`}
        onRowClick={(task) => onSelectTask(task.id)}
      />
    </Modal>
  );
}
