import { render, renderHook, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { personalAchievement } from "../../tests/fixtures/activitySegments";
import { useSegmentPersonalAchievements } from "../../lib/queries";
import SegmentPersonalAchievements from "../segment-detail/SegmentPersonalAchievements";

const query = vi.hoisted(() => ({ use: vi.fn() }));
vi.mock("../../lib/api", () => ({
  $api: { useQuery: query.use },
  $typedApi: { useQuery: query.use },
}));

beforeEach(() => {
  query.use.mockReset().mockReturnValue({ data: [], isError: false });
});

describe("saved personal achievements", () => {
  it("displays the saved PR, second and third with links to the exact attempts", () => {
    query.use.mockReturnValue({
      data: [
        personalAchievement({ duration_seconds: 355 }),
        personalAchievement({
          position: 2,
          effort_id: 2,
          duration_seconds: 360,
        }),
        personalAchievement({
          position: 3,
          effort_id: 3,
          duration_seconds: 360,
        }),
      ],
    });
    render(<SegmentPersonalAchievements segmentId={14} />);
    const panel = screen.getByRole("region", { name: "Personal achievements" });
    expect(query.use).toHaveBeenCalledWith(
      "get",
      "/segments/{id}/personal-achievements",
      { params: { path: { id: 14 } } },
      { enabled: true },
    );
    expect(within(panel).getByText("All-time")).toBeInTheDocument();
    const links = within(panel).getAllByRole("link");
    expect(links.map((link) => link.textContent)).toEqual([
      "Your PR5m 55s",
      "Second6m 00s",
      "Third6m 00s",
    ]);
    expect(links.map((link) => link.getAttribute("href"))).toEqual([
      "/segments/14?efforts=1",
      "/segments/14?efforts=2",
      "/segments/14?efforts=3",
    ]);
  });

  it("shows only the saved positions for a rider with one attempt", () => {
    query.use.mockReturnValue({ data: [personalAchievement()] });
    render(<SegmentPersonalAchievements segmentId={14} />);
    expect(screen.getAllByRole("listitem")).toHaveLength(1);
    expect(screen.queryByText("Second")).not.toBeInTheDocument();
    expect(screen.queryByText("Third")).not.toBeInTheDocument();
  });

  it("shows an empty state when no achievements are saved", () => {
    render(<SegmentPersonalAchievements segmentId={14} />);
    expect(
      screen.getByText("No personal achievements yet."),
    ).toBeInTheDocument();
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
  });

  it("shows loading before the first saved response", () => {
    query.use.mockReturnValue({ data: undefined, isError: false });
    render(<SegmentPersonalAchievements segmentId={14} />);
    expect(screen.getByRole("status")).toHaveTextContent(
      "Loading personal achievements...",
    );
  });

  it("shows a failed read without presenting an empty success", () => {
    query.use.mockReturnValue({ data: undefined, isError: true });
    render(<SegmentPersonalAchievements segmentId={14} />);
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Unable to load personal achievements.",
    );
    expect(
      screen.queryByText("No personal achievements yet."),
    ).not.toBeInTheDocument();
  });

  it("keeps saved positions visible during a failed refresh", () => {
    query.use.mockReturnValue({
      data: [personalAchievement()],
      isError: true,
      isFetching: true,
    });
    render(<SegmentPersonalAchievements segmentId={14} />);
    expect(
      screen.getByRole("link", { name: "Your PR 5m 12s" }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});

it.each(["invalid", 0, -1])("does not request an invalid segment %s", (id) => {
  renderHook(() => useSegmentPersonalAchievements(id));
  expect(query.use).toHaveBeenCalledWith(
    "get",
    "/segments/{id}/personal-achievements",
    { params: { path: { id: 0 } } },
    { enabled: false },
  );
});
