import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import ActionMenu from "../ui/ActionMenu";

it("renders outside clipped rows and supports keyboard selection without row clicks", async () => {
  const select = vi.fn();
  const rowClick = vi.fn();
  const user = userEvent.setup();
  const { container } = render(
    <div onClick={rowClick} style={{ overflow: "hidden" }}>
      <ActionMenu
        label="Row tasks"
        items={[
          { label: "Disabled", disabled: true, onSelect: select },
          { label: "First", onSelect: select },
          { label: "Last", onSelect: select },
        ]}
      />
    </div>,
  );
  const trigger = screen.getByRole("button", { name: "Row tasks" });
  await user.click(trigger);
  expect(container).not.toContainElement(screen.getByRole("menu"));
  expect(screen.getByRole("menuitem", { name: "First" })).toHaveFocus();
  await user.keyboard("{ArrowDown}");
  expect(screen.getByRole("menuitem", { name: "Last" })).toHaveFocus();
  await user.keyboard("{Home}{ArrowUp}");
  expect(screen.getByRole("menuitem", { name: "Last" })).toHaveFocus();
  await user.keyboard("{Enter}");
  expect(select).toHaveBeenCalledOnce();
  expect(rowClick).not.toHaveBeenCalled();
  expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  expect(trigger).toHaveFocus();
});

it("dismisses on Escape and outside clicks while allowing its own menu to scroll", async () => {
  const user = userEvent.setup();
  render(
    <ActionMenu
      label="Row tasks"
      items={[{ label: "Task", onSelect: vi.fn() }]}
    />,
  );
  const trigger = screen.getByRole("button", { name: "Row tasks" });
  await user.click(trigger);
  fireEvent.scroll(screen.getByRole("menu"));
  expect(screen.getByRole("menu")).toBeInTheDocument();
  await user.keyboard("{Escape}");
  expect(trigger).toHaveFocus();
  expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  await user.click(trigger);
  await user.click(document.body);
  expect(trigger).toHaveAttribute("aria-expanded", "false");
});

it("keeps the menu anchored and usable when a wide grid scrolls after opening", async () => {
  const select = vi.fn();
  const user = userEvent.setup();
  const { container } = render(
    <div>
      <ActionMenu
        label="Row tasks"
        items={[{ label: "Task", onSelect: select }]}
      />
    </div>,
  );
  const trigger = screen.getByRole("button", { name: "Row tasks" });
  const bounds = vi
    .spyOn(trigger, "getBoundingClientRect")
    .mockReturnValue(new DOMRect(500, 100, 32, 32));
  const focus = vi.spyOn(HTMLElement.prototype, "focus");
  await user.click(trigger);
  expect(focus).toHaveBeenCalledWith({ preventScroll: true });
  bounds.mockReturnValue(new DOMRect(400, 100, 32, 32));
  fireEvent.scroll(container.firstChild!);
  expect(screen.getByRole("menu")).toHaveStyle({
    right: `${window.innerWidth - 432}px`,
    top: "136px",
  });
  await user.click(screen.getByRole("menuitem", { name: "Task" }));
  expect(select).toHaveBeenCalledOnce();
  bounds.mockRestore();
  focus.mockRestore();
});
