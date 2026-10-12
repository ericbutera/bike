"use client";

import {
  faCircleInfo,
  faCircleQuestion,
} from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { useId, useRef, useState } from "react";
import { createPortal } from "react-dom";

type InfoTooltipProps = {
  label: string;
  tip: string;
  position?: "top" | "right" | "bottom" | "left";
  className?: string;
  icon?: "info" | "help";
};

type TooltipCoordinates = {
  left: number;
  top: number;
  transform: string;
};

export default function InfoTooltip({
  label,
  tip,
  position = "right",
  className,
  icon = "info",
}: InfoTooltipProps) {
  const tooltipId = useId();
  const triggerRef = useRef<HTMLSpanElement | null>(null);
  const [coordinates, setCoordinates] = useState<TooltipCoordinates | null>(
    null,
  );
  const [container, setContainer] = useState<Element | null>(null);

  function openTooltip() {
    const trigger = triggerRef.current;
    if (!trigger) {
      return;
    }

    setContainer(trigger.closest("dialog") ?? document.body);
    setCoordinates(coordinatesFor(trigger.getBoundingClientRect(), position));
  }

  function closeTooltip() {
    setCoordinates(null);
  }

  return (
    <>
      <span
        ref={triggerRef}
        className={`inline-flex cursor-help items-center text-base-content/60 ${className ?? ""}`.trim()}
        aria-label={label}
        aria-describedby={coordinates ? tooltipId : undefined}
        tabIndex={0}
        onPointerEnter={openTooltip}
        onPointerLeave={closeTooltip}
        onFocus={openTooltip}
        onBlur={closeTooltip}
      >
        <FontAwesomeIcon
          icon={icon === "help" ? faCircleQuestion : faCircleInfo}
          className="h-4 w-4"
          aria-hidden
        />
      </span>
      {coordinates && container
        ? createPortal(
            <span
              id={tooltipId}
              role="tooltip"
              className="fixed z-[9999] max-w-sm whitespace-normal rounded bg-neutral px-3 py-2 text-sm font-normal leading-5 text-neutral-content shadow-lg"
              style={coordinates}
            >
              {tip}
            </span>,
            container,
          )
        : null}
    </>
  );
}

function coordinatesFor(
  rect: DOMRect,
  position: NonNullable<InfoTooltipProps["position"]>,
): TooltipCoordinates {
  const offset = 8;

  switch (position) {
    case "top":
      return {
        left: rect.left + rect.width / 2,
        top: rect.top - offset,
        transform: "translate(-50%, -100%)",
      };
    case "bottom":
      return {
        left: rect.left + rect.width / 2,
        top: rect.bottom + offset,
        transform: "translate(-50%, 0)",
      };
    case "left":
      return {
        left: rect.left - offset,
        top: rect.top + rect.height / 2,
        transform: "translate(-100%, -50%)",
      };
    case "right":
      return {
        left: rect.right + offset,
        top: rect.top + rect.height / 2,
        transform: "translate(0, -50%)",
      };
  }
}
