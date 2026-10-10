import { render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import Nav from "../admin/Nav";

vi.mock("next/navigation", () => ({ usePathname: () => "/admin/imports/17" }));

it("opens import history in Admin and keeps its detail route active", () => {
  render(<Nav />);
  expect(screen.getByRole("link", { name: "Import history" })).toHaveAttribute(
    "href",
    "/admin/imports",
  );
  expect(screen.getByRole("link", { name: "Import history" })).toHaveClass(
    "menu-active",
  );
});
