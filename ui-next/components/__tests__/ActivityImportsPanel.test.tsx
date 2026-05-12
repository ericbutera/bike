import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ActivityImportsPanel from "../ActivityImportsPanel";

const mocks = vi.hoisted(() => ({
  useCurrentUser: vi.fn(),
  useActivityArchiveImportJobs: vi.fn(),
  useActivityImports: vi.fn(),
  useImportActivityArchiveUrl: vi.fn(),
  useUploadActivityImport: vi.fn(),
  importArchiveAsync: vi.fn(),
  uploadAsync: vi.fn(),
  routerPush: vi.fn(),
  toastError: vi.fn(),
  toastSuccess: vi.fn(),
}));

vi.mock("@ericbutera/kaleido", () => ({
  auth: {
    useAuthApi: () => ({
      useCurrentUser: mocks.useCurrentUser,
    }),
  },
}));

vi.mock("../../lib/queries", () => ({
  useActivityArchiveImportJobs: mocks.useActivityArchiveImportJobs,
  useActivityImports: mocks.useActivityImports,
  useImportActivityArchiveUrl: mocks.useImportActivityArchiveUrl,
  useUploadActivityImport: mocks.useUploadActivityImport,
}));

vi.mock("next/navigation", () => ({
  useRouter: () => ({
    push: mocks.routerPush,
  }),
}));

vi.mock("react-hot-toast", () => ({
  default: {
    error: mocks.toastError,
    success: mocks.toastSuccess,
  },
}));

vi.mock("next/link", () => ({
  default: ({ href, children, ...props }: any) => (
    <a href={href} {...props}>
      {children}
    </a>
  ),
}));

function makeActivityImport(
  overrides: Partial<{
    id: number;
    activity_id: number | null;
    original_filename: string;
    format: string;
    status: string;
    size_bytes: number;
    mime_type: string | null;
    created_at: string;
    activity_started_at: string | null;
    activity_duration_seconds: number | null;
    activity_location: string | null;
  }> = {},
) {
  return {
    id: 1,
    activity_id: 7,
    original_filename: "ride.gpx",
    format: "gpx",
    status: "uploaded",
    size_bytes: 4096,
    mime_type: "application/gpx+xml",
    created_at: "2026-05-06T12:00:00Z",
    activity_started_at: "2026-05-06T12:00:00Z",
    activity_duration_seconds: 3600,
    activity_location: "Portland, OR",
    ...overrides,
  };
}

function makeArchiveImportJob(
  overrides: Partial<{
    id: number;
    archive_url: string;
    resolved_url: string | null;
    status: string;
    failure_message: string | null;
    total_entries: number;
    supported_entry_count: number;
    imported_count: number;
    duplicate_count: number;
    skipped_unsupported_count: number;
    failed_count: number;
    error_samples: string[];
    created_at: string;
    started_at: string | null;
    finished_at: string | null;
    updated_at: string;
  }> = {},
) {
  return {
    id: 51,
    archive_url: "https://downloads.example.com/export.zip",
    resolved_url: null,
    status: "queued",
    failure_message: null,
    total_entries: 12,
    supported_entry_count: 10,
    imported_count: 8,
    duplicate_count: 2,
    skipped_unsupported_count: 2,
    failed_count: 0,
    error_samples: [],
    created_at: "2026-05-06T12:00:00Z",
    started_at: null,
    finished_at: null,
    updated_at: "2026-05-06T12:00:00Z",
    ...overrides,
  };
}

