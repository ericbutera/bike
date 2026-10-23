import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import ActivityModal from "../activity-detail/ActivityModal";
import { activity } from "../../tests/fixtures/activities";

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

it("saves changed sport, purpose, location, and description without resending the title", async () => {
  const user = userEvent.setup();
  const close = vi.fn();
  mutation.updateAsync.mockResolvedValue(activity());
  render(<ActivityModal activity={activity()} onClose={close} />);
  expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
  await user.selectOptions(screen.getByLabelText("Activity type"), "hike");
  await user.click(screen.getByRole("radio", { name: "Race" }));
  await user.type(screen.getByLabelText("Location"), "Empire, Michigan");
  await user.type(screen.getByLabelText("Description"), "Dunes loop");
  await user.click(screen.getByRole("button", { name: "Save" }));
  expect(mutation.updateAsync).toHaveBeenCalledWith(42, {
    sport: "hike",
    activity_type: "race",
    location: "Empire, Michigan",
    description: "Dunes loop",
  });
  expect(close).toHaveBeenCalledOnce();
});

it("sends explicit clears while preserving an unknown stored sport", async () => {
  const user = userEvent.setup();
  mutation.updateAsync.mockResolvedValue(activity());
  render(
    <ActivityModal
      activity={activity({
        sport: "fat_bike",
        location: "Empire, Michigan",
        description: "Dunes loop",
      })}
      onClose={vi.fn()}
    />,
  );
  expect(screen.getByLabelText("Activity type")).toHaveValue("fat_bike");
  await user.clear(screen.getByLabelText("Location"));
  await user.clear(screen.getByLabelText("Description"));
  await user.click(screen.getByRole("button", { name: "Save" }));
  expect(mutation.updateAsync).toHaveBeenCalledWith(42, {
    location: "",
    description: "",
  });
});

it("keeps the draft open after a failed save and supports retry", async () => {
  const user = userEvent.setup();
  const close = vi.fn();
  mutation.updateAsync.mockRejectedValueOnce(new Error("Unavailable"));
  const props = { activity: activity(), onClose: close };
  const { rerender } = render(<ActivityModal {...props} />);
  await user.type(screen.getByLabelText("Title"), " updated");
  await user.click(screen.getByRole("button", { name: "Save" }));
  mutation.isError = true;
  rerender(<ActivityModal {...props} />);
  expect(screen.getByLabelText("Title")).toHaveValue("Fixture ride updated");
  expect(screen.getByRole("alert")).toHaveTextContent(
    "Your changes are still here",
  );
  expect(close).not.toHaveBeenCalled();
  mutation.updateAsync.mockResolvedValueOnce(activity());
  await user.click(screen.getByRole("button", { name: "Save" }));
  expect(close).toHaveBeenCalledOnce();
});

it("disables editing and dismissal while saving", () => {
  mutation.isPending = true;
  render(<ActivityModal activity={activity()} onClose={vi.fn()} />);
  expect(screen.getByLabelText("Title")).toBeDisabled();
  expect(screen.getByLabelText("Activity type")).toBeDisabled();
  expect(screen.getByLabelText("Location")).toBeDisabled();
  expect(screen.getByLabelText("Description")).toBeDisabled();
  expect(screen.getByRole("radio", { name: "Race" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Cancel" })).toBeDisabled();
  expect(
    screen.getByRole("button", { name: "Close activity modal" }),
  ).toBeDisabled();
});

it("edits recorded gear and private notes when present", async () => {
  const user = userEvent.setup();
  mutation.updateAsync.mockResolvedValue(activity());
  render(
    <ActivityModal
      activity={activity({
        gear: { id: "bike-7", name: "Trail bike" },
        private_notes: "Personal note",
      })}
      onClose={vi.fn()}
    />,
  );
  await user.clear(screen.getByLabelText("Gear name"));
  await user.type(screen.getByLabelText("Gear name"), "Winter bike");
  await user.clear(screen.getByLabelText("Private notes"));
  await user.type(
    screen.getByLabelText("Private notes"),
    "Suspension needs service",
  );
  await user.click(screen.getByRole("button", { name: "Save" }));
  expect(mutation.updateAsync).toHaveBeenCalledWith(42, {
    gear_name: "Winter bike",
    private_notes: "Suspension needs service",
  });
});

it("omits gear and private-note editing when no source values exist", () => {
  render(<ActivityModal activity={activity()} onClose={vi.fn()} />);
  expect(screen.queryByLabelText("Gear name")).not.toBeInTheDocument();
  expect(screen.queryByLabelText("Private notes")).not.toBeInTheDocument();
});

it("keeps text field accessible names independent of their existing values", () => {
  render(
    <ActivityModal
      activity={activity({
        description: "Loop description",
        private_notes: "Personal note",
        gear: { id: "bike-7", name: "Trail bike" },
      })}
      onClose={vi.fn()}
    />,
  );
  expect(screen.getByRole("textbox", { name: "Description" })).toHaveValue(
    "Loop description",
  );
  expect(screen.getByRole("textbox", { name: "Private notes" })).toHaveValue(
    "Personal note",
  );
  expect(screen.getByRole("textbox", { name: "Gear name" })).toHaveValue(
    "Trail bike",
  );
});

it("saves a changed title as its own field patch", async () => {
  const user = userEvent.setup();
  const close = vi.fn();
  mutation.updateAsync.mockResolvedValue(activity());
  render(<ActivityModal activity={activity()} onClose={close} />);

  await user.clear(screen.getByLabelText("Title"));
  await user.type(screen.getByLabelText("Title"), "Evening Trail Work");
  await user.click(screen.getByRole("button", { name: "Save" }));

  expect(mutation.updateAsync).toHaveBeenCalledExactlyOnceWith(42, {
    title: "Evening Trail Work",
  });
  expect(close).toHaveBeenCalledOnce();
});

it("keeps saving disabled for a blank title", async () => {
  const user = userEvent.setup();
  render(<ActivityModal activity={activity()} onClose={vi.fn()} />);

  await user.clear(screen.getByLabelText("Title"));

  expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
  expect(mutation.updateAsync).not.toHaveBeenCalled();
});
