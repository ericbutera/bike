import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import TasksPageContent from "../admin/TasksPageContent";

const mocks = vi.hoisted(() => ({
  useAdminTask: vi.fn(),
  useAdminTaskCancel: vi.fn(),
  useAdminTaskRerun: vi.fn(),
  useAdminTasks: vi.fn(),
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
}));

describe("TasksPageContent", () => {
  beforeEach(() => {
    vi.clearAllMocks();
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
});
