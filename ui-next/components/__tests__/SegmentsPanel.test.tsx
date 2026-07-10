import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import SegmentsPanel from "../SegmentsPanel";

const mocks = vi.hoisted(() => ({
  useCurrentUser: vi.fn(),
  useSegments: vi.fn(),
  useUpdateSegment: vi.fn(),
  useUploadSegment: vi.fn(),
  uploadAsync: vi.fn(),
  updateSegmentAsync: vi.fn(),
  toastError: vi.fn(),
  toastSuccess: vi.fn(),
}));

vi.mock("@ericbutera/kaleido", async () => {
  const actual = await vi.importActual<typeof import("@ericbutera/kaleido")>(
    "@ericbutera/kaleido",
  );

  return {
    ...actual,
    auth: {
      useAuthApi: () => ({
        useCurrentUser: mocks.useCurrentUser,
      }),
    },
  };
});

vi.mock("../../lib/queries", () => ({
  useSegments: mocks.useSegments,
  useUpdateSegment: mocks.useUpdateSegment,
  useUploadSegment: mocks.useUploadSegment,
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

function makeSegment(
  overrides: Partial<{
    id: number;
    title: string;
    source: string;
    mode: "xc" | "dh";
    original_filename: string | null;
    format: string | null;
    distance_meters: number | null;
    effort_count: number;
    best_duration_seconds: number | null;
    current_user_pr_duration_seconds: number | null;
    created_at: string;
    starred: boolean;
  }> = {},
) {
  return {
    id: 9,
    title: "North Climb",
    source: "manual_upload",
    mode: "xc",
    original_filename: "north-climb.gpx",
    format: "gpx",
    distance_meters: 1800,
    effort_count: 3,
    best_duration_seconds: 312,
    current_user_pr_duration_seconds: 320,
    created_at: "2026-05-07T07:00:00Z",
    starred: false,
    ...overrides,
  };
}

describe("SegmentsPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();

    mocks.useCurrentUser.mockReturnValue({
      user: { id: 1, email: "rider@example.com" },
      isLoading: false,
    });
    mocks.useSegments.mockReturnValue({
      data: [],
      isError: false,
      isFetching: false,
      error: null,
    });
    mocks.useUploadSegment.mockReturnValue({
      uploadAsync: mocks.uploadAsync,
      isPending: false,
    });
    mocks.useUpdateSegment.mockReturnValue({
      updateAsync: mocks.updateSegmentAsync,
      isPending: false,
      isError: false,
      error: null,
    });
    mocks.uploadAsync.mockResolvedValue(makeSegment());
  });

  it("renders sign-in actions when the user is signed out", () => {
    mocks.useCurrentUser.mockReturnValue({ user: null, isLoading: false });

    render(<SegmentsPanel />);

    expect(screen.getByText("Manual Segment Imports")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Sign in" })).toHaveAttribute(
      "href",
      "/login",
    );
    expect(
      screen.getByRole("link", { name: "Create account" }),
    ).toHaveAttribute("href", "/signup");
  });

  it("uploads a selected segment route", async () => {
    const user = userEvent.setup();

    render(<SegmentsPanel />);

    const input = screen.getByLabelText(
      "Segment route file",
    ) as HTMLInputElement;
    const file = new File(["segment"], "north-climb.gpx", {
      type: "application/gpx+xml",
    });

    await user.upload(input, file);
    await user.click(screen.getByRole("button", { name: "Import segment" }));

    await waitFor(() => {
      expect(mocks.uploadAsync).toHaveBeenCalledWith(file);
    });

    expect(mocks.toastSuccess).toHaveBeenCalledWith(
      "Imported North Climb. Segment matching queued.",
    );
    expect(input.value).toBe("");
  });

  it("renders recent imported segments", () => {
    mocks.useSegments.mockReturnValue({
      data: [
        makeSegment({
          id: 13,
          title: "FMR Lower",
          mode: "dh",
          best_duration_seconds: 280,
          current_user_pr_duration_seconds: 295,
        }),
        makeSegment({ id: 12, title: "River Sprint", effort_count: 2 }),
      ],
      isError: false,
      isLoading: false,
      isFetching: false,
      error: null,
    });

    render(<SegmentsPanel />);

    expect(screen.getByRole("link", { name: "River Sprint" })).toHaveAttribute(
      "href",
      "/segments/12",
    );
    expect(screen.getByRole("link", { name: "FMR Lower" })).toHaveAttribute(
      "href",
      "/segments/13",
    );
    expect(screen.getByText("Comparison-ready routes")).toBeInTheDocument();
    expect(screen.getByText("Type")).toBeInTheDocument();
    expect(screen.getByText("Efforts")).toBeInTheDocument();
    expect(screen.getByText("2")).toBeInTheDocument();
    expect(screen.getByText("KOM")).toBeInTheDocument();
    expect(screen.getByText("Your PR")).toBeInTheDocument();
    expect(screen.getByText("xc")).toBeInTheDocument();
    expect(screen.getByText("dh")).toBeInTheDocument();
    const segmentLinks = screen
      .getAllByRole("link")
      .filter((link) =>
        ["FMR Lower", "River Sprint"].includes(link.textContent ?? ""),
      );
    expect(segmentLinks.map((link) => link.textContent)).toEqual([
      "FMR Lower",
      "River Sprint",
    ]);
    expect(
      screen.queryByRole("link", { name: "Compare efforts" }),
    ).not.toBeInTheDocument();
  });

  it("filters the segments grid by mode", async () => {
    const user = userEvent.setup();

    mocks.useSegments.mockReturnValue({
      data: [
        makeSegment({ id: 12, title: "River Sprint", effort_count: 2 }),
        makeSegment({
          id: 13,
          title: "FMR Lower",
          mode: "dh",
          best_duration_seconds: 280,
          current_user_pr_duration_seconds: 295,
        }),
      ],
      isError: false,
      isLoading: false,
      isFetching: false,
      error: null,
    });

    render(<SegmentsPanel />);

    await user.selectOptions(screen.getByRole("combobox"), "dh");
    await user.click(screen.getByRole("button", { name: "Search" }));

    expect(
      screen.queryByRole("link", { name: "River Sprint" }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: "FMR Lower" })).toBeInTheDocument();
  });

  it("toggles a segment star from the library grid", async () => {
    const user = userEvent.setup();

    mocks.useSegments.mockReturnValue({
      data: [makeSegment({ id: 12, title: "River Sprint", starred: false })],
      isError: false,
      isLoading: false,
      isFetching: false,
      error: null,
    });

    render(<SegmentsPanel />);

    await user.click(screen.getByRole("button", { name: "Star River Sprint" }));

    expect(mocks.updateSegmentAsync).toHaveBeenCalledWith({
      id: 12,
      starred: true,
    });
  });
});
