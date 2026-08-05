import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ActivityStream from "../ActivityStream";
import RequireAuth from "../RequireAuth";

const mocks = vi.hoisted(() => ({
  useCurrentUser: vi.fn(),
  useFeatureFlag: vi.fn(),
  useActivities: vi.fn(),
  routerReplace: vi.fn(),
  searchParams: "",
}));

vi.mock("@ericbutera/kaleido", () => ({
  auth: {
    useAuthApi: () => ({
      useCurrentUser: mocks.useCurrentUser,
    }),
  },
  featureFlags: {
    useFeatureFlag: mocks.useFeatureFlag,
  },
  LoadingCard: () => <div aria-label="Loading" />,
  LoadingSpinner: (props: any) => <span aria-hidden="true" {...props} />,
  Pagination: ({ page, perPage, total, onPageChange }: any) => (
    <div>
      <span>{`pagination:${page}:${perPage}:${total}`}</span>
      <button type="button" onClick={() => onPageChange(page + 1)}>
        Next page
      </button>
    </div>
  ),
}));

vi.mock("../../lib/queries", () => ({
  useActivities: mocks.useActivities,
}));

vi.mock("next/navigation", () => ({
  usePathname: () => "/",
  useRouter: () => ({
    replace: mocks.routerReplace,
  }),
  useSearchParams: () => new URLSearchParams(mocks.searchParams),
}));

vi.mock("next/link", () => ({
  default: ({ href, children, ...props }: any) => (
    <a href={href} {...props}>
      {children}
    </a>
  ),
}));

function makeActivity(
  overrides: Partial<{
    id: number;
    title: string;
    sport: string;
    source: string;
    original_filename: string | null;
    format: string | null;
    started_at: string;
    location: string | null;
    route_points: Array<{
      elapsed_seconds: number;
      latitude: number;
      longitude: number;
    }>;
    distance_meters: number | null;
    moving_time_seconds: number | null;
    total_time_seconds: number | null;
    elevation_gain_meters: number | null;
    max_heart_rate_bpm: number | null;
    achievement_highlights: Array<{
      segment_id: number;
      segment_title: string;
      effort_index: number;
      overall_rank?: number | null;
      personal_rank?: number | null;
      personal_best_duration_seconds?: number | null;
    }>;
    segment_efforts: Array<{
      segment_id: number;
      segment_title: string;
      effort_index: number;
      duration_seconds: number;
      start_route_point_index: number;
      end_route_point_index: number;
      overall_rank?: number | null;
      personal_rank?: number | null;
      personal_best_duration_seconds?: number | null;
    }>;
  }> = {},
) {
  return {
    id: 7,
    title: "Morning Ride",
    sport: "ride",
    source: "manual_upload",
    original_filename: "morning-ride.gpx",
    format: "gpx",
    started_at: "2026-05-06T14:00:00Z",
    location: "Portland, OR",
    route_points: [
      { elapsed_seconds: 0, latitude: 45.0, longitude: -122.0 },
      { elapsed_seconds: 1200, latitude: 45.015, longitude: -121.985 },
      { elapsed_seconds: 2400, latitude: 45.03, longitude: -121.97 },
    ],
    ended_at: "2026-05-06T15:05:00Z",
    distance_meters: 40200,
    moving_time_seconds: 3600,
    total_time_seconds: 3900,
    elevation_gain_meters: 520,
    elevation_loss_meters: 515,
    average_speed_mps: 9.5,
    max_speed_mps: 16.2,
    average_heart_rate_bpm: 142,
    max_heart_rate_bpm: 171,
    average_cadence_rpm: 86,
    max_cadence_rpm: 104,
    calories: 860,
    achievement_highlights: [],
    segment_efforts: [],
    ...overrides,
  };
}

