import { render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { overallAchievement } from "../../tests/fixtures/activitySegments";
import { useSegmentOverallAchievements } from "../../lib/queries";
import SegmentOverallAchievements from "../segment-detail/SegmentOverallAchievements";

const mocks = vi.hoisted(() => ({ useQuery: vi.fn() }));
vi.mock("../../lib/api", () => ({ $typedApi: { useQuery: mocks.useQuery } }));

beforeEach(() => {
  mocks.useQuery.mockReturnValue({
    data: [],
    isLoading: false,
    isError: false,
  });
});

describe("SegmentOverallAchievements", () => {
  it("shows stored positions and times for different athletes without renumbering", () => {
    mocks.useQuery.mockReturnValue({
      data: [
        overallAchievement(),
        overallAchievement({
          position: 7,
          user_id: 8,
          rider_name: "Other Rider",
          effort_id: 91,
          duration_seconds: 355,
        }),
      ],
      isLoading: false,
      isError: false,
    });
    render(<SegmentOverallAchievements segmentId="11" />);
    const table = screen.getByRole("table", { name: "Overall top ten" });
    const rows = within(table).getAllByRole("row");
    expect(rows).toHaveLength(3);
    expect(
      within(rows[1])
        .getAllByRole("cell")
        .map((cell) => cell.textContent),
    ).toEqual(["1", "Fixture Rider", "5m 12s"]);
    expect(
      within(rows[2])
        .getAllByRole("cell")
        .map((cell) => cell.textContent),
    ).toEqual(["7", "Other Rider", "5m 55s"]);
    expect(
      screen.getByText("All time · Fastest effort per athlete"),
    ).toBeInTheDocument();
    expect(mocks.useQuery).toHaveBeenCalledWith(
      "get",
      "/segments/{id}/overall-achievements",
      { params: { path: { id: 11 } } },
      { enabled: true },
    );
  });

  it("retains a stored position when its profile name is unavailable", () => {
    mocks.useQuery.mockReturnValue({
      data: [overallAchievement({ rider_name: null })],
      isLoading: false,
      isError: false,
    });
    render(<SegmentOverallAchievements segmentId={11} />);
    expect(screen.getByRole("cell", { name: "Athlete" })).toBeInTheDocument();
  });

  it("shows an empty state when no positions are stored", () => {
    render(<SegmentOverallAchievements segmentId={11} />);
    expect(screen.getByText("No overall standings yet.")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  it("shows loading while fetching standings", () => {
    mocks.useQuery.mockReturnValue({ isLoading: true, isError: false });
    render(<SegmentOverallAchievements segmentId={11} />);
    expect(screen.getByText("Loading...")).toBeInTheDocument();
    expect(
      screen.queryByText("No overall standings yet."),
    ).not.toBeInTheDocument();
  });

  it("reports failures without presenting them as empty results", () => {
    mocks.useQuery.mockReturnValue({ isLoading: false, isError: true });
    render(<SegmentOverallAchievements segmentId={11} />);
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Unable to load overall standings.",
    );
  });

  it.each([null, undefined, "bad-id", 0, -1])(
    "disables the query for invalid segment %s",
    (id) => {
      function QueryProbe() {
        useSegmentOverallAchievements(id);
        return null;
      }
      render(<QueryProbe />);
      expect(mocks.useQuery).toHaveBeenLastCalledWith(
        "get",
        "/segments/{id}/overall-achievements",
        { params: { path: { id: 0 } } },
        { enabled: false },
      );
    },
  );
});
