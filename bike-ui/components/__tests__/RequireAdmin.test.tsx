import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import RequireAdmin from "../RequireAdmin";

const mocks = vi.hoisted(() => ({ replace: vi.fn() }));
vi.mock("next/navigation", () => ({
  useRouter: () => ({ replace: mocks.replace }),
}));
vi.mock("../../lib/auth", () => ({
  useAuth: () => ({ user: { id: 7, is_admin: true } }),
}));

describe("admin access", () => {
  it("shows protected content to a signed-in administrator", () => {
    render(
      <RequireAdmin>
        <h1>Admin tasks</h1>
      </RequireAdmin>,
    );
    expect(
      screen.getByRole("heading", { name: "Admin tasks" }),
    ).toBeInTheDocument();
    expect(mocks.replace).not.toHaveBeenCalled();
  });
});
