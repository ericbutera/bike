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
      "/imports/17",
    );
    expect(screen.getByText("No activity yet")).toBeInTheDocument();
    expect(screen.getByText("Invalid GPX")).toBeInTheDocument();
  });

  it("paginates and resets to the first page when filters change", async () => {
    const user = userEvent.setup();
    render(<ImportHistory />);
    await user.click(screen.getByRole("button", { name: "Next page" }));
    expect(mocks.history).toHaveBeenLastCalledWith({
      page: 2,
      source: undefined,
      status: undefined,
      archive_job_id: undefined,
    });
    await user.selectOptions(
      screen.getByRole("combobox", { name: "Source" }),
      "strava_sync",
    );
    await user.selectOptions(
      screen.getByRole("combobox", { name: "Outcome" }),
      "failed",
    );
    expect(mocks.history).toHaveBeenLastCalledWith({
      page: 1,
      source: "strava_sync",
      status: "failed",
      archive_job_id: undefined,
    });
  });

  it("filters imports belonging to an archive job", () => {
    render(<ImportHistory archiveJobId={9} />);
    expect(mocks.history).toHaveBeenCalledWith({
      page: 1,
      source: undefined,
      status: undefined,
      archive_job_id: 9,
    });
    expect(
      screen.getByRole("link", { name: "Show all imports" }),
    ).toHaveAttribute("href", "/imports");
  });
});
