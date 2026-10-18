import { render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import AdminActivitiesPage from "../../app/admin/activities/page";
import type { AdminActivity } from "../../lib/queries";
import { adminActivity } from "@/tests/fixtures/adminTasks";

vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn() }) }));

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
        adminActivity({
          id: 42,
          title: "Linked ride",
          activity_import_id: 1676,
        }),
        adminActivity({
          id: 43,
          title: "Legacy ride",
          activity_import_id: null,
        }),
      ].map((activity) => (
        <div key={activity.id}>
          {columns
            .find((column) => column.key === "actions")
            ?.render?.(activity)}
        </div>
      ))}
    </>
  ),
}));

describe("admin activity processing", () => {
  it("offers task menus on every activity row", () => {
    render(<AdminActivitiesPage />);
    expect(
      screen.getByRole("button", {
        name: "Tasks for Linked ride (#42)",
      }),
    ).toHaveAttribute("aria-haspopup", "menu");
    expect(
      screen.getByRole("button", { name: /Legacy ride/ }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});
