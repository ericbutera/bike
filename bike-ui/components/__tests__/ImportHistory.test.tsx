import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ImportHistory from "../imports/ImportHistory";

const mocks = vi.hoisted(() => ({ history: vi.fn(), refetch: vi.fn() }));
vi.mock("../../lib/queries", () => ({
  useActivityImportHistory: mocks.history,
}));

beforeEach(() => {
  mocks.refetch.mockReset();
  mocks.history.mockReset().mockReturnValue({
    refetch: mocks.refetch,
    isFetching: false,
    error: null,
    data: {
      page: 1,
      per_page: 25,
      total: 27,
      items: [
        {
          id: 17,
          source: "archive_url_import",
          original_filename: "broken.gpx",
          format: "gpx",
          status: "failed",
          processing_stage: "activity_parsed",
          processing_error: "Invalid GPX",
          activity_id: null,
          size_bytes: 12,
          created_at: "2026-10-08T12:00:00Z",
        },
      ],
    },
  });
});

describe("Import history", () => {
  it("refreshes the current list only on request", async () => {
    const user = userEvent.setup();
    render(<ImportHistory />);
    expect(mocks.refetch).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Refresh" }));
    expect(mocks.refetch).toHaveBeenCalledOnce();
  });
  it("opens failed imports before they have an activity", () => {
    render(<ImportHistory />);
    expect(screen.getByRole("link", { name: "broken.gpx" })).toHaveAttribute(
      "href",
      "/admin/imports/17",
    );
    expect(screen.getByText("No activity yet")).toBeInTheDocument();
    expect(screen.getByText("Invalid GPX")).toBeInTheDocument();
    expect(screen.getByText("Invalid GPX")).toHaveAttribute(
      "title",
      "Invalid GPX",
    );
    expect(screen.getByRole("columnheader", { name: "Outcome" })).toBeVisible();
    expect(screen.getByRole("columnheader", { name: "Stage" })).toBeVisible();
    const source = screen.getByRole("combobox", { name: "Source" });
    const table = screen.getByRole("table", { name: "Import history" });
    expect(source.closest(".card")).toBe(table.closest(".card"));
    expect(
      source.compareDocumentPosition(table) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it("paginates and resets to the first page when filters change", async () => {
    const user = userEvent.setup();
    render(<ImportHistory />);
    await user.click(screen.getByRole("button", { name: "2" }));
    expect(mocks.history).toHaveBeenLastCalledWith(
      {
        page: 2,
        source: undefined,
        status: undefined,
        archive_job_id: undefined,
        user_id: undefined,
      },
      { enabled: true },
    );
    await user.selectOptions(
      screen.getByRole("combobox", { name: "Source" }),
      "strava_sync",
    );
    await user.selectOptions(
      screen.getByRole("combobox", { name: "Outcome" }),
      "failed",
    );
    expect(mocks.history).toHaveBeenLastCalledWith(
      {
        page: 1,
        source: "strava_sync",
        status: "failed",
        archive_job_id: undefined,
        user_id: undefined,
      },
      { enabled: true },
    );
  });

  it("filters imports belonging to an archive job", () => {
    render(<ImportHistory archiveJobId={9} />);
    expect(mocks.history).toHaveBeenCalledWith(
      {
        page: 1,
        source: undefined,
        status: undefined,
        archive_job_id: 9,
        user_id: undefined,
      },
      { enabled: true },
    );
    expect(
      screen.getByRole("link", { name: "Show all imports" }),
    ).toHaveAttribute("href", "/admin/imports");
  });

  it("refreshes the selected filters and clears them back to page one", async () => {
    const user = userEvent.setup();
    render(<ImportHistory />);
    await user.type(screen.getByRole("spinbutton", { name: "User ID" }), "7");
    await user.selectOptions(
      screen.getByRole("combobox", { name: "Source" }),
      "archive_url_import",
    );
    await user.selectOptions(
      screen.getByRole("combobox", { name: "Outcome" }),
      "failed",
    );
    await user.click(screen.getByRole("button", { name: "2" }));
    await user.click(screen.getByRole("button", { name: "Search" }));
    expect(mocks.refetch).toHaveBeenCalledOnce();
    expect(mocks.history).toHaveBeenLastCalledWith(
      expect.objectContaining({
        page: 2,
        user_id: 7,
        source: "archive_url_import",
        status: "failed",
      }),
      { enabled: true },
    );
    await user.click(screen.getByRole("button", { name: "Clear" }));
    expect(screen.getByRole("spinbutton", { name: "User ID" })).toHaveValue(
      null,
    );
    expect(screen.getByRole("combobox", { name: "Source" })).toHaveValue("");
    expect(screen.getByRole("combobox", { name: "Outcome" })).toHaveValue("");
    expect(mocks.history).toHaveBeenLastCalledWith(
      {
        page: 1,
        user_id: undefined,
        source: undefined,
        status: undefined,
        archive_job_id: undefined,
      },
      { enabled: true },
    );
  });

  it("selects another owner and blocks invalid owner IDs", async () => {
    const user = userEvent.setup();
    render(<ImportHistory />);
    const input = screen.getByRole("spinbutton", { name: "User ID" });
    await user.type(input, "7");
    expect(mocks.history).toHaveBeenLastCalledWith(
      expect.objectContaining({ user_id: 7, page: 1 }),
      { enabled: true },
    );
    await user.clear(input);
    await user.type(input, "0");
    expect(mocks.history).toHaveBeenLastCalledWith(
      expect.objectContaining({ user_id: 0 }),
      { enabled: false },
    );
    expect(screen.getByText("User ID must be positive")).toBeInTheDocument();
  });
});
