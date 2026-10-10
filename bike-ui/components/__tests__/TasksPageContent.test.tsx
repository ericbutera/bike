import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import TasksPageContent from "../admin/TasksPageContent";

const mocks = vi.hoisted(() => ({
  useAdminTask: vi.fn(),
  useAdminTaskCancel: vi.fn(),
  useAdminTaskRerun: vi.fn(),
  useAdminTasks: vi.fn(),
  useWorkerProcessors: vi.fn(),
  cancelAsync: vi.fn(),
  rerunAsync: vi.fn(),
  refetchTasks: vi.fn(),
  refetchTask: vi.fn(),
}));

vi.mock("@/lib/queries", () => ({
  useAdminTask: mocks.useAdminTask,
  useAdminTaskCancel: mocks.useAdminTaskCancel,
  useAdminTaskRerun: mocks.useAdminTaskRerun,
  useAdminTasks: mocks.useAdminTasks,
  useWorkerProcessors: mocks.useWorkerProcessors,
}));

describe("TasksPageContent", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.useWorkerProcessors.mockReturnValue({ data: [], error: null });
    mocks.useAdminTasks.mockReturnValue({
      data: [
        {
          id: 42,
          task_type: "rebuild_fitness_freshness",
          status: "completed",
          attempts: 1,
          max_attempts: 3,
          error: null,
          scheduled_for: null,
          started_at: "2026-01-02T12:00:00Z",
          completed_at: "2026-01-02T12:00:01Z",
          created_at: "2026-01-02T11:59:00Z",
          updated_at: "2026-01-02T12:00:01Z",
        },
      ],
      metadata: { total: 1, per_page: 20 },
      isLoading: false,
      error: null,
      refetch: mocks.refetchTasks,
    });
    mocks.useAdminTask.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: null,
      refetch: mocks.refetchTask,
    });
    mocks.useAdminTaskCancel.mockReturnValue({
      isPending: false,
      mutateAsync: mocks.cancelAsync,
    });
    mocks.useAdminTaskRerun.mockReturnValue({
      isPending: false,
      mutateAsync: mocks.rerunAsync,
    });
    mocks.rerunAsync.mockResolvedValue({ id: 43, status: "pending" });
    mocks.refetchTasks.mockResolvedValue(undefined);
  });

  it("allows rerunning a completed task into a fresh queue entry", async () => {
    const user = userEvent.setup();
    render(<TasksPageContent />);

    await user.click(screen.getByLabelText("Task 42 actions"));
    const rerun = screen.getByRole("button", { name: /rerun/i });
    expect(rerun).toBeEnabled();
    await user.click(rerun);

    await waitFor(() => {
      expect(mocks.rerunAsync).toHaveBeenCalledWith({
        params: { path: { id: 42 } },
      });
    });
    expect(mocks.refetchTasks).toHaveBeenCalled();
  });

  it("searches task lineage by the correlation ID from an error log", async () => {
    const user = userEvent.setup();
    render(<TasksPageContent />);
    await user.type(
      screen.getByLabelText("Request, trace, or pipeline ID"),
      "request-123",
    );
    expect(mocks.useAdminTasks).toHaveBeenLastCalledWith(
      expect.objectContaining({ correlationId: "request-123" }),
    );
  });

  it("filters registered and historical processors and retains the selection on an empty page", () => {
    const distribution = { samples: 0, p50_seconds: null, p90_seconds: null };
    mocks.useWorkerProcessors.mockReturnValue({
      data: [
        {
          task_type: "receive_strava_delivery",
          queued: 0,
          scheduled: 0,
          running: 0,
          retrying: 0,
          failed: 0,
          attempt: distribution,
          eligible_wait: distribution,
          logical_completion: distribution,
        },
      ],
    });
    const view = render(<TasksPageContent />);
    expect(
      screen.getByRole("table", { name: "Background task history" }),
    ).toBeVisible();
    const filter = screen.getByLabelText("Filter task type");
    expect(filter).toHaveTextContent("receive strava delivery");
    expect(filter).toHaveTextContent("rebuild fitness freshness");
    fireEvent.change(filter, {
      target: { value: "rebuild_fitness_freshness" },
    });
    expect(mocks.useAdminTasks).toHaveBeenLastCalledWith(
      expect.objectContaining({ taskType: "rebuild_fitness_freshness" }),
    );
    mocks.useAdminTasks.mockReturnValue({
      data: [],
      metadata: { total: 0, per_page: 20 },
    });
    view.rerender(<TasksPageContent />);
    expect(filter).toHaveValue("rebuild_fitness_freshness");
    expect(screen.getByText("No tasks found matching criteria.")).toBeVisible();
  });
});
