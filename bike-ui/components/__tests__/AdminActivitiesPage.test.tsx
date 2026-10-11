import { fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import AdminActivitiesPage from "../../app/admin/activities/page";
import type { AdminActivity } from "../../lib/queries";

vi.mock("../../lib/queries", () => ({ useAdminActivities: vi.fn() }));
vi.mock("../admin/LocalAdminLayout", () => ({
  LocalAdminLayout: ({ children }: { children: ReactNode }) => children,
}));
vi.mock("../admin/ActivityPipelines", () => ({
  default: ({ activityId }: { activityId: number }) => (
    <p>Worker runs for #{activityId}</p>
  ),
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
  it("opens the combined import page for linked activities and preserves legacy worker inspection", () => {
    render(<AdminActivitiesPage />);
    expect(
      screen.getByRole("link", {
        name: "View processing pipeline for Linked ride",
      }),
    ).toHaveAttribute("href", "/admin/imports/1676");
    fireEvent.click(
      screen.getByRole("button", {
        name: "View processing pipeline for Legacy ride",
      }),
    );
    expect(screen.getByText("Worker runs for #43")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});
