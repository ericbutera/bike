"use client";

import { faGear } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { createPortal } from "react-dom";

export type ActionMenuItem = {
  label: string;
  onSelect: () => void;
  disabled?: boolean;
  explanation?: string;
  section?: string;
};

export default function ActionMenu({
  label,
  items,
}: {
  label: string;
  items: ActionMenuItem[];
}) {
  const id = useId();
  const trigger = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState<{
    top: number;
    right: number;
  } | null>(null);
  const isOpen = position !== null;
  const sections = new Map<string, ActionMenuItem[]>();
  for (const item of items) {
    const section = item.section ?? "Actions";
    const entries = sections.get(section) ?? [];
    entries.push(item);
    sections.set(section, entries);
  }
  const grouped = items.some((item) => item.section);
  const rowCount = items.length + (grouped ? sections.size : 0);

  function close() {
    setPosition(null);
    trigger.current?.focus({ preventScroll: true });
  }

  useEffect(() => {
    if (!isOpen) return;
    menuControls(menu.current)[0]?.focus({ preventScroll: true });
    function dismiss(event: MouseEvent) {
      if (menu.current?.contains(event.target as Node)) return;
      if (trigger.current?.contains(event.target as Node)) return;
      setPosition(null);
    }
    const reposition = (event: Event) => {
      if (event.target instanceof Node && menu.current?.contains(event.target))
        return;
      if (trigger.current)
        setPosition(
          menuPosition(trigger.current.getBoundingClientRect(), rowCount),
        );
    };
    document.addEventListener("mousedown", dismiss);
    window.addEventListener("resize", reposition);
    window.addEventListener("scroll", reposition, true);
    return () => {
      document.removeEventListener("mousedown", dismiss);
      window.removeEventListener("resize", reposition);
      window.removeEventListener("scroll", reposition, true);
    };
  }, [isOpen, rowCount]);

  function navigate(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      close();
      return;
    }
    if (event.key === "Tab") {
      close();
      return;
    }
    const buttons = menuControls(menu.current);
    const current = buttons.indexOf(document.activeElement as HTMLElement);
    const next = {
      ArrowDown: (current + 1) % buttons.length,
      ArrowUp: (current - 1 + buttons.length) % buttons.length,
      Home: 0,
      End: buttons.length - 1,
    }[event.key];
    if (next !== undefined) {
      event.preventDefault();
      buttons[next]?.focus();
    }
  }

  return (
    <span onClick={(event) => event.stopPropagation()}>
      <button
        ref={trigger}
        type="button"
        className="btn btn-ghost btn-square btn-sm"
        aria-label={label}
        title={label}
        aria-haspopup="menu"
        aria-expanded={!!position}
        aria-controls={position ? id : undefined}
        onClick={() => {
          if (position) return close();
          const rect = trigger.current!.getBoundingClientRect();
          setPosition(menuPosition(rect, rowCount));
        }}
      >
        <FontAwesomeIcon icon={faGear} className="h-4 w-4" />
      </button>
      {position
        ? createPortal(
            <div
              ref={menu}
              id={id}
              role="menu"
              aria-label={label}
              className="fixed z-50 max-h-96 w-72 max-w-[calc(100vw-1rem)] overflow-y-auto rounded-box border border-base-300 bg-base-100 p-2 shadow-xl"
              style={position}
              onKeyDown={navigate}
              onClick={(event) => event.stopPropagation()}
            >
              {grouped ? (
                <ul className="menu menu-vertical w-full p-0" role="none">
                  {Array.from(sections, ([section, entries]) => (
                    <li key={section} role="none">
                      <details open={section === "Viewing"}>
                        <summary tabIndex={0}>{section}</summary>
                        <ul role="group" aria-label={section}>
                          {entries.map((item) => (
                            <li key={item.label} role="none">
                              <MenuAction item={item} onClose={close} />
                            </li>
                          ))}
                        </ul>
                      </details>
                    </li>
                  ))}
                </ul>
              ) : (
                items.map((item) => (
                  <MenuAction key={item.label} item={item} onClose={close} />
                ))
              )}
            </div>,
            document.body,
          )
        : null}
    </span>
  );
}

function menuControls(menu: HTMLDivElement | null) {
  const controls =
    menu?.querySelectorAll<HTMLElement>("summary, button:not(:disabled)") ?? [];
  return Array.from(controls).filter(
    (control) =>
      control.matches("summary") || !control.closest("details:not([open])"),
  );
}

function MenuAction({
  item,
  onClose,
}: {
  item: ActionMenuItem;
  onClose: () => void;
}) {
  return (
    <button
      role="menuitem"
      type="button"
      disabled={item.disabled}
      title={item.explanation}
      className="block w-full rounded-lg px-3 py-2 text-left text-sm hover:bg-base-200 focus:bg-base-200 disabled:opacity-50"
      onClick={() => {
        onClose();
        item.onSelect();
      }}
    >
      {item.label}
      {item.disabled && item.explanation ? (
        <span className="block text-xs">{item.explanation}</span>
      ) : null}
    </button>
  );
}

function menuPosition(rect: DOMRect, itemCount: number) {
  return {
    top: Math.max(
      8,
      Math.min(
        rect.bottom + 4,
        window.innerHeight - Math.min(itemCount * 48 + 16, 400),
      ),
    ),
    right: Math.max(8, window.innerWidth - rect.right),
  };
}
