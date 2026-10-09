import { render, screen } from "@testing-library/react";
import type { ComponentProps } from "react";
import { describe, expect, it, vi } from "vitest";
import Layout from "../Layout";
import { LocalAdminLayout, StatItem } from "../admin/LocalAdminLayout";

vi.mock("../Navigation", () => ({
  default: () => <nav aria-label="Bike">Bike navigation</nav>,
}));
vi.mock("next/navigation", () => ({
  usePathname: () => "/admin/tasks",
  useRouter: () => ({ replace: vi.fn() }),
}));
vi.mock("next/link", () => ({
  default: (props: ComponentProps<"a">) => <a {...props} />,
}));
vi.mock("../../lib/auth", () => ({
  useAuth: () => ({ user: { id: 7, is_admin: true } }),
}));

describe("page shells", () => {
  it("places page content under the shared navigation", () => {
    render(
      <Layout mainClassName="ride-page">
        <h1>My rides</h1>
      </Layout>,
    );
    expect(
      screen.getByRole("navigation", { name: "Bike" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("main")).toHaveClass("ride-page");
    expect(
      screen.getByRole("heading", { name: "My rides" }),
    ).toBeInTheDocument();
  });

  it("renders admin content and highlights the current sidebar route", () => {
    render(
      <LocalAdminLayout title="Task overview">
        <p>One task completed.</p>
      </LocalAdminLayout>,
    );
    expect(
      screen.getByRole("heading", { name: "Task overview" }),
    ).toBeInTheDocument();
    expect(screen.getByText("One task completed.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Tasks" })).toHaveClass(
      "menu-active",
    );
  });

  it("displays a successful metric's value and reporting period", () => {
    render(<StatItem title="Completed" value={12} desc="last 24 hours" />);
    expect(screen.getByText("Completed")).toBeInTheDocument();
    expect(screen.getByText("12")).toBeInTheDocument();
    expect(screen.getByText("last 24 hours")).toBeInTheDocument();
  });
});
