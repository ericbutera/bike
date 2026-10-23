import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ActivityStream from "../ActivityStream";
import type { Activity } from "../../lib/queries";
import { activityEarnedAward } from "../../tests/fixtures/activities";
import { activityAchievements } from "../activity-stream/activityAchievements";
import RequireAuth from "../RequireAuth";
import { formatActivityTimestamp } from "../../lib/activityFormatting";

const mocks = vi.hoisted(() => ({
  useCurrentUser: vi.fn(),
  useFeatureFlag: vi.fn(),
  useActivities: vi.fn(),
  routerReplace: vi.fn(),
  searchParams: "",
}));

vi.mock("../../lib/auth", () => ({
  useAuth: () => ({
    user: mocks.useCurrentUser().user,
    isLoading: mocks.useCurrentUser().isLoading,
  }),
}));

vi.mock("../../lib/featureFlags", async () => {
  const actual = await vi.importActual<typeof import("../../lib/featureFlags")>(
    "../../lib/featureFlags",
  );
  return { ...actual, useFeatureFlag: mocks.useFeatureFlag };
});

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

function makeActivity(overrides: Partial<Activity> = {}): Activity {
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
    earned_awards: [],
    segment_efforts: [],
    ...overrides,
  };
}

describe("ActivityStream", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.searchParams = "";
    mocks.routerReplace.mockImplementation((url: string) => {
      mocks.searchParams = new URL(
        url,
        "http://localhost",
      ).searchParams.toString();
    });
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
      "/activity-previews/thumbnail/7?activityId=2",
    );
    const latestEffortArticle = screen
      .getByRole("heading", { level: 3, name: "Latest Effort" })
      .closest("article");
    expect(latestEffortArticle).not.toBeNull();
    const latestEffortLocations = within(
      latestEffortArticle as HTMLElement,
    ).getAllByText("Portland, OR");
    expect(latestEffortLocations).toHaveLength(1);
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
    ).toHaveLength(1);
    expect(screen.queryByText("morning-ride.gpx")).not.toBeInTheDocument();
    expect(screen.queryByText("earlier-effort.gpx")).not.toBeInTheDocument();
    expect(screen.queryByText("Mountain biking")).not.toBeInTheDocument();
    expect(screen.queryByText(/^gpx$/i)).not.toBeInTheDocument();
    expect(
      screen.queryByRole("link", { name: "View details" }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "1" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(screen.getByRole("button", { name: "2" })).toBeInTheDocument();
  });

  it("aligns location and date in one metadata row and places the sport before the title", () => {
    const activity = makeActivity({ sport: "mountain_bike" });
    mocks.useActivities.mockReturnValue({
      data: [activity],
      isError: false,
      isFetching: false,
    });

    render(<ActivityStream />);

    const timestamp = screen.getByText(
      formatActivityTimestamp(activity.started_at),
    );
    const location = screen.getByText("Portland, OR");
    const metadata = timestamp.parentElement;
    expect(location.parentElement).toBe(metadata);
    expect(metadata).toHaveClass(
      "flex",
      "items-baseline",
      "text-sm",
      "text-base-content/60",
    );
    expect(timestamp).toHaveAttribute("datetime", activity.started_at);
    const heading = screen.getByRole("heading", {
      name: activity.title,
      level: 3,
    });
    const icon = screen.getByRole("img", { name: "Mountain bike" });
    expect(icon.parentElement?.nextElementSibling).toBe(heading);
    expect(heading.parentElement).toContainElement(icon);
    expect(metadata).not.toContainElement(icon);
  });

  it("resets pagination and preserves other URL parameters when selecting or clearing a sport", async () => {
    mocks.searchParams = "sport=walk&page=3&q=forest";
    const user = userEvent.setup();
    render(<ActivityStream />);

    expect(mocks.useActivities).toHaveBeenLastCalledWith({
      page: 3,
      perPage: 10,
      sport: "walk",
    });
    await user.click(
      screen.getByRole("button", { name: "Activity type: Walk" }),
    );
    await user.click(screen.getByRole("menuitemradio", { name: "Hike" }));
    expect(mocks.routerReplace).toHaveBeenLastCalledWith(
      "/?sport=hike&q=forest",
      { scroll: false },
    );
    expect(mocks.useActivities).toHaveBeenLastCalledWith({
      page: 1,
      perPage: 10,
      sport: "hike",
    });

    await user.click(
      screen.getByRole("button", { name: "Activity type: Hike" }),
    );
    await user.click(
      screen.getByRole("menuitemradio", { name: "All activity types" }),
    );
    expect(mocks.routerReplace).toHaveBeenLastCalledWith("/?q=forest", {
      scroll: false,
    });
    expect(mocks.useActivities).toHaveBeenLastCalledWith({
      page: 1,
      perPage: 10,
    });
  });

  it("does not derive awards from current ranks when saved awards are empty", () => {
    const activity = makeActivity({
      segment_efforts: [
        {
          segment_id: 11,
          segment_title: "North Climb",
          effort_index: 1,
          duration_seconds: 60,
          start_route_point_index: 0,
          end_route_point_index: 1,
          overall_rank: 1,
          personal_rank: 1,
        },
      ],
    });
    expect(activityAchievements(activity)).toEqual([]);
  });

  it("preserves the saved award order without sorting by rank or title", () => {
    const base = activityEarnedAward({
      activity_id: 7,
      position: 2,
      display_position: 1,
      segment_id: 11,
      segment_title: "Zulu Climb",
      overall_rank: null,
    });
    const achievements = activityAchievements(
      makeActivity({
        earned_awards: [
          base,
          {
            ...base,
            position: 1,
            display_position: 2,
            segment_id: 12,
            segment_title: "Alpha Climb",
            overall_rank: 1,
          },
        ],
      }),
    );
    expect(
      achievements.map(({ label, segmentTitle }) => [label, segmentTitle]),
    ).toEqual([
      ["PR", "Zulu Climb"],
      ["KOM", "Alpha Climb"],
    ]);
  });

  it("shows segment achievements on the activity list", () => {
    mocks.useActivities.mockReturnValue({
      data: [
        makeActivity({
          id: 2,
          title: "Latest Effort",
          earned_awards: [
            activityEarnedAward({
              activity_id: 2,
              position: 1,
              display_position: 1,
              segment_id: 11,
              segment_title: "North Climb",
              effort_index: 1,
              overall_rank: 1,
              personal_rank: 1,
              personal_best_duration_seconds: 312,
            }),
            activityEarnedAward({
              activity_id: 2,
              position: 2,
              display_position: 2,
              segment_id: 12,
              segment_title: "Bridge Sprint",
              effort_index: 1,
              overall_rank: 7,
              personal_rank: 1,
              personal_best_duration_seconds: 48,
            }),
            activityEarnedAward({
              activity_id: 2,
              position: 3,
              display_position: 3,
              segment_id: 13,
              segment_title: "Park Loop",
              effort_index: 1,
              overall_rank: null,
              personal_rank: 2,
              personal_best_duration_seconds: 134,
            }),
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
      "/activity-map-images/full/1?activityId=2",
    );
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

    expect(screen.getByRole("button", { name: "3" })).toHaveAttribute(
      "aria-current",
      "page",
    );
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
          total: 65,
          total_pages: 7,
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

  it("jumps from page five to page one and restores activity data and the URL", async () => {
    const user = userEvent.setup();
    mocks.useActivities.mockImplementation(({ page }: { page: number }) => ({
      data: [makeActivity({ id: page, title: `Page ${page}` })],
      metadata: { page, per_page: 10, total: 101, total_pages: 11 },
      isError: false,
      isFetching: false,
      error: null,
    }));
    render(<ActivityStream />);

    await user.click(screen.getByRole("button", { name: "5" }));
    expect(mocks.routerReplace).toHaveBeenLastCalledWith("/?page=5", {
      scroll: false,
    });
    expect(
      screen.getByRole("heading", { level: 3, name: "Page 5" }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "First page" }));
    expect(mocks.routerReplace).toHaveBeenLastCalledWith("/", {
      scroll: false,
    });
    expect(mocks.useActivities).toHaveBeenLastCalledWith({
      page: 1,
      perPage: 10,
    });
    expect(
      screen.getByRole("heading", { level: 3, name: "Page 1" }),
    ).toBeInTheDocument();
  });

  it("renders page two and restores page one without a page URL parameter", async () => {
    const user = userEvent.setup();
    mocks.useActivities.mockImplementation(({ page }: { page: number }) => ({
      data: [
        makeActivity({
          id: page * 100 + 1,
          title: `Parity page ${page} ride 1`,
        }),
      ],
      metadata: { page, per_page: 10, total: 20, total_pages: 2 },
      isError: false,
      isFetching: false,
      error: null,
    }));

    render(<ActivityStream />);

    expect(screen.getAllByRole("heading", { level: 3 })[0]).toHaveTextContent(
      "Parity page 1 ride 1",
    );

    await user.click(screen.getByRole("button", { name: "2" }));

    expect(mocks.routerReplace).toHaveBeenLastCalledWith("/?page=2", {
      scroll: false,
    });
    expect(mocks.searchParams).toBe("page=2");
    expect(mocks.useActivities).toHaveBeenLastCalledWith({
      page: 2,
      perPage: 10,
    });
    expect(screen.getAllByRole("heading", { level: 3 })[0]).toHaveTextContent(
      "Parity page 2 ride 1",
    );

    await user.click(screen.getByRole("button", { name: "1" }));

    expect(mocks.routerReplace).toHaveBeenLastCalledWith("/", {
      scroll: false,
    });
    expect(mocks.searchParams).toBe("");
    expect(mocks.useActivities).toHaveBeenLastCalledWith({
      page: 1,
      perPage: 10,
    });
    expect(screen.getAllByRole("heading", { level: 3 })[0]).toHaveTextContent(
      "Parity page 1 ride 1",
    );
  });
});
