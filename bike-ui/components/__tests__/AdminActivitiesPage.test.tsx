import { render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import AdminActivitiesPage from "../../app/admin/activities/page";
import type { AdminActivity } from "../../lib/queries";

vi.mock("../../lib/queries", () => ({ useAdminActivities: vi.fn() }));
vi.mock("../admin/LocalAdminLayout", () => ({
  LocalAdminLayout: ({ children }: { children: ReactNode }) => children,
}));
vi.mock("../ui/GenericList", () => ({
  default: ({
    columns,
  }: {
    columns: { key: string; render?: (activity: AdminActivity) => ReactNode }[];
  }) => (
    <>
      {[
        { id: 42, title: "Linked ride", activity_import_id: 1676 },
        { id: 43, title: "Legacy ride", activity_import_id: null },
      ].map((activity) => (
        <div key={activity.id}>
          {columns
            .find((column) => column.key === "actions")
            ?.render?.(activity as AdminActivity)}
        </div>
      ))}
    </>
  ),
}));

describe("admin activity processing", () => {
  it("links retained import details without reconstructing task graphs", () => {
    render(<AdminActivitiesPage />);
    expect(
      screen.getByRole("link", {
        name: "View import details for Linked ride",
      }),
    ).toHaveAttribute("href", "/admin/imports/1676");
    expect(
      screen.queryByRole("button", { name: /Legacy ride/ }),
    ).not.toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});
