import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { activity, climbingRoutePoints } from "../../tests/fixtures/activities";
import { sharedSegmentFor } from "../../tests/fixtures/segmentTenants";
import SegmentBuilderWorkspace from "../SegmentBuilderWorkspace";
import SegmentBuilderPage from "../SegmentBuilderPage";

const mocks = vi.hoisted(() => ({
  createAsync: vi.fn(),
  push: vi.fn(),
  success: vi.fn(),
  search: new URLSearchParams(),
  useActivity: vi.fn(),
  useSegment: vi.fn(),
}));
vi.mock("next/navigation", () => ({
  useRouter: () => ({ push: mocks.push }),
  useSearchParams: () => mocks.search,
}));
vi.mock("react-hot-toast", () => ({
  default: { success: mocks.success, error: vi.fn() },
}));
vi.mock("../../lib/queries", () => ({
  useActivity: mocks.useActivity,
  useSegment: mocks.useSegment,
  useCreateSegmentFromActivity: () => ({
    createAsync: mocks.createAsync,
    isPending: false,
  }),
}));
vi.mock("../MapLibreRouteMap", () => ({
  default: () => <div role="img" aria-label="Builder map" />,
}));

beforeEach(() => {
  vi.clearAllMocks();
  mocks.useActivity.mockReturnValue({
    data: activity({ route_points: climbingRoutePoints() }),
    isLoading: false,
  });
});

it("opens a new segment builder from the selected activity", () => {
  mocks.search = new URLSearchParams("activityId=42");
  render(<SegmentBuilderPage />);
  expect(mocks.useActivity).toHaveBeenCalledWith(42);
  expect(
    screen.getByRole("heading", { name: "Fixture ride" }),
  ).toBeInTheDocument();
});

it("blocks old saved-segment edit links even when an activity is supplied", () => {
  mocks.search = new URLSearchParams("segmentId=1&activityId=42");
  render(<SegmentBuilderPage />);
  expect(screen.getByRole("alert")).toHaveTextContent(
    "Saved segment geometry is locked. Rename the segment from its detail page.",
  );
  expect(screen.queryByRole("slider")).not.toBeInTheDocument();
  expect(mocks.useActivity).not.toHaveBeenCalled();
  expect(mocks.useSegment).not.toHaveBeenCalled();
});

it.each(["matching-task", undefined])(
  "creates the selected crop and only reports matching when a task was queued (%s)",
  async (processingTaskId) => {
    const user = userEvent.setup();
    const saved = {
      ...sharedSegmentFor(7),
      processing_task_id: processingTaskId,
    };
    mocks.createAsync.mockResolvedValue(saved);
    render(
      <SegmentBuilderWorkspace
        activity={activity({ route_points: climbingRoutePoints() })}
        isLoading={false}
      />,
    );
    await user.click(
      screen.getByRole("button", {
        name: "Move start point forward one point",
      }),
    );
    await user.click(
      screen.getByRole("button", { name: "Move end point backward one point" }),
    );
    await user.type(
      screen.getByRole("textbox", { name: "Segment name" }),
      "  New crop  ",
    );
    await user.click(screen.getByRole("button", { name: "Save segment" }));
    expect(mocks.createAsync).toHaveBeenCalledExactlyOnceWith({
      activityId: 42,
      title: "New crop",
      startRoutePointIndex: 1,
      endRoutePointIndex: 3,
    });
    expect(mocks.push).toHaveBeenCalledExactlyOnceWith("/segments/1");
    expect(mocks.success).toHaveBeenCalledExactlyOnceWith(
      processingTaskId
        ? `Saved Shared climb. Segment matching queued as task ${processingTaskId}.`
        : "Saved Shared climb.",
    );
  },
);
