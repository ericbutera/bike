import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { ActivityHeaderActions } from "../activity-detail/ActivityHeaderActions";
import { activity, climbingRoutePoints } from "../../tests/fixtures/activities";
import { initializeClientConfig } from "../../lib/config";

const mutation = vi.hoisted(() => ({
  updateAsync: vi.fn(),
  isPending: false,
  isError: false,
}));
vi.mock("../../lib/queries", () => ({ useUpdateActivity: () => mutation }));
beforeEach(() => {
  vi.clearAllMocks();
  mutation.isPending = false;
  mutation.isError = false;
});

function renderHeader() {
  return render(
    <ActivityHeaderActions
      activity={activity({ sport: "mountain_bike", activity_type: "training" })}
      isDeleting={false}
      onOpenEditDialog={vi.fn()}
      onDelete={vi.fn()}
    />,
  );
}

afterEach(() => initializeClientConfig({}));

it("keeps operational actions out of the activity menu", () => {
  initializeClientConfig({ TEMPORAL_UI_URL: "https://temporal.example.test" });
  const props = {
    activity: activity({
      sport: "mountain_bike_ride",
      activity_import_id: 17,
      route_points: climbingRoutePoints(),
      can_regenerate: true,
      can_download_source_file: true,
    }),
    isDeleting: false,
    onOpenEditDialog: vi.fn(),
    onDelete: vi.fn(),
  };
  render(<ActivityHeaderActions {...props} />);
  expect(
    screen.getByRole("button", { name: "Change activity type: Mountain bike" }),
  ).toBeInTheDocument();
  expect(screen.getAllByRole("link").map((link) => link.textContent)).toEqual([
    "Build segment",
    "Download original source",
  ]);
  const menu = screen
    .getByRole("button", { name: "Open activity actions" })
    .parentElement!.querySelector("ul")!;
  expect(
    within(menu)
      .getAllByRole("button")
      .map((button) => button.textContent),
  ).toEqual(["Edit activity", "Delete activity"]);
  expect(within(menu).getAllByRole("listitem")[0]).toHaveTextContent(
    "Edit activity",
  );
});

it("saves the activity type and training/race selectors as individual field patches", async () => {
  const user = userEvent.setup();
  mutation.updateAsync.mockResolvedValue(activity());
  renderHeader();
  await user.click(
    screen.getByRole("button", { name: "Change activity type: Mountain bike" }),
  );
  await user.click(screen.getByRole("menuitemradio", { name: "Hike" }));
  expect(mutation.updateAsync).toHaveBeenCalledWith(42, { sport: "hike" });
  await user.click(
    screen.getByRole("button", { name: "Change training/race: Training" }),
  );
  await user.click(screen.getByRole("menuitemradio", { name: "Race" }));
  expect(mutation.updateAsync).toHaveBeenCalledWith(42, {
    activity_type: "race",
  });
});

it("disables classification choices while saving and exposes a save failure", () => {
  mutation.isPending = true;
  mutation.isError = true;
  renderHeader();
  expect(
    screen.getByRole("button", { name: "Change activity type: Mountain bike" }),
  ).toBeDisabled();
  expect(
    screen.getByRole("button", { name: "Change training/race: Training" }),
  ).toBeDisabled();
  expect(screen.getByRole("alert")).toHaveTextContent(
    "Unable to update activity",
  );
});
