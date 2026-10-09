import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import ThemeToggle from "../ThemeToggle";

beforeEach(() => {
  localStorage.clear();
  document.documentElement.removeAttribute("data-theme");
  document.documentElement.style.colorScheme = "";
  vi.stubGlobal(
    "matchMedia",
    vi.fn().mockReturnValue({
      matches: false,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    }),
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
  document.documentElement.removeAttribute("data-theme");
  document.documentElement.style.colorScheme = "";
});

describe("theme selection", () => {
  it("applies and stores the rider's selected dark theme", () => {
    render(<ThemeToggle />);
    const toggle = screen.getByRole("checkbox", {
      name: "Switch to dark mode",
    });
    fireEvent.click(toggle);

    expect(localStorage.getItem("bike-theme")).toBe("dark");
    expect(document.documentElement).toHaveAttribute("data-theme", "dark");
    expect(document.documentElement.style.colorScheme).toBe("dark");
    expect(
      screen.getByRole("checkbox", { name: "Switch to light mode" }),
    ).toBeChecked();
  });
});
