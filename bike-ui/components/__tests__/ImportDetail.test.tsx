import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import ImportDetail from "../imports/ImportDetail";

const mocks = vi.hoisted(() => ({ trace: vi.fn() }));
vi.mock("../../lib/queries", () => ({ useActivityImportTrace: mocks.trace }));
vi.mock("../activity-detail/ActivityImportTracePanel", () => ({
  default: ({ canReplay }: { canReplay: boolean }) => (
    <p>{canReplay ? "Replay available" : "Read only"}</p>
  ),
}));
vi.mock("../admin/ActivityPipelines", () => ({
  default: ({ activityId }: { activityId: number }) => (
    <p>Worker runs for activity #{activityId}</p>
  ),
}));

describe("import detail", () => {
  it("shows the source summary and links to its saved activity", () => {
    mocks.trace.mockReturnValue({
      data: {
        import: {
          original_filename: "ride.gpx",
          status: "processed",
          source: "manual_upload",
          size_bytes: 4096,
          activity_id: 42,
        },
      },
      isLoading: false,
      error: null,
    });
    render(<ImportDetail importId="17" />);
    expect(mocks.trace).toHaveBeenCalledWith("17");
    expect(
      screen.getByRole("heading", { name: "ride.gpx" }),
    ).toBeInTheDocument();
    expect(screen.getByText("4,096 bytes")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "View activity" })).toHaveAttribute(
      "href",
      "/activities/42",
    );
    expect(
      screen.getByRole("link", { name: /Import history/ }),
    ).toHaveAttribute("href", "/admin/imports");
    expect(screen.getByText("Replay available")).toBeInTheDocument();
    expect(
      screen.getByText("Worker runs for activity #42"),
    ).toBeInTheDocument();
    expect(screen.getByText("manual upload")).toBeInTheDocument();
    expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
  });

  it("keeps the import identifiable before its summary is loaded", () => {
    mocks.trace.mockReturnValue({
      data: undefined,
      isLoading: true,
      error: null,
    });
    render(<ImportDetail importId="17" />);
    expect(
      screen.getByRole("heading", { name: "Import #17" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("link", { name: "View activity" }),
    ).not.toBeInTheDocument();
  });

  it("retains stage recovery before activity creation", () => {
    mocks.trace.mockReturnValue({
      data: { import: { id: 17, status: "failed", source: "archive" } },
      isLoading: false,
      error: null,
    });
    render(<ImportDetail importId="17" />);
    expect(screen.getByText("Replay available")).toBeInTheDocument();
    expect(screen.getByText("No activity created")).toBeInTheDocument();
    expect(screen.getByText(/Worker runs will appear/)).toBeInTheDocument();
    expect(
      screen.queryByText(/Worker runs for activity/),
    ).not.toBeInTheDocument();
  });

  it("refreshes the selected import only when requested", () => {
    const refetch = vi.fn();
    mocks.trace.mockReturnValue({
      data: { import: { id: 17, status: "processing" } },
      isLoading: false,
      isFetching: false,
      error: null,
      refetch,
    });
    const { rerender } = render(<ImportDetail importId="17" />);
    expect(refetch).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Refresh" }));
    expect(refetch).toHaveBeenCalledOnce();
    mocks.trace.mockReturnValue({
      data: { import: { id: 17, status: "processing" } },
      isLoading: false,
      isFetching: true,
      error: null,
      refetch,
    });
    rerender(<ImportDetail importId="17" />);
    expect(screen.getByRole("button", { name: "Refresh" })).toBeDisabled();
  });
});
