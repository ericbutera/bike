import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import ImportDetail from "../imports/ImportDetail";

const mocks = vi.hoisted(() => ({ detail: vi.fn() }));
vi.mock("../../lib/queries", () => ({ useAdminActivityImport: mocks.detail }));
describe("import detail", () => {
  it("shows the source summary and links to its saved activity", () => {
    mocks.detail.mockReturnValue({
      data: {
        original_filename: "ride.gpx",
        status: "processed",
        source: "manual_upload",
        size_bytes: 4096,
        activity_id: 42,
      },
      isLoading: false,
      error: null,
    });
    render(<ImportDetail importId="17" />);
    expect(mocks.detail).toHaveBeenCalledWith("17");
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
    expect(screen.getByText("manual upload")).toBeInTheDocument();
    expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
  });

  it("keeps the import identifiable before its summary is loaded", () => {
    mocks.detail.mockReturnValue({
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

  it("shows failure details without stage replay controls", () => {
    mocks.detail.mockReturnValue({
      data: {
        id: 17,
        status: "failed",
        source: "archive",
        size_bytes: 4096,
        processing_error: "Invalid source",
      },
      isLoading: false,
      error: null,
    });
    render(<ImportDetail importId="17" />);
    expect(screen.getByText("No activity created")).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent("Invalid source");
    expect(
      screen.queryByRole("button", { name: /Replay/ }),
    ).not.toBeInTheDocument();
  });

  it("refreshes the selected import only when requested", () => {
    const refetch = vi.fn();
    mocks.detail.mockReturnValue({
      data: { id: 17, status: "processing", size_bytes: 4096 },
      isLoading: false,
      isFetching: false,
      error: null,
      refetch,
    });
    const { rerender } = render(<ImportDetail importId="17" />);
    expect(refetch).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Refresh" }));
    expect(refetch).toHaveBeenCalledOnce();
    mocks.detail.mockReturnValue({
      data: { id: 17, status: "processing", size_bytes: 4096 },
      isLoading: false,
      isFetching: true,
      error: null,
      refetch,
    });
    rerender(<ImportDetail importId="17" />);
    expect(screen.getByRole("button", { name: "Refresh" })).toBeDisabled();
  });
});
