import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import SegmentsPanel from "../SegmentsPanel";

const mocks = vi.hoisted(() => ({
  useCurrentUser: vi.fn(),
  useSegments: vi.fn(),
  useUploadSegment: vi.fn(),
  uploadAsync: vi.fn(),
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
  useSegments: mocks.useSegments,
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
    original_filename: string | null;
    format: string | null;
    distance_meters: number | null;
    effort_count: number;
    best_duration_seconds: number | null;
    current_user_pr_duration_seconds: number | null;
    created_at: string;
  }> = {},
) {
  return {
    id: 9,
    title: "North Climb",
    source: "manual_upload",
    original_filename: "north-climb.gpx",
    format: "gpx",
    distance_meters: 1800,
    effort_count: 3,
    best_duration_seconds: 312,
    current_user_pr_duration_seconds: 320,
    created_at: "2026-05-07T07:00:00Z",
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
      "Imported North Climb and matched 3 efforts.",
    );
    expect(input.value).toBe("");
  });

  it("renders recent imported segments", () => {
    mocks.useSegments.mockReturnValue({
      data: [makeSegment({ id: 12, title: "River Sprint", effort_count: 2 })],
      isError: false,
      isFetching: false,
      error: null,
    });

    render(<SegmentsPanel />);

    expect(screen.getByRole("link", { name: "River Sprint" })).toHaveAttribute(
      "href",
      "/segments/12",
    );
    expect(screen.getByText("Efforts")).toBeInTheDocument();
    expect(screen.getByText("2")).toBeInTheDocument();
    expect(screen.getByText("KOM")).toBeInTheDocument();
    expect(screen.getByText("Your PR")).toBeInTheDocument();
    expect(screen.queryByText("north-climb.gpx")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("link", { name: "Compare efforts" }),
    ).not.toBeInTheDocument();
  });
});
