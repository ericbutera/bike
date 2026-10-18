import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AdminActivitiesPage from "../../app/admin/activities/page";
import AdminIntegrationsPage from "../../app/admin/integrations/page";
import Nav from "../admin/Nav";
import AdminUsersPageContent from "../admin/AdminUsersPageContent";
import { adminUser } from "@/tests/fixtures/adminTasks";

const mocks = vi.hoisted(() => ({
  activities: vi.fn(),
  events: vi.fn(),
  users: vi.fn(),
  replace: vi.fn(),
  push: vi.fn(),
  refetch: vi.fn(),
  search: "",
  pathname: "/admin/activities",
}));
vi.mock("next/navigation", () => ({
  useSearchParams: () => new URLSearchParams(mocks.search),
  usePathname: () => mocks.pathname,
  useRouter: () => ({ replace: mocks.replace, push: mocks.push }),
}));
vi.mock("../../lib/queries", () => ({
  useAdminActivities: mocks.activities,
  useAdminIntegrationEvents: mocks.events,
  useAdminUsers: mocks.users,
  useAdminUser: vi.fn(),
  useAdminUserAuditEvents: vi.fn(),
  useUpdateAdminUser: () => ({ isPending: false }),
  useDisableAdminUser: () => ({ isPending: false }),
}));
vi.mock("../admin/LocalAdminLayout", () => ({
  LocalAdminLayout: ({ children }: { children: ReactNode }) => children,
}));

beforeEach(() => {
  vi.clearAllMocks();
  mocks.search = "";
  mocks.pathname = "/admin/activities";
  mocks.activities.mockReturnValue({
    data: [
      {
        id: 42,
        user_id: 7,
        title: "A long hill ride title",
        sport: "road_ride",
        source: "manual_upload",
        format: "fit",
        activity_import_id: 17,
        import_status: "processed",
        import_processing_stage: "complete",
        started_at: "2026-10-10T12:00:00Z",
      },
    ],
    metadata: { page: 1, per_page: 25, total: 30 },
    isLoading: false,
    error: null,
  });
  mocks.events.mockReturnValue({
    data: [
      {
        id: 1,
        user_id: 7,
        provider: "strava",
        event_type: "sync.completed",
        level: "success",
        message: "Imported ride",
        payload: {},
        created_at: "2026-10-10T12:00:00Z",
      },
    ],
    isLoading: false,
    error: null,
    refetch: mocks.refetch,
  });
});

