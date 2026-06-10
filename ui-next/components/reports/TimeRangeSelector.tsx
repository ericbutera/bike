"use client";

import React from "react";

export type TimeRange = "day" | "week" | "month" | "3month" | "6month" | "1year" | "2year";

const OPTIONS: { key: TimeRange; label: string }[] = [
  { key: "day", label: "Day" },
  { key: "week", label: "Week" },
  { key: "month", label: "Month" },
  { key: "3month", label: "3 Month" },
  { key: "6month", label: "6 Month" },
  { key: "1year", label: "1 Year" },
  { key: "2year", label: "2 Year" },
];

export default function TimeRangeSelector({
  value,
  onChange,
}: {
  value: TimeRange;
  onChange: (v: TimeRange) => void;
}) {
  return (
    <div className="flex gap-2">
      {OPTIONS.map((opt) => (
        <button
          key={opt.key}
          type="button"
          className={
            "btn btn-ghost btn-sm " + (value === opt.key ? "btn-active" : "")
          }
          onClick={() => onChange(opt.key)}
        >
          {opt.label}
        </button>
      ))}
    </div>
  );
}
