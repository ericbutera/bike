"use client";

import React from "react";
import {
  ResponsiveContainer,
  LineChart,
  Line,
  XAxis,
  YAxis,
  Tooltip,
  CartesianGrid,
  BarChart,
  Bar,
  Legend,
} from "recharts";
import type { TimeRange } from "./TimeRangeSelector";
import type { TrainingReportPoint } from "../../lib/queries";

type ChartType =
  | "distance"
  | "z2_speed"
  | "decoupling"
  | "climbing_pace"
  | "hr_zones"
  | "elevation"
  | "activity_type_time";
type ChartRow = {
  label: string;
  distance: number;
  speed?: number | null;
  decoupling?: number | null;
  climbing?: number | null;
  z1: number;
  z2: number;
  z3: number;
  z4: number;
  z5: number;
  elevation: number;
  trainingHours: number;
  raceHours: number;
};

const LINE_CHART_CONFIG = {
  z2_speed: {
    dataKey: "speed",
    name: "Z2 speed",
    unit: "mph",
    axisLabel: "mph",
  },
  decoupling: {
    dataKey: "decoupling",
    name: "Aerobic decoupling",
    unit: "%",
    axisLabel: "%",
  },
  climbing_pace: {
    dataKey: "climbing",
    name: "Median climb rate",
    unit: "ft/h",
    axisLabel: "ft/h",
  },
} as const;

const BAR_CHART_CONFIG = {
  distance: {
    dataKey: "distance",
    name: "Distance",
    unit: "mi",
    axisLabel: "mi",
    fill: "#2563eb",
  },
  elevation: {
    dataKey: "elevation",
    name: "Elevation gain",
    unit: "ft",
    axisLabel: "ft",
    fill: "#16a34a",
  },
} as const;

export function chartBucketLabel(date: Date, range: TimeRange) {
  if (range === "week" || range === "month") {
    return date.toLocaleDateString([], {
      month: "short",
      day: "numeric",
      timeZone: "UTC",
    });
  }
  if (range === "6month") {
    return date.toLocaleDateString([], {
      month: "short",
      day: "numeric",
      timeZone: "UTC",
    });
  }
  if (range === "3year" || range === "5year" || range === "all") {
    return date.toLocaleDateString([], { year: "numeric", timeZone: "UTC" });
  }
  return date.toLocaleDateString([], {
    month: "short",
    year: "numeric",
    timeZone: "UTC",
  });
}

function mapData(
  points: TrainingReportPoint[],
  type: ChartType,
  range: TimeRange,
) {
  return points.map((point) => {
    const label = chartBucketLabel(new Date(point.bucket_start), range);
    const row = {
      label,
      distance: point.distance_miles,
      speed:
        point.z2_average_speed_mps == null
          ? null
          : point.z2_average_speed_mps * 2.236_936,
      decoupling: point.average_aerobic_decoupling_percent,
      climbing: point.climbing_vertical_rate_feet_per_hour,
      z1: point.z1_seconds,
      z2: point.z2_seconds,
      z3: point.z3_seconds,
      z4: point.z4_seconds,
      z5: point.z5_seconds,
      elevation: point.elevation_gain_feet,
      trainingHours: activityTypeHours(point, "training"),
      raceHours: activityTypeHours(point, "race"),
    };

    if (type === "hr_zones") {
      const total = row.z1 + row.z2 + row.z3 + row.z4 + row.z5;
      return total > 0
        ? {
            ...row,
            z1: (row.z1 / total) * 100,
            z2: (row.z2 / total) * 100,
            z3: (row.z3 / total) * 100,
            z4: (row.z4 / total) * 100,
            z5: (row.z5 / total) * 100,
          }
        : row;
    }

    return row;
  });
}

