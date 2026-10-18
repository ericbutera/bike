import { fireEvent, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Map as MapLibreMap } from "maplibre-gl";
import RouteMapNavigationControl from "../RouteMapNavigationControl";

vi.mock("../../lib/maplibre", () => ({
  NavigationControl: class {
    private element = document.createElement("div");
    onAdd() {
      for (const label of ["Zoom in", "Zoom out"]) {
        const button = document.createElement("button");
        button.setAttribute("aria-label", label);
        this.element.append(button);
      }
      return this.element;
    }
    onRemove() {
      this.element.remove();
    }
  },
}));

describe("RouteMapNavigationControl", () => {
  let control: RouteMapNavigationControl;
  let element: HTMLElement;
  const onChange = vi.fn();
  beforeEach(() => {
    onChange.mockClear();
    control = new RouteMapNavigationControl({
      segments: true,
      climbs: false,
      onChange,
    });
    element = control.onAdd({} as MapLibreMap);
    document.body.append(element);
  });
  afterEach(() => {
    control.onRemove();
  });

  it("places layers below both zoom controls and exposes the defaults", async () => {
    const user = userEvent.setup();
    expect(
      within(element)
        .getAllByRole("button")
        .map((button) => button.getAttribute("aria-label")),
    ).toEqual(["Zoom in", "Zoom out", "Map layers"]);
    const button = screen.getByRole("button", { name: "Map layers" });
    expect(button).toHaveAttribute("aria-expanded", "false");
    expect(
      screen.queryByRole("group", { name: "Map layers" }),
    ).not.toBeInTheDocument();
    await user.click(button);
    expect(button).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("checkbox", { name: "Segments" })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: "Climbs" })).not.toBeChecked();
  });

  it("toggles each layer by keyboard and updates controls without replacing them", async () => {
    const user = userEvent.setup();
    screen.getByRole("button", { name: "Map layers" }).focus();
    await user.keyboard("{Enter}");
    const climbs = screen.getByRole("checkbox", { name: "Climbs" });
    climbs.focus();
    await user.keyboard(" ");
    expect(onChange).toHaveBeenLastCalledWith("climbs", true);
    control.setLayers({ segments: true, climbs: true, onChange });
    expect(screen.getByRole("checkbox", { name: "Climbs" })).toBe(climbs);
    expect(climbs).toHaveFocus();
    await user.click(screen.getByRole("checkbox", { name: "Segments" }));
    expect(onChange).toHaveBeenLastCalledWith("segments", false);
    control.setLayers({ segments: false, climbs: true, onChange });
    expect(
      screen.getByRole("checkbox", { name: "Segments" }),
    ).not.toBeChecked();
  });

  it("closes on Escape and outside input while preserving the appropriate focus", async () => {
    const user = userEvent.setup();
    const button = screen.getByRole("button", { name: "Map layers" });
    await user.click(button);
    screen.getByRole("checkbox", { name: "Climbs" }).focus();
    await user.keyboard("{Escape}");
    expect(button).toHaveFocus();
    expect(button).toHaveAttribute("aria-expanded", "false");
    await user.click(button);
    const outside = document.createElement("input");
    document.body.append(outside);
    await user.click(outside);
    expect(button).toHaveAttribute("aria-expanded", "false");
    expect(outside).toHaveFocus();
    outside.remove();
  });

  it("removes popup listeners and retains normal navigation without layer options", async () => {
    const user = userEvent.setup();
    const button = screen.getByRole("button", { name: "Map layers" });
    await user.click(button);
    control.setLayers(undefined);
    expect(within(element).getAllByRole("button")).toHaveLength(2);
    expect(button).toHaveAttribute("aria-expanded", "false");
    control.setLayers({ segments: false, climbs: true, onChange });
    await user.click(button);
    const focus = vi.spyOn(button, "focus");
    control.onRemove();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(focus).not.toHaveBeenCalled();
  });
});
