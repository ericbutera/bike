import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import ActivityPipelines from "../admin/ActivityPipelines";
const { useActivityPipelines } = vi.hoisted(() => ({
  useActivityPipelines: vi.fn(),
}));
vi.mock("../../lib/queries", () => ({ useActivityPipelines }));
vi.mock("../admin/PipelinePanel", () => ({
  default: ({ runId }: { runId: string }) => <p>Selected run {runId}</p>,
}));
describe("ActivityPipelines", () => {
  it("follows related runs and advances the run cursor without keeping a stale selection", () => {
    useActivityPipelines.mockReturnValue({
      data: { run_ids: ["first", "second"], next_cursor: "second" },
      error: null,
    });
    render(<ActivityPipelines activityId={7} />);
    expect(screen.getByText("Selected run first")).toBeInTheDocument();
    fireEvent.change(
      screen.getByRole("combobox", { name: "Activity pipeline" }),
      { target: { value: "second" } },
    );
    expect(screen.getByText("Selected run second")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "More runs" }));
    expect(useActivityPipelines).toHaveBeenLastCalledWith(7, "second");
    expect(screen.getByText("Selected run first")).toBeInTheDocument();
  });
  it("keeps missing historical lineage unknown and reports a failed lookup", () => {
    useActivityPipelines.mockReturnValue({
      data: { run_ids: [] },
      error: new Error("unavailable"),
    });
    render(<ActivityPipelines activityId={7} />);
    expect(screen.getByRole("alert")).toHaveTextContent("Unable to load");
    expect(screen.getByText(/Historical work/)).toBeInTheDocument();
  });
});
