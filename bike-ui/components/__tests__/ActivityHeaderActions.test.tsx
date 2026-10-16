import { render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { ActivityHeaderActions } from "../activity-detail/ActivityHeaderActions";
import type { Activity } from "../../lib/queries";

it("limits import details to administrators", () => {
  const props = {
    activity: {
      id: 42,
      activity_import_id: 17,
      route_points: [],
    } as unknown as Activity,
    isRegenerating: false,
    isDeleting: false,
    onOpenEditDialog: vi.fn(),
    onRegenerate: vi.fn(),
    onDelete: vi.fn(),
  };
  const view = render(<ActivityHeaderActions {...props} />);
  expect(
    screen.queryByRole("link", { name: "Import details" }),
  ).not.toBeInTheDocument();
  view.rerender(<ActivityHeaderActions {...props} isAdmin />);
  expect(screen.getByRole("link", { name: "Import details" })).toHaveAttribute(
    "href",
    "/admin/imports/17",
  );
});