export default function Charts({
  type,
  range,
  points,
}: {
  type: ChartType;
  range: TimeRange;
  points: TrainingReportPoint[];
}) {
  const data = mapData(points, type, range);

  if (type === "activity_type_time") {
    const hasActivityTypeTime = data.some(
      (row) => row.trainingHours > 0 || row.raceHours > 0,
    );

    if (!hasActivityTypeTime) {
      return <EmptyChart label="No activity type time data in this range." />;
    }

    return (
      <div style={{ width: "100%", height: 280 }}>
        <ResponsiveContainer>
          <BarChart data={data}>
            <CartesianGrid strokeDasharray="3 3" />
            <XAxis dataKey="label" />
            <YAxis
              width={58}
              label={{
                value: "hours",
                angle: -90,
                position: "insideLeft",
              }}
              tickFormatter={(value) => formatAxisTick(value)}
            />
            <Tooltip formatter={(value) => formatTooltipValue(value, "h")} />
            <Legend />
            <Bar
              dataKey="trainingHours"
              name="Training"
              stackId="activity-type"
              fill="#2563eb"
            />
            <Bar
              dataKey="raceHours"
              name="Race"
              stackId="activity-type"
              fill="#dc2626"
            />
          </BarChart>
        </ResponsiveContainer>
      </div>
    );
  }

  if (type === "hr_zones") {
    return (
      <div style={{ width: "100%", height: 240 }}>
        <ResponsiveContainer>
          <BarChart data={data}>
            <CartesianGrid strokeDasharray="3 3" />
            <XAxis dataKey="label" />
            <YAxis
              unit="%"
              label={{ value: "%", angle: -90, position: "insideLeft" }}
            />
            <Tooltip formatter={(value) => formatTooltipValue(value, "%")} />
            <Legend />
            <Bar dataKey="z1" stackId="a" fill="#8884d8" />
            <Bar dataKey="z2" stackId="a" fill="#82ca9d" />
            <Bar dataKey="z3" stackId="a" fill="#ffc658" />
            <Bar dataKey="z4" stackId="a" fill="#ff7f7f" />
            <Bar dataKey="z5" stackId="a" fill="#8dd1e1" />
          </BarChart>
        </ResponsiveContainer>
      </div>
    );
  }

  if (type === "distance" || type === "elevation") {
    const config = BAR_CHART_CONFIG[type];
    const barData = data.filter((row) => hasFiniteValue(row, config.dataKey));

    if (barData.length === 0) {
      return (
        <EmptyChart
          label={`No ${config.name.toLowerCase()} data in this range.`}
        />
      );
    }

    return (
      <div style={{ width: "100%", height: 280 }}>
        <ResponsiveContainer>
          <BarChart data={barData}>
            <CartesianGrid strokeDasharray="3 3" />
            <XAxis dataKey="label" />
            <YAxis
              width={58}
              label={{
                value: config.axisLabel,
                angle: -90,
                position: "insideLeft",
              }}
              tickFormatter={(value) => formatAxisTick(value)}
            />
            <Tooltip
              formatter={(value) => formatTooltipValue(value, config.unit)}
            />
            <Bar
              name={config.name}
              dataKey={config.dataKey}
              fill={config.fill}
            />
          </BarChart>
        </ResponsiveContainer>
      </div>
    );
  }

  const config = LINE_CHART_CONFIG[type];
  const lineData = data.filter((row) => hasFiniteValue(row, config.dataKey));

  if (lineData.length === 0) {
    return (
      <EmptyChart
        label={`No ${config.name.toLowerCase()} data in this range.`}
      />
    );
  }

  return (
    <div style={{ width: "100%", height: 240 }}>
      <ResponsiveContainer>
        <LineChart data={lineData}>
          <CartesianGrid strokeDasharray="3 3" />
          <XAxis dataKey="label" />
          <YAxis
            width={58}
            label={{
              value: config.axisLabel,
              angle: -90,
              position: "insideLeft",
            }}
            tickFormatter={(value) => formatAxisTick(value)}
          />
          <Tooltip
            formatter={(value) => formatTooltipValue(value, config.unit)}
          />
          <Line
            type="monotone"
            name={config.name}
            dataKey={config.dataKey}
            stroke="#8884d8"
            strokeWidth={2}
            dot={lineData.length <= 12}
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}

function activityTypeHours(point: TrainingReportPoint, activityType: string) {
  const seconds =
    point.activity_type_times.find((row) => row.activity_type === activityType)
      ?.seconds ?? 0;
  return seconds / 3600;
}

function EmptyChart({ label }: { label: string }) {
  return (
    <div className="flex h-60 items-center justify-center rounded border border-dashed border-base-300 text-sm text-base-content/60">
      {label}
    </div>
  );
}

function hasFiniteValue(row: ChartRow, key: keyof ChartRow) {
  const value = row[key];
  return typeof value === "number" && Number.isFinite(value);
}

function formatAxisTick(value: unknown) {
  return typeof value === "number"
    ? new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 }).format(
        value,
      )
    : `${value}`;
}

function formatTooltipValue(value: unknown, unit: string): [string, string] {
  if (typeof value !== "number") {
    return [`${value}`, unit];
  }

  const formatted = new Intl.NumberFormat(undefined, {
    maximumFractionDigits: value >= 100 ? 0 : 1,
  }).format(value);
  return [`${formatted} ${unit}`, unit];
}
