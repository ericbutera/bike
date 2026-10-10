import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi } from "vitest";
import ProcessorOverview from "../admin/ProcessorOverview";
const processors = vi.hoisted(() => vi.fn());
vi.mock("../../lib/queries", () => ({ useWorkerProcessors: processors }));
describe("ProcessorOverview", () => {
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
    render(<ProcessorOverview />);
    expect(screen.getByText("prepare_heatmap")).toBeInTheDocument();
    expect(screen.getAllByText("N/A / N/A · 0")).toHaveLength(3);
    const user = userEvent.setup();
    await user.selectOptions(screen.getByLabelText("Processor window"), "168");
    await user.selectOptions(
      screen.getByLabelText("Attempt outcome"),
      "failed",
    );
    expect(processors).toHaveBeenLastCalledWith(168, "failed");
  });
  it("keeps load failures visible", () => {
    processors.mockReturnValue({
      data: undefined,
      error: new Error("unavailable"),
    });
    render(<ProcessorOverview />);
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Unable to load processor statistics",
    );
  });
});
