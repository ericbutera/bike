import { render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { ReactNode } from "react";
import AdminTasksPage from "../../app/admin/tasks/page";
import { initializeClientConfig } from "../../lib/config";

vi.mock("../admin/LocalAdminLayout", () => ({
  LocalAdminLayout: ({ children }: { children: ReactNode }) => children,
}));

afterEach(() => initializeClientConfig({}));

it("opens native workflow status and controls from the tasks page", () => {
  initializeClientConfig({ TEMPORAL_UI_URL: "https://temporal.example.com" });
  render(<AdminTasksPage />);
  expect(
    screen.getByRole("link", { name: "Open Temporal workflows" }),
  ).toHaveAttribute(
    "href",
    "https://temporal.example.com/namespaces/bike/workflows",
  );
  expect(
    screen.getByText(
      /Inspect running tasks, failures, retries, and cancellations/,
    ),
  ).toBeInTheDocument();
});
