import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { overallAchievement } from "../../tests/fixtures/activitySegments";
import {
  useSegmentOverallAchievements,
  type OverallAchievement,
} from "../../lib/queries";
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

function renderStandings(data: OverallAchievement[] = [overallAchievement()]) {
  mocks.useQuery.mockReturnValue({ data, isLoading: false, isError: false });
  render(<SegmentOverallAchievements segmentId={11} />);
}

function QueryProbe({ id }: { id: number | string | null | undefined }) {
  useSegmentOverallAchievements(id);
  return null;
}

describe("SegmentOverallAchievements", () => {
  it("preserves stored positions without renumbering", () => {
    renderStandings([overallAchievement({ position: 7 })]);
    expect(screen.getByRole("cell", { name: "7" })).toBeInTheDocument();
  });

  it("displays the athlete's profile name", () => {
    renderStandings([overallAchievement({ rider_name: "Other Rider" })]);
    expect(
      screen.getByRole("cell", { name: "Other Rider" }),
    ).toBeInTheDocument();
  });

  it("formats the saved effort duration", () => {
    renderStandings([overallAchievement({ duration_seconds: 355 })]);
    expect(screen.getByRole("cell", { name: "5m 55s" })).toBeInTheDocument();
  });

  it("describes the standings as all time and one effort per athlete", () => {
    render(<SegmentOverallAchievements segmentId={11} />);
    expect(
      screen.getByText("All time · Fastest effort per athlete"),
    ).toBeInTheDocument();
  });

  it("uses an athlete label when the profile name is unavailable", () => {
    renderStandings([overallAchievement({ rider_name: null })]);
    expect(screen.getByRole("cell", { name: "Athlete" })).toBeInTheDocument();
  });

  it("shows an empty state when no positions are stored", () => {
    renderStandings([]);
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
});

describe("useSegmentOverallAchievements", () => {
  it("requests standings for the selected segment", () => {
    render(<QueryProbe id="11" />);
    expect(mocks.useQuery).toHaveBeenCalledWith(
      "get",
      "/segments/{id}/overall-achievements",
      { params: { path: { id: 11 } } },
      { enabled: true },
    );
  });

  it.each([null, undefined, "bad-id", 0, -1])(
    "disables the query for invalid segment %s",
    (id) => {
      render(<QueryProbe id={id} />);
      expect(mocks.useQuery).toHaveBeenLastCalledWith(
        "get",
        "/segments/{id}/overall-achievements",
        { params: { path: { id: 0 } } },
        { enabled: false },
      );
    },
  );
});
