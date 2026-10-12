import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ProcessorHistoryModal from "../admin/ProcessorHistoryModal";
import { formatTaskDate } from "../../lib/taskTiming";

const query = vi.hoisted(() => vi.fn());
vi.mock("../../lib/queries", () => ({ useAdminTasks: query }));

function renderHistory(onClose = vi.fn(), onSelectTask = vi.fn()) {
  return render(
    <ProcessorHistoryModal
      taskType="prepare_heatmap"
      onClose={onClose}
      onSelectTask={onSelectTask}
    />,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  query.mockReturnValue({ data: [], isLoading: false, error: null });
});

describe("ProcessorHistoryModal", () => {
  it("shows failed and queued runs with timing, attempts, errors, and task selection", async () => {
    query.mockReturnValue({
      data: [
        {
          id: 42,
          status: "failed",
          attempts: 3,
          max_attempts: 3,
          created_at: "2026-10-10T12:00:00Z",
          started_at: "2026-10-10T12:01:00Z",
          updated_at: "2026-10-10T12:02:30Z",
          error: "Route projection failed",
        },
        { id: 41, status: "pending", attempts: 0, max_attempts: 3 },
      ],
    });
    const select = vi.fn();
    renderHistory(vi.fn(), select);
    const table = screen.getByRole("table", {
      name: "prepare_heatmap recent runs",
    });
    expect(within(table).getByText("failed")).toBeVisible();
    expect(within(table).getByText("3/3")).toBeVisible();
    expect(within(table).getByText("1m 30s")).toBeVisible();
    expect(
      within(table).getByText(formatTaskDate("2026-10-10T12:01:00Z")),
    ).toBeVisible();
    expect(within(table).getByText("Route projection failed")).toHaveAttribute(
      "title",
      "Route projection failed",
    );
    expect(within(table).getByText("N/A")).toBeVisible();
    await userEvent.setup().click(screen.getByRole("button", { name: "#42" }));
    expect(select).toHaveBeenCalledExactlyOnceWith(42);
  });

  it("exposes the processor description on hover and focus inside the dialog", async () => {
    renderHistory();
    const help = screen.getByLabelText("prepare_heatmap description");
    fireEvent.pointerEnter(help);
    expect(screen.getByRole("tooltip")).toHaveTextContent("personal heatmap");
    expect(screen.getByRole("dialog")).toContainElement(
      screen.getByRole("tooltip"),
    );
    fireEvent.pointerLeave(help);
    expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
    fireEvent.focus(help);
    expect(screen.getByRole("tooltip")).toHaveTextContent("current projection");
    fireEvent.blur(help);
    expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
  });

  it("uses elapsed time for running tasks and the recorded finish for completed tasks", () => {
    vi.useFakeTimers();
    vi.setSystemTime("2026-10-10T12:03:00Z");
    try {
      query.mockReturnValue({
        data: [
          {
            id: 42,
            status: "running",
            attempts: 1,
            max_attempts: 3,
            started_at: "2026-10-10T12:01:00Z",
          },
          {
            id: 41,
            status: "completed",
            attempts: 1,
            max_attempts: 3,
            started_at: "2026-10-10T12:01:00Z",
            completed_at: "2026-10-10T12:02:00Z",
          },
        ],
      });
      const select = vi.fn();
      renderHistory(vi.fn(), select);
      expect(screen.getByText("2m 0s")).toBeVisible();
      expect(screen.getByText("1m 0s")).toBeVisible();
      fireEvent.click(screen.getByText("running"));
      expect(select).toHaveBeenCalledExactlyOnceWith(42);
    } finally {
      vi.useRealTimers();
    }
  });

  it("shows loading, load errors, and empty history explicitly", () => {
    query.mockReturnValue({ isLoading: true });
    const view = renderHistory();
    expect(screen.getByText("Loading...")).toBeVisible();
    query.mockReturnValue({ error: new Error("Unavailable") });
    view.rerender(
      <ProcessorHistoryModal
        taskType="prepare_heatmap"
        onClose={vi.fn()}
        onSelectTask={vi.fn()}
      />,
    );
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Unable to load processor runs.",
    );
    query.mockReturnValue({ data: [] });
    view.rerender(
      <ProcessorHistoryModal
        taskType="prepare_heatmap"
        onClose={vi.fn()}
        onSelectTask={vi.fn()}
      />,
    );
    expect(
      screen.getByText("No runs have been recorded for this processor."),
    ).toBeVisible();
  });

  it("closes on Escape, Close, and the backdrop", async () => {
    const close = vi.fn();
    renderHistory(close);
    fireEvent(
      screen.getByRole("dialog"),
      new Event("cancel", { cancelable: true }),
    );
    await userEvent
      .setup()
      .click(screen.getByRole("button", { name: "Close" }));
    await userEvent
      .setup()
      .click(screen.getByRole("button", { name: "Close dialog" }));
    expect(close).toHaveBeenCalledTimes(3);
  });
});
