import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Navigation from "../Navigation";

const mocks = vi.hoisted(() => ({
  logoutAsync: vi.fn(),
  useCurrentUser: vi.fn(),
}));

vi.mock("@ericbutera/kaleido", () => ({
  auth: {
    useAuthApi: () => ({
      useCurrentUser: mocks.useCurrentUser,
      useLogout: () => ({
        mutateAsync: mocks.logoutAsync,
        isPending: false,
      }),
    }),
  },
}));

vi.mock("next/link", () => ({
  default: ({ href, children, ...props }: any) => (
    <a href={href} {...props}>
      {children}
    </a>
  ),
}));

vi.mock("../ThemeToggle", () => ({
  default: () => <button type="button">Theme</button>,
}));

describe("Navigation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.useCurrentUser.mockReturnValue({
      user: { id: 1, email: "rider@example.com", is_admin: true },
      isLoading: false,
    });
  });

  it("renders authenticated navigation as a daisyUI megamenu", () => {
    const { container } = render(<Navigation />);

    const megamenu = container.querySelector(".megamenu");
    const trainingButton = screen.getByRole("button", { name: "Training" });
    const accountButton = screen.getAllByRole("button", {
      name: "Account",
    })[0];
    const trainingMenu = document.getElementById("bike-nav-training-menu");
    const accountMenu = document.getElementById("bike-nav-account-menu");

    expect(megamenu).toHaveAttribute("id", "bike-nav-menu");
    expect(megamenu).toHaveClass("megamenu-wide");
    expect(megamenu).toHaveClass("max-sm:megamenu-vertical");
    expect(megamenu).toHaveAttribute("popover", "auto");
    expect(container.querySelector("details")).not.toBeInTheDocument();

    expect(trainingButton).toHaveAttribute(
      "popovertarget",
      "bike-nav-training-menu",
    );
    expect(trainingMenu).toHaveAttribute("popover", "auto");
    expect(screen.getByRole("link", { name: "Segments" })).toHaveAttribute(
      "href",
      "/segments",
    );

    expect(accountButton).toHaveAttribute(
      "popovertarget",
      "bike-nav-account-menu",
    );
    expect(accountMenu).toHaveAttribute("popover", "auto");
    expect(screen.getByRole("link", { name: "Admin" })).toHaveAttribute(
      "href",
      "/admin",
    );
  });

  it("hides protected navigation when signed out", () => {
    mocks.useCurrentUser.mockReturnValue({
      user: null,
      isLoading: false,
    });

    render(<Navigation />);

    expect(screen.queryByText("Activities")).not.toBeInTheDocument();
    expect(screen.queryByText("Training")).not.toBeInTheDocument();
    expect(screen.queryByText("Account")).not.toBeInTheDocument();
    expect(screen.queryByText("Theme")).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Sign in" })).toHaveAttribute(
      "href",
      "/login",
    );
  });
});
