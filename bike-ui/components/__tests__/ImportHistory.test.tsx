import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ImportHistory from "../imports/ImportHistory";

const mocks = vi.hoisted(() => ({ history: vi.fn() }));
vi.mock("../../lib/queries", () => ({
  useActivityImportHistory: mocks.history,
}));

beforeEach(() => {
  mocks.history.mockReset().mockReturnValue({
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
  it("opens failed imports before they have an activity", () => {
    render(<ImportHistory />);
    expect(screen.getByRole("link", { name: "broken.gpx" })).toHaveAttribute(
      "href",
      "/admin/imports/17",
    );
    expect(screen.getByText("No activity yet")).toBeInTheDocument();
    expect(screen.getByText("Invalid GPX")).toBeInTheDocument();
  });

  it("paginates and resets to the first page when filters change", async () => {
    const user = userEvent.setup();
    render(<ImportHistory />);
    await user.click(screen.getByRole("button", { name: "Next page" }));
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
