import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, it, expect, vi } from "vitest";
import ProcessorOverview from "../admin/ProcessorOverview";
const processors = vi.hoisted(() => vi.fn());
const tasks = vi.hoisted(() => vi.fn());
vi.mock("../../lib/queries", () => ({
  useWorkerProcessors: processors,
  useAdminTasks: tasks,
}));
describe("ProcessorOverview", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    tasks.mockReturnValue({ data: [], isLoading: false, error: null });
  });
  it("shows inactive registered processors as N/A and uses the selected population", async () => {
    const empty = { samples: 0, p50_seconds: null, p90_seconds: null };
    processors.mockReturnValue({
      data: [
        {
          task_type: "prepare_heatmap",
          queued: 2,
          scheduled: 1,
          running: 1,
          retrying: 0,
          failed: 0,
          attempt: empty,
          eligible_wait: empty,
          logical_completion: empty,
        },
      ],
      error: null,
    });
    render(<ProcessorOverview onSelectTask={vi.fn()} />);
    expect(screen.getByText("prepare_heatmap")).toBeInTheDocument();
    expect(screen.getAllByText("N/A / N/A · 0")).toHaveLength(3);
    const user = userEvent.setup();
    await user.selectOptions(screen.getByLabelText("Processor window"), "168");
    await user.selectOptions(
      screen.getByLabelText("Attempt outcome"),
      "failed",
    );
    expect(processors).toHaveBeenLastCalledWith(168, "failed");
    expect(tasks).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "prepare_heatmap" }));
    const modal = screen.getByRole("dialog");
    expect(within(modal).getByText("Last 10 runs")).toBeVisible();
    expect(tasks).toHaveBeenLastCalledWith({
      taskType: "prepare_heatmap",
      page: 1,
      perPage: 10,
    });
    await user.click(within(modal).getByRole("button", { name: "Close" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByLabelText("Processor window")).toHaveValue("168");
  });
  it("keeps load failures visible", () => {
    processors.mockReturnValue({
      data: undefined,
      error: new Error("unavailable"),
    });
    render(<ProcessorOverview onSelectTask={vi.fn()} />);
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Unable to load processor statistics",
    );
  });
  it("opens task details from a processor run and closes the history modal", async () => {
    const empty = { samples: 0, p50_seconds: null, p90_seconds: null };
    processors.mockReturnValue({
      data: [
        {
          task_type: "prepare_heatmap",
          queued: 0,
          scheduled: 0,
          running: 0,
          retrying: 0,
          failed: 0,
          attempt: empty,
          eligible_wait: empty,
          logical_completion: empty,
        },
      ],
    });
    tasks.mockReturnValue({
      data: [{ id: 42, status: "pending", attempts: 0, max_attempts: 3 }],
    });
    const select = vi.fn();
    const user = userEvent.setup();
    render(<ProcessorOverview onSelectTask={select} />);
    await user.click(screen.getByText("prepare_heatmap"));
    await user.click(screen.getByRole("button", { name: "#42" }));
    expect(select).toHaveBeenCalledExactlyOnceWith(42);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});