describe("ActivityImportsPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();

    mocks.useCurrentUser.mockReturnValue({
      user: { id: 1, email: "rider@example.com" },
      isLoading: false,
    });
    mocks.useActivityImports.mockReturnValue({
      data: [],
      isError: false,
      isFetching: false,
      error: null,
    });
    mocks.useActivityArchiveImportJobs.mockReturnValue({
      data: [],
      isError: false,
      isFetching: false,
      error: null,
    });
    mocks.useUploadActivityImport.mockReturnValue({
      uploadAsync: mocks.uploadAsync,
      isPending: false,
    });
    mocks.useImportActivityArchiveUrl.mockReturnValue({
      importAsync: mocks.importArchiveAsync,
      isPending: false,
    });
    mocks.uploadAsync.mockResolvedValue(makeActivityImport());
    mocks.importArchiveAsync.mockResolvedValue(makeArchiveImportJob());
  });

  it("renders sign-in actions when the user is signed out", () => {
    mocks.useCurrentUser.mockReturnValue({ user: null, isLoading: false });

    render(<ActivityImportsPanel />);

    expect(screen.getByText("Activity Imports")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Sign in" })).toHaveAttribute(
      "href",
      "/login",
    );
    expect(
      screen.getByRole("link", { name: "Create account" }),
    ).toHaveAttribute("href", "/signup");
  });

  it("shows the selected file details before upload", async () => {
    const user = userEvent.setup();

    render(<ActivityImportsPanel />);

    const input = screen.getByLabelText("Activity file");
    const file = new File(["1234"], "morning-ride.GPX", {
      type: "application/gpx+xml",
    });

    await user.upload(input, file);

    expect(screen.getByText("morning-ride.GPX")).toBeInTheDocument();
    expect(screen.getByText("4 B")).toBeInTheDocument();
    expect(screen.getByText("gpx")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Upload activity" }),
    ).toBeEnabled();
  });

  it("rejects unsupported file extensions before calling the upload mutation", async () => {
    const user = userEvent.setup({ applyAccept: false });

    render(<ActivityImportsPanel />);

    const input = screen.getByLabelText("Activity file");
    const file = new File(["csv"], "ride.csv", { type: "text/csv" });

    await user.upload(input, file);
    await user.click(screen.getByRole("button", { name: "Upload activity" }));

    expect(mocks.toastError).toHaveBeenCalledWith(
      "Only .fit, .tcx, and .gpx files are supported. Rejected ride.csv.",
    );
    expect(mocks.uploadAsync).not.toHaveBeenCalled();
  });

  it("uploads a selected activity and resets the chooser", async () => {
    const user = userEvent.setup();

    render(<ActivityImportsPanel />);

    const input = screen.getByLabelText("Activity file") as HTMLInputElement;
    const file = new File(["gpx-data"], "ride.gpx", {
      type: "application/gpx+xml",
    });

    await user.upload(input, file);
    await user.click(screen.getByRole("button", { name: "Upload activity" }));

    await waitFor(() => {
      expect(mocks.uploadAsync).toHaveBeenCalledWith(file);
    });

    expect(mocks.toastSuccess).toHaveBeenCalledWith(
      "Imported ride.gpx into your activity feed.",
    );
    expect(screen.queryByText("ride.gpx")).not.toBeInTheDocument();
    expect(
      screen.getByText(
        "Choose one or more `.fit`, `.tcx`, or `.gpx` files to seed the system.",
      ),
    ).toBeInTheDocument();
    expect(input.value).toBe("");
  });

  it("surfaces duplicate single uploads without creating a second activity", async () => {
    const user = userEvent.setup();

    mocks.uploadAsync.mockResolvedValue(
      makeActivityImport({ status: "duplicate" }),
    );

    render(<ActivityImportsPanel />);

    const input = screen.getByLabelText("Activity file") as HTMLInputElement;
    const file = new File(["gpx-data"], "ride.gpx", {
      type: "application/gpx+xml",
    });

    await user.upload(input, file);
    await user.click(screen.getByRole("button", { name: "Upload activity" }));

    await waitFor(() => {
      expect(mocks.uploadAsync).toHaveBeenCalledWith(file);
    });

    expect(mocks.toastSuccess).toHaveBeenCalledWith(
      "Already had ride.gpx in your activity feed.",
    );
  });

  it("uploads multiple selected activities in sequence", async () => {
    const user = userEvent.setup();

    render(<ActivityImportsPanel />);

    const input = screen.getByLabelText("Activity file") as HTMLInputElement;
    const files = [
      new File(["gpx-data"], "ride-1.gpx", { type: "application/gpx+xml" }),
      new File(["tcx-data"], "ride-2.tcx", { type: "application/xml" }),
    ];

    mocks.uploadAsync
      .mockResolvedValueOnce(
        makeActivityImport({ original_filename: "ride-1.gpx" }),
      )
      .mockResolvedValueOnce(
        makeActivityImport({
          id: 2,
          original_filename: "ride-2.tcx",
          format: "tcx",
        }),
      );

    await user.upload(input, files);

    expect(screen.getByText("2 files selected")).toBeInTheDocument();
    expect(screen.getByText("ride-1.gpx")).toBeInTheDocument();
    expect(screen.getByText("ride-2.tcx")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Upload activities" }));

    await waitFor(() => {
      expect(mocks.uploadAsync).toHaveBeenNthCalledWith(1, files[0]);
      expect(mocks.uploadAsync).toHaveBeenNthCalledWith(2, files[1]);
    });

    expect(mocks.toastSuccess).toHaveBeenCalledWith(
      "Imported 2 activities into your activity feed.",
    );
    expect(
      screen.getByText(
        "Choose one or more `.fit`, `.tcx`, or `.gpx` files to seed the system.",
      ),
    ).toBeInTheDocument();
    expect(input.value).toBe("");
  });

  it("fetches an archive import by URL", async () => {
    const user = userEvent.setup();

    render(<ActivityImportsPanel />);

    await user.type(
      screen.getByLabelText("Archive URL"),
      "https://downloads.example.com/export.zip",
    );
    await user.click(
      screen.getByRole("button", { name: "Queue archive import" }),
    );

    await waitFor(() => {
      expect(mocks.importArchiveAsync).toHaveBeenCalledWith(
        "https://downloads.example.com/export.zip",
      );
    });

    expect(mocks.toastSuccess).toHaveBeenCalledWith(
      "Queued archive import. Bike will fetch and process it in the background.",
    );
    expect(
      screen.getByText("Worker-backed status updates"),
    ).toBeInTheDocument();
  });

  it("renders recent archive import job status", () => {
    mocks.useActivityArchiveImportJobs.mockReturnValue({
      data: [
        makeArchiveImportJob({
          status: "running",
          resolved_url: "https://cdn.example.com/export.zip",
        }),
      ],
      isError: false,
      isFetching: false,
      error: null,
    });

    render(<ActivityImportsPanel />);

    expect(screen.getByText("running")).toBeInTheDocument();
    expect(
      screen.getByText("https://cdn.example.com/export.zip"),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Worker-backed status updates"),
    ).toBeInTheDocument();
  });

  it("renders recent uploads from the query response", () => {
    mocks.useActivityImports.mockReturnValue({
      data: [
        makeActivityImport({
          id: 42,
          original_filename: "tempo.fit",
          format: "fit",
          activity_location: "Traverse City, MI",
        }),
      ],
      isError: false,
      isFetching: false,
      error: null,
    });

    render(<ActivityImportsPanel />);

    expect(screen.getByText("Traverse City, MI")).toBeInTheDocument();
    expect(screen.getByText("1h 00m")).toBeInTheDocument();
    expect(
      screen
        .getAllByRole("link")
        .some((link) => link.getAttribute("href") === "/activities/7"),
    ).toBe(true);
    expect(
      screen.queryByRole("button", { name: "Regenerate" }),
    ).not.toBeInTheDocument();
  });

  it("navigates to activity details when a recent upload row is clicked", async () => {
    const user = userEvent.setup();
    mocks.useActivityImports.mockReturnValue({
      data: [makeActivityImport({ id: 42, activity_id: 11 })],
      isError: false,
      isFetching: false,
      error: null,
    });

    render(<ActivityImportsPanel />);

    await user.click(screen.getByText("Portland, OR"));

    expect(mocks.routerPush).toHaveBeenCalledWith("/activities/11");
  });

  it("keeps uploads without activities non-clickable", async () => {
    const user = userEvent.setup();
    mocks.useActivityImports.mockReturnValue({
      data: [makeActivityImport({ id: 42, activity_id: null })],
      isError: false,
      isFetching: false,
      error: null,
    });

    render(<ActivityImportsPanel />);

    await user.click(screen.getByText("Portland, OR"));

    expect(mocks.routerPush).not.toHaveBeenCalled();
  });
});
