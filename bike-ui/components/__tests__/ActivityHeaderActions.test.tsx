import { render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { ActivityHeaderActions } from "../activity-detail/ActivityHeaderActions";
import type { Activity } from "../../lib/queries";

it("limits the import replay shortcut to the administrative trace permission", () => {
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
    screen.queryByRole("link", { name: "Processing stages and replay" }),
  ).not.toBeInTheDocument();
  view.rerender(<ActivityHeaderActions {...props} canViewImportTrace />);
  expect(
    screen.getByRole("link", { name: "Processing stages and replay" }),
  ).toHaveAttribute("href", "/admin/imports/17");
});
