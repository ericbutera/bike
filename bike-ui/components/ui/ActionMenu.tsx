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

  function close() {
    setPosition(null);
    trigger.current?.focus({ preventScroll: true });
  }

  useEffect(() => {
    if (!isOpen) return;
    menu.current
      ?.querySelector<HTMLButtonElement>("button:not(:disabled)")
      ?.focus({ preventScroll: true });
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
          menuPosition(trigger.current.getBoundingClientRect(), items.length),
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
  }, [isOpen, items.length]);

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
    const buttons = Array.from(
      menu.current?.querySelectorAll<HTMLButtonElement>(
        "button:not(:disabled)",
      ) ?? [],
    );
    const current = buttons.indexOf(
      document.activeElement as HTMLButtonElement,
    );
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
          setPosition(menuPosition(rect, items.length));
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
              {items.map((item) => (
                <button
                  key={item.label}
                  role="menuitem"
                  type="button"
                  disabled={item.disabled}
                  title={item.explanation}
                  className="block w-full rounded-lg px-3 py-2 text-left text-sm hover:bg-base-200 focus:bg-base-200 disabled:opacity-50"
                  onClick={() => {
                    close();
                    item.onSelect();
                  }}
                >
                  {item.label}
                  {item.disabled && item.explanation ? (
                    <span className="block text-xs">{item.explanation}</span>
                  ) : null}
                </button>
              ))}
            </div>,
            document.body,
          )
        : null}
    </span>
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