describe("ActivityStream", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.searchParams = "";
    mocks.useFeatureFlag.mockReturnValue(false);
    mocks.useCurrentUser.mockReturnValue({
      user: { id: 1, email: "rider@example.com" },
      isLoading: false,
    });
    mocks.useActivities.mockReturnValue({
      data: [],
      metadata: { page: 1, per_page: 10, total: 0, total_pages: 1 },
      isError: false,
      isFetching: false,
      error: null,
    });
  });

  it("renders activities in the order returned by the query", () => {
    mocks.useActivities.mockReturnValue({
      data: [
        makeActivity({ id: 2, title: "Latest Effort" }),
        makeActivity({
          id: 1,
          title: "Earlier Effort",
          original_filename: "earlier-effort.gpx",
          sport: "mountain_biking",
          location: "Traverse City, MI",
        }),
      ],
      metadata: { page: 1, per_page: 10, total: 12, total_pages: 2 },
      isError: false,
      isFetching: false,
      error: null,
    });

    render(<ActivityStream />);

    const headings = screen.getAllByRole("heading", { level: 3 });
    expect(headings[0]).toHaveTextContent("Latest Effort");
    expect(headings[1]).toHaveTextContent("Earlier Effort");
    expect(screen.getByRole("link", { name: "Latest Effort" })).toHaveAttribute(
      "href",
      "/activities/2",
    );
    expect(
      screen.getByRole("img", { name: "Route thumbnail for Latest Effort" }),
    ).toBeInTheDocument();
    const routeThumbnail = screen.getByRole("img", {
      name: "Route thumbnail for Latest Effort",
    });
    expect(routeThumbnail).toHaveAttribute("src");
    expect(routeThumbnail.getAttribute("src")).toContain(
      "/activity-previews/thumbnail/6?",
    );
    expect(routeThumbnail.getAttribute("src")).toContain("activityId=2");
    expect(routeThumbnail.getAttribute("src")).not.toContain("points=");
    expect(routeThumbnail.getAttribute("src")).not.toContain("variant=");
    expect(routeThumbnail.getAttribute("src")).not.toContain("v=");
    const latestEffortArticle = screen
      .getByRole("heading", { level: 3, name: "Latest Effort" })
      .closest("article");
    expect(latestEffortArticle).not.toBeNull();
    const latestEffortLocations = within(
      latestEffortArticle as HTMLElement,
    ).getAllByText("Portland, OR");
    expect(latestEffortLocations).toHaveLength(2);
    expect(latestEffortLocations[0]).toHaveClass("hidden");
    expect(latestEffortLocations[1]).toHaveClass("sm:hidden");
    expect(
      (latestEffortArticle?.firstElementChild as HTMLElement).className,
    ).toContain("sm:grid-cols-[8.5rem_minmax(0,1fr)]");
    const earlierEffortArticle = screen
      .getByRole("heading", { level: 3, name: "Earlier Effort" })
      .closest("article");
    expect(earlierEffortArticle).not.toBeNull();
    expect(
      within(earlierEffortArticle as HTMLElement).getAllByText(
        "Traverse City, MI",
      ),
    ).toHaveLength(2);
    expect(screen.queryByText("morning-ride.gpx")).not.toBeInTheDocument();
    expect(screen.queryByText("earlier-effort.gpx")).not.toBeInTheDocument();
    expect(screen.queryByText("Mountain biking")).not.toBeInTheDocument();
    expect(screen.queryByText(/^gpx$/i)).not.toBeInTheDocument();
    expect(
      screen.queryByRole("link", { name: "View details" }),
    ).not.toBeInTheDocument();
    expect(screen.getByText("pagination:1:10:12")).toBeInTheDocument();
  });

  it("shows segment achievements on the activity list", () => {
    mocks.useActivities.mockReturnValue({
      data: [
        makeActivity({
          id: 2,
          title: "Latest Effort",
          achievement_highlights: [
            {
              segment_id: 11,
              segment_title: "North Climb",
              effort_index: 1,
              overall_rank: 1,
              personal_rank: 1,
              personal_best_duration_seconds: 312,
            },
            {
              segment_id: 12,
              segment_title: "Bridge Sprint",
              effort_index: 1,
              overall_rank: 7,
              personal_rank: 1,
              personal_best_duration_seconds: 48,
            },
            {
              segment_id: 13,
              segment_title: "Park Loop",
              effort_index: 1,
              personal_rank: 2,
              personal_best_duration_seconds: 134,
            },
          ],
        }),
      ],
      metadata: { page: 1, per_page: 10, total: 1, total_pages: 1 },
      isError: false,
      isFetching: false,
      error: null,
    });

    render(<ActivityStream />);

    expect(screen.getByText("KOM")).toBeInTheDocument();
    expect(screen.getByText("North Climb")).toBeInTheDocument();
    expect(screen.getByText("Top 7")).toBeInTheDocument();
    expect(screen.getByText("Bridge Sprint")).toBeInTheDocument();
    expect(screen.getByText("2nd best")).toBeInTheDocument();
    expect(screen.getByText("Park Loop")).toBeInTheDocument();
  });

  it("renders a full-width static route preview when the feature flag is enabled", () => {
    mocks.useFeatureFlag.mockReturnValue(true);
    mocks.useActivities.mockReturnValue({
      data: [makeActivity({ id: 2, title: "Latest Effort" })],
      metadata: { page: 1, per_page: 10, total: 1, total_pages: 1 },
      isError: false,
      isFetching: false,
      error: null,
    });

    render(<ActivityStream />);

    expect(
      screen.getByRole("img", { name: "Route map for Latest Effort" }),
    ).toBeInTheDocument();
    const fullPreview = screen.getByRole("img", {
      name: "Route map for Latest Effort",
    });
    expect(fullPreview.getAttribute("src")).toContain(
      "/activity-previews/full/6?",
    );
    expect(fullPreview.getAttribute("src")).toContain("activityId=2");
    expect(fullPreview.getAttribute("src")).not.toContain("points=");
    expect(fullPreview.getAttribute("src")).not.toContain("variant=");
    expect(fullPreview.getAttribute("src")).not.toContain("v=");
    expect(
      screen.queryByRole("img", {
        name: "Route thumbnail for Latest Effort",
      }),
    ).not.toBeInTheDocument();

    const article = screen
      .getByRole("heading", { level: 3, name: "Latest Effort" })
      .closest("article");

    expect(article).not.toBeNull();
    expect((article?.firstElementChild as HTMLElement).className).toBe(
      "grid gap-3",
    );
  });

  it("initializes the current page from the url", () => {
    mocks.searchParams = "page=3";
    mocks.useActivities.mockReturnValue({
      data: [makeActivity({ id: 3, title: "Page 3" })],
      metadata: { page: 3, per_page: 10, total: 25, total_pages: 3 },
      isError: false,
      isFetching: false,
      error: null,
    });

    render(<ActivityStream />);

    expect(screen.getByText("pagination:3:10:25")).toBeInTheDocument();
    expect(
      screen.getByRole("heading", { level: 3, name: "Page 3" }),
    ).toBeInTheDocument();
  });

  it("requests the next page when pagination changes", async () => {
    const user = userEvent.setup();
    const requestedPages: number[] = [];

    mocks.useActivities.mockImplementation(({ page }: { page?: number }) => {
      const currentPage = page ?? 1;
      requestedPages.push(currentPage);

      return {
        data: [makeActivity({ id: currentPage, title: `Page ${currentPage}` })],
        metadata: {
          page: currentPage,
          per_page: 10,
          total: 25,
          total_pages: 3,
        },
        isError: false,
        isFetching: false,
        error: null,
      };
    });

    render(<ActivityStream />);

    await user.click(screen.getByRole("button", { name: "Next page" }));

    expect(requestedPages).toContain(1);
    expect(requestedPages).toContain(2);
    expect(mocks.routerReplace).toHaveBeenCalledWith("/?page=2", {
      scroll: false,
    });
    expect(
      screen.getByRole("heading", { level: 3, name: "Page 2" }),
    ).toBeInTheDocument();
  });
});
