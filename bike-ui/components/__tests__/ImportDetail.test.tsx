import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import ImportDetail from "../imports/ImportDetail";

const mocks = vi.hoisted(() => ({ trace: vi.fn() }));
vi.mock("../../lib/queries", () => ({ useActivityImportTrace: mocks.trace }));
vi.mock("../activity-detail/ActivityImportTracePanel", () => ({
  default: ({ canReplay }: { canReplay: boolean }) => (
    <p>{canReplay ? "Replay available" : "Read only"}</p>
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
    expect(
      screen.getByText(/processed · manual_upload · 4,096 bytes/),
    ).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "View activity" })).toHaveAttribute(
      "href",
      "/activities/42",
    );
    expect(
      screen.getByRole("link", { name: /Import history/ }),
    ).toHaveAttribute("href", "/imports");
    expect(screen.getByText("Replay available")).toBeInTheDocument();
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
});
