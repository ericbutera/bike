import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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

  it("keeps top-level dropdown menus mutually exclusive", async () => {
    const user = userEvent.setup();
    render(<Navigation />);

    const trainingSummary = screen.getByText("Training");
    const accountSummary = screen.getAllByText("Account")[0];
    const trainingMenu = trainingSummary.closest("details");
    const accountMenu = accountSummary.closest("details");

    expect(trainingMenu).not.toBeNull();
    expect(accountMenu).not.toBeNull();

    await user.click(trainingSummary);
    expect(trainingMenu).toHaveAttribute("open");
    expect(accountMenu).not.toHaveAttribute("open");

    await user.click(accountSummary);
    expect(trainingMenu).not.toHaveAttribute("open");
    expect(accountMenu).toHaveAttribute("open");
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