describe("admin list grids", () => {
  it("keeps the Users status reference as filled pills with semantic colors", () => {
    mocks.users.mockReturnValue({
      data: [
        adminUser(),
        adminUser({ id: 8, disabled: true, email_verified: false }),
      ],
    });
    render(<AdminUsersPageContent />);
    const table = within(screen.getByRole("table", { name: "Users" }));
    expect(table.getByText("Active")).toHaveClass("badge", "badge-success");
    expect(
      table.getByRole("cell", { name: "Verified" }).firstElementChild,
    ).toHaveClass("badge-success");
    expect(table.getByText("Disabled")).toHaveClass("badge-error");
    expect(table.getByText("Unverified")).toHaveClass("badge-warning");
    expect(table.getByText("Active")).not.toHaveClass("badge-outline");
    expect(
      table
        .getAllByRole("columnheader")
        .slice(-2)
        .map((cell) => cell.textContent),
    ).toEqual(["Created", ""]);
    expect(
      table.queryByRole("columnheader", { name: "Last login" }),
    ).not.toBeInTheDocument();
    const userCells = within(table.getAllByRole("row")[1]).getAllByRole("cell");
    expect(userCells.at(-2)).toHaveTextContent("2026");
    expect(
      within(userCells.at(-1)!).getByRole("button", { name: /Tasks for/ }),
    ).toBeVisible();
  });

  it("puts activity filters above the grid and separates compact activity fields", () => {
    render(<AdminActivitiesPage />);
    const input = screen.getByRole("searchbox", { name: "Search activities" });
    const table = screen.getByRole("table", { name: "Activities" });
    expect(input.closest(".card")).toBe(table.closest(".card"));
    expect(
      input.compareDocumentPosition(table) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    for (const name of [
      "ID",
      "User",
      "Sport",
      "Source",
      "Format",
      "Import",
      "Import status",
    ]) {
      expect(screen.getByRole("columnheader", { name })).toBeVisible();
    }
    expect(
      screen.getByRole("link", { name: "A long hill ride title" }),
    ).toHaveAttribute("title", "A long hill ride title");
    expect(screen.getByText("processed")).toHaveAttribute("title", "complete");
    expect(screen.getByText("processed")).toHaveClass("badge", "badge-success");
    expect(screen.getByText("processed")).not.toHaveClass("badge-outline");
    expect(
      within(table)
        .getAllByRole("columnheader")
        .slice(-2)
        .map((cell) => cell.textContent),
    ).toEqual(["Started", ""]);
    const activityCells = within(
      within(table).getAllByRole("row")[1],
    ).getAllByRole("cell");
    expect(activityCells.at(-2)).toHaveTextContent("2026");
    expect(
      within(activityCells.at(-1)!).getByRole("button", { name: /Tasks for/ }),
    ).toBeVisible();
    expect(
      within(table).getByRole("cell", { name: "manual_upload" })
        .firstElementChild,
    ).toBeNull();
  });

  it("resets activity pagination when searching and sends URL filters to the API", async () => {
    mocks.search = "page=2&source=strava_sync&user_id=7&sport=road_ride";
    render(<AdminActivitiesPage />);
    expect(mocks.activities).toHaveBeenLastCalledWith(
      expect.objectContaining({
        page: 2,
        source: "strava_sync",
        userId: 7,
        sport: "road_ride",
      }),
    );
    fireEvent.change(screen.getByLabelText("Search activities"), {
      target: { value: "Hill" },
    });
    await waitFor(() =>
      expect(mocks.replace).toHaveBeenCalledWith(
        "/admin/activities?page=1&source=strava_sync&user_id=7&sport=road_ride&q=Hill",
        { scroll: false },
      ),
    );
    await userEvent
      .setup()
      .click(screen.getByRole("button", { name: "Clear" }));
    expect(mocks.push).toHaveBeenCalledWith("/admin/activities", {
      scroll: false,
    });
  });

  it("keeps invalid activity owners and API failures visible", () => {
    mocks.search = "user_id=0";
    const view = render(<AdminActivitiesPage />);
    expect(mocks.activities).toHaveBeenLastCalledWith(
      expect.objectContaining({ enabled: false }),
    );
    expect(screen.getByRole("alert")).toHaveTextContent(
      "User ID must be positive",
    );
    mocks.search = "";
    mocks.activities.mockReturnValue({
      data: [],
      error: new Error("Cannot load activities"),
    });
    view.rerender(<AdminActivitiesPage />);
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Cannot load activities",
    );
  });

  it("keeps integration filters in the event grid card and clears provider and IDs", async () => {
    mocks.pathname = "/admin/integrations";
    mocks.search = "provider=strava&user_id=7&activity_id=42&import_id=17";
    const user = userEvent.setup();
    render(<AdminIntegrationsPage />);
    const provider = screen.getByLabelText("Provider");
    const table = screen.getByRole("table");
    expect(screen.getByText("success")).toHaveClass("badge", "badge-success");
    expect(screen.getByText("success")).not.toHaveClass("badge-outline");
    expect(provider.closest(".card")).toBe(table.closest(".card"));
    expect(
      provider.compareDocumentPosition(table) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(mocks.events).toHaveBeenLastCalledWith(
      expect.objectContaining({
        provider: "strava",
        userId: 7,
        activityId: 42,
        importId: 17,
      }),
    );
    await user.type(
      screen.getByLabelText("Search recent integration events"),
      "missing",
    );
    expect(
      screen.getByText("No integration events matched the current filter."),
    ).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Clear" }));
    expect(provider).toHaveValue("");
    expect(screen.getByLabelText("User ID")).toHaveValue(null);
    expect(
      screen.getByRole("button", { name: "Inspect event 1" }),
    ).toBeVisible();
  });

  it("rejects fractional integration IDs instead of querying an unrelated population", () => {
    mocks.pathname = "/admin/integrations";
    mocks.search = "user_id=1.5";
    render(<AdminIntegrationsPage />);
    expect(mocks.events).toHaveBeenLastCalledWith(
      expect.objectContaining({ enabled: false }),
    );
    expect(screen.getByRole("alert")).toHaveTextContent(
      "IDs must be positive integers.",
    );
  });

  it("preserves all providers in task-history links when the page is refreshed", () => {
    mocks.pathname = "/admin/integrations";
    mocks.search = "provider=&user_id=7&activity_id=42";
    render(<AdminIntegrationsPage />);
    expect(screen.getByLabelText("Provider")).toHaveValue("");
    expect(mocks.events).toHaveBeenLastCalledWith(
      expect.objectContaining({
        provider: undefined,
        userId: 7,
        activityId: 42,
      }),
    );
    expect(mocks.replace).not.toHaveBeenCalled();
  });

  it("removes retired analytics and manual task pages from navigation", () => {
    render(<Nav />);
    expect(
      screen.queryByRole("link", { name: "Analytics" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("link", { name: "Manual tasks" }),
    ).not.toBeInTheDocument();
  });

  it("blocks Search for invalid integration IDs and resumes with a valid owner", async () => {
    mocks.pathname = "/admin/integrations";
    mocks.search = "user_id=invalid";
    const user = userEvent.setup();
    render(<AdminIntegrationsPage />);
    await user.click(screen.getByRole("button", { name: "Search" }));
    expect(mocks.refetch).not.toHaveBeenCalled();
    await user.clear(screen.getByLabelText("User ID"));
    await user.type(screen.getByLabelText("User ID"), "7");
    await user.click(screen.getByRole("button", { name: "Search" }));
    expect(mocks.refetch).toHaveBeenCalledTimes(1);
    expect(mocks.events).toHaveBeenLastCalledWith(
      expect.objectContaining({ userId: 7, enabled: true }),
    );
  });
});
