import { render, screen } from "@testing-library/react";
import { expect, it } from "vitest";
import StatusBadge from "../ui/StatusBadge";

it.each([
  ["Active", "badge-success"],
  ["Completed", "badge-success"],
  ["processed", "badge-success"],
  ["complete", "badge-success"],
  ["Failed", "badge-error"],
  ["TimedOut", "badge-error"],
  ["Running", "badge-info"],
  ["activity_parsed", "badge-info"],
  ["queued", "badge-warning"],
  ["Canceled", "badge-warning"],
  ["duplicate", "badge-warning"],
  ["new_state", "badge-neutral"],
])("renders %s as a filled semantic status pill", (status, color) => {
  render(<StatusBadge status={status} />);
  const badge = screen.getByText(status);
  expect(badge).toHaveClass("badge", color);
  expect(badge).not.toHaveClass("badge-outline", "badge-sm", "uppercase");
});

it("keeps the display label and stage tooltip separate from the status color", () => {
  render(
    <StatusBadge
      status="Running"
      label="Running / queued"
      title="activity_parsed"
    />,
  );
  expect(screen.getByText("Running / queued")).toHaveAttribute(
    "title",
    "activity_parsed",
  );
  expect(screen.getByText("Running / queued")).toHaveClass("badge-info");
});
