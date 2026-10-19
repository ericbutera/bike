import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { ACTIVITY_SPORT_OPTIONS } from "../../lib/activitySports";
import ActivitySportFilter from "../activity-stream/ActivitySportFilter";

it("shows icons and text for every sport and marks the current choice", async () => {
  const user = userEvent.setup();
  render(<ActivitySportFilter value="mountain_bike" onChange={vi.fn()} />);

  await user.click(
    screen.getByRole("button", { name: "Activity type: Mountain bike" }),
  );

  expect(screen.getAllByRole("menuitemradio")).toHaveLength(
    ACTIVITY_SPORT_OPTIONS.length + 1,
  );
  for (const { value, label } of ACTIVITY_SPORT_OPTIONS) {
    const choice = screen.getByRole("menuitemradio", { name: label });
    expect(choice).toHaveAttribute(
      "aria-checked",
      String(value === "mountain_bike"),
    );
    expect(choice.querySelector("svg")).toBeInTheDocument();
    expect(within(choice).queryByRole("img")).not.toBeInTheDocument();
  }
  expect(
    screen.getByRole("menuitemradio", { name: "Mountain bike" }),
  ).toHaveFocus();
});

it("selects a sport by keyboard and restores focus to the trigger", async () => {
  const onChange = vi.fn();
  const user = userEvent.setup();
  render(<ActivitySportFilter value="walk" onChange={onChange} />);
  const trigger = screen.getByRole("button", { name: "Activity type: Walk" });
  trigger.focus();

  await user.keyboard("{Enter}{ArrowDown}{Enter}");

  expect(onChange).toHaveBeenCalledExactlyOnceWith("hike");
  expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  expect(trigger).toHaveFocus();
});

it("clears the filter through All activity types", async () => {
  const onChange = vi.fn();
  const user = userEvent.setup();
  render(<ActivitySportFilter value="road_ride" onChange={onChange} />);
  await user.click(
    screen.getByRole("button", { name: "Activity type: Road ride" }),
  );
  await user.click(
    screen.getByRole("menuitemradio", { name: "All activity types" }),
  );
  expect(onChange).toHaveBeenCalledExactlyOnceWith("");
});

it("shows All activity types as selected when no filter is active", async () => {
  const user = userEvent.setup();
  render(<ActivitySportFilter onChange={vi.fn()} />);
  await user.click(
    screen.getByRole("button", { name: "Activity type: All activity types" }),
  );
  expect(
    screen.getByRole("menuitemradio", { name: "All activity types" }),
  ).toHaveAttribute("aria-checked", "true");
});
