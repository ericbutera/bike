"use client";

import React from "react";

export type TimeRange =
  | "week"
  | "month"
  | "6month"
  | "ytd"
  | "1year"
  | "3year"
  | "5year"
  | "all";

export const TIME_RANGE_OPTIONS: { key: TimeRange; label: string }[] = [
  { key: "week", label: "Week" },
  { key: "month", label: "30 days" },
  { key: "6month", label: "6 months" },
  { key: "ytd", label: "Year to Date" },
  { key: "1year", label: "Year (365 days)" },
  { key: "3year", label: "3 years" },
  { key: "5year", label: "5 years" },
  { key: "all", label: "All" },
];

export default function TimeRangeSelector({
  value,
  onChange,
}: {
  value: TimeRange;
  onChange: (v: TimeRange) => void;
}) {
  return (
    <div className="form-control w-full min-w-52">
      <label className="label" htmlFor="report-interval">
        Interval
      </label>
      <select
        id="report-interval"
        className="select w-full"
        value={value}
        onChange={(event) => onChange(event.target.value as TimeRange)}
      >
        {TIME_RANGE_OPTIONS.map((opt) => (
          <option key={opt.key} value={opt.key}>
            {opt.label}
          </option>
        ))}
      </select>
    </div>
  );
}
