"use client";

import { faCircleInfo } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";

type InfoTooltipProps = {
  label: string;
  tip: string;
  position?: "top" | "right" | "bottom" | "left";
  className?: string;
};

export default function InfoTooltip({
  label,
  tip,
  position = "right",
  className,
}: InfoTooltipProps) {
  return (
    <span
      className={`tooltip tooltip-${position} inline-flex cursor-help items-center text-base-content/60 ${className ?? ""}`.trim()}
      data-tip={tip}
      aria-label={label}
      tabIndex={0}
      title={tip}
    >
      <FontAwesomeIcon icon={faCircleInfo} className="h-4 w-4" aria-hidden />
    </span>
  );
}
