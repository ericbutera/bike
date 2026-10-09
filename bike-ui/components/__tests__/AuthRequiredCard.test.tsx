import { render, screen } from "@testing-library/react";
import type { ImageProps } from "next/image";
import type { ComponentProps } from "react";
import { describe, expect, it, vi } from "vitest";
import AuthRequiredCard from "../AuthRequiredCard";

vi.mock("next/link", () => ({
  default: (props: ComponentProps<"a">) => <a {...props} />,
}));
vi.mock("next/image", () => ({
  default: ({ alt }: ImageProps) => <span role="img" aria-label={alt} />,
}));

describe("sign-in invitation", () => {
  it("shows the application preview and link to sign in", () => {
    render(<AuthRequiredCard />);
    expect(screen.getByRole("img")).toHaveAccessibleName(
      "all vibes bike analytic platform",
    );
    expect(screen.getByRole("link", { name: "Sign in" })).toHaveAttribute(
      "href",
      "/login",
    );
    expect(screen.getByText(/Sign in to view activities/)).toBeInTheDocument();
  });
});
