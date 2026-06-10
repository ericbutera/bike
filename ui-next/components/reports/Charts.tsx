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
  AreaChart,
  Area,
  BarChart,
  Bar,
  Legend,
} from "recharts";
import type { TimeRange } from "./TimeRangeSelector";
import type { TrainingReportPoint } from "../../lib/queries";

type ChartType = "z2_speed" | "decoupling" | "climbing_pace" | "hr_zones" | "elevation";

function makeLabel(date: Date, range: TimeRange) {
  if (range === "day") {
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }
  if (range === "week" || range === "month") {
    return date.toLocaleDateString([], { month: "short", day: "numeric" });
  }
  if (range === "3month" || range === "6month") {
    return date.toLocaleDateString([], { month: "short", day: "numeric" });
  }
  return date.toLocaleDateString([], { year: "numeric", month: "short" });
}

function mapData(points: TrainingReportPoint[], type: ChartType, range: TimeRange) {
  return points.map((point) => {
    const label = makeLabel(new Date(point.bucket_start), range);
    const row = {
      label,
      speed: point.z2_average_speed_mps,
      decoupling: point.average_aerobic_decoupling_percent,
      climbing: point.climbing_pace_feet_per_week,
      z1: point.z1_seconds,
      z2: point.z2_seconds,
      z3: point.z3_seconds,
      z4: point.z4_seconds,
      z5: point.z5_seconds,
      elevation: point.elevation_gain_feet,
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

  if (type === "hr_zones") {
    return (
      <div style={{ width: "100%", height: 240 }}>
        <ResponsiveContainer>
          <BarChart data={data}>
            <CartesianGrid strokeDasharray="3 3" />
            <XAxis dataKey="label" />
            <YAxis unit="%" />
            <Tooltip />
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

  // line charts for other metrics
  const lineKey =
    type === "z2_speed"
      ? "speed"
      : type === "decoupling"
        ? "decoupling"
        : type === "climbing_pace"
          ? "climbing"
          : "elevation";

  return (
    <div style={{ width: "100%", height: 240 }}>
      <ResponsiveContainer>
        <LineChart data={data}>
          <CartesianGrid strokeDasharray="3 3" />
          <XAxis dataKey="label" />
          <YAxis />
          <Tooltip />
          <Line type="monotone" dataKey={lineKey} stroke="#8884d8" strokeWidth={2} dot={false} />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}
