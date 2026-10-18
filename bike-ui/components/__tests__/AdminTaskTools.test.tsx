import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AdminTaskTools from "../admin/AdminTaskTools";

const mocks = vi.hoisted(() => ({
  useActivityArchiveImportJobs: vi.fn(),
  useAdminBackfillAnalytics: vi.fn(),
  useCleanupUserDuplicateActivities: vi.fn(),
  useImportActivityArchiveUrl: vi.fn(),
  useRegenerateSegmentEfforts: vi.fn(),
  backfillAnalyticsAsync: vi.fn(),
  cleanupAsync: vi.fn(),
  importArchiveAsync: vi.fn(),
  regenerateSegmentEffortsAsync: vi.fn(),
}));

vi.mock("@/lib/queries", () => ({
  useActivityArchiveImportJobs: mocks.useActivityArchiveImportJobs,
  useAdminBackfillAnalytics: mocks.useAdminBackfillAnalytics,
  useCleanupUserDuplicateActivities: mocks.useCleanupUserDuplicateActivities,
  useImportActivityArchiveUrl: mocks.useImportActivityArchiveUrl,
  useRegenerateSegmentEfforts: mocks.useRegenerateSegmentEfforts,
}));

vi.mock("next/link", () => ({
  default: ({ href, children, ...props }: any) => (
    <a href={href} {...props}>
      {children}
    </a>
  ),
}));

describe("AdminTaskTools", () => {
  beforeEach(() => {
    vi.clearAllMocks();

    mocks.useActivityArchiveImportJobs.mockReturnValue({
      data: [],
    });
    mocks.useAdminBackfillAnalytics.mockReturnValue({
      backfillAsync: mocks.backfillAnalyticsAsync,
      isPending: false,
    });
    mocks.useCleanupUserDuplicateActivities.mockReturnValue({
      cleanupAsync: mocks.cleanupAsync,
      isPending: false,
    });
    mocks.useImportActivityArchiveUrl.mockReturnValue({
      importAsync: mocks.importArchiveAsync,
      isPending: false,
    });
    mocks.useRegenerateSegmentEfforts.mockReturnValue({
      regenerateAsync: mocks.regenerateSegmentEffortsAsync,
      isPending: false,
    });
  });

  it("queues targeted segment effort regeneration", async () => {
    mocks.regenerateSegmentEffortsAsync.mockResolvedValue({
      segment_id: 51,
      status: "queued",
      message: "Segment effort regeneration queued.",
    });

    const user = userEvent.setup();
    render(<AdminTaskTools />);

    await user.type(screen.getByLabelText(/segment id/i), "51");
    await user.click(
      screen.getByRole("button", { name: /regenerate segment efforts/i }),
    );

    expect(mocks.regenerateSegmentEffortsAsync).toHaveBeenCalledWith(51);

    await waitFor(() => {
      expect(screen.getByText("Segment effort request")).toBeInTheDocument();
    });
    expect(screen.getByText("queued")).toBeInTheDocument();
    expect(
      screen.getByText("Segment effort regeneration queued."),
    ).toBeInTheDocument();
  });

  it("directs activity and user commands to their row menus", () => {
    render(<AdminTaskTools />);
    expect(screen.getByRole("link", { name: "Activities" })).toHaveAttribute(
      "href",
      "/admin/activities",
    );
    expect(screen.getByRole("link", { name: "Users" })).toHaveAttribute(
      "href",
      "/admin/users",
    );
    expect(screen.queryByLabelText(/activity id/i)).not.toBeInTheDocument();
    expect(
      screen.getByRole("heading", { name: "Fetch provider export archives" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("heading", { name: "Clean up duplicate activities" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("heading", { name: "Prewarm analytics caches" }),
    ).toBeInTheDocument();
  });
});
