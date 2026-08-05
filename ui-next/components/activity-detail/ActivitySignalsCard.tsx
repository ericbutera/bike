"use client";

import { useMemo, useState } from "react";
import {
  Area,
  CartesianGrid,
  ComposedChart,
  Line,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import {
  formatElevation,
  formatHeartRate,
  formatPower,
  formatSpeed,
  type UnitSystem,
} from "../../lib/activityFormatting";
import type { ActivityChartPoint } from "../../lib/queries";
import { zeroBasedDomain } from "../../lib/chartDomains";
import { AppCard, CardHeader } from "../ui/Card";

type ChartSeriesPoint = {
  x: number;
  y: number;
};

type SignalMetricKey = "heartRate" | "power" | "speed" | "elevation";

type SignalSeries = {
  key: SignalMetricKey;
  label: string;
  points: ChartSeriesPoint[];
  buttonClassName: string;
  dotClassName: string;
  strokeColor: string;
  strokeWidth?: number;
  fillColor?: string;
  summary: string;
};

type SignalChartRow = {
  elapsedSeconds: number;
  heartRate?: number | null;
  power?: number | null;
  speed?: number | null;
  elevation?: number | null;
};

const DEFAULT_VISIBLE_SIGNAL_KEYS: SignalMetricKey[] = [
  "heartRate",
  "elevation",
];

const SIGNAL_KEY_ORDER: SignalMetricKey[] = [
  "heartRate",
  "power",
  "speed",
  "elevation",
];

function formatElapsedAxisLabel(value: number) {
  if (value <= 0) {
    return "0m";
  }

  const hours = Math.floor(value / 3600);
  const minutes = Math.floor((value % 3600) / 60);

  if (hours > 0) {
    return minutes > 0 ? `${hours}h ${minutes}m` : `${hours}h`;
  }

  if (minutes > 0) {
    return `${minutes}m`;
  }

  return `${value}s`;
}

function buildSeries(
  chartPoints: ActivityChartPoint[] | null | undefined,
  selector: (point: ActivityChartPoint) => number | null | undefined,
) {
  return (chartPoints ?? []).flatMap((point) => {
    const value = selector(point);

    if (value == null || Number.isNaN(value)) {
      return [];
    }

    return [
      { x: Math.max(0, point.elapsed_seconds), y: value },
    ] satisfies ChartSeriesPoint[];
  });
}

function maxSeriesValue(points: ChartSeriesPoint[]) {
  return points.reduce(
    (maxValue, point) => Math.max(maxValue, point.y),
    points[0]?.y ?? 0,
  );
}

function minSeriesValue(points: ChartSeriesPoint[]) {
  return points.reduce(
    (minValue, point) => Math.min(minValue, point.y),
    points[0]?.y ?? 0,
  );
}

function buildSignalChartRows(series: SignalSeries[]) {
  const rowsByElapsedSeconds = new Map<number, SignalChartRow>();

  for (const entry of series) {
    for (const point of entry.points) {
      const existing = rowsByElapsedSeconds.get(point.x) ?? {
        elapsedSeconds: point.x,
      };

      rowsByElapsedSeconds.set(point.x, {
        ...existing,
        [entry.key]: point.y,
      });
    }
  }

  return Array.from(rowsByElapsedSeconds.values()).sort(
    (left, right) => left.elapsedSeconds - right.elapsedSeconds,
  );
}

function signalMetricLabel(key: SignalMetricKey) {
  switch (key) {
    case "heartRate":
      return "Heart rate";
    case "power":
      return "Power";
    case "speed":
      return "Speed";
    case "elevation":
      return "Elevation";
  }
}

function formatSignalValue(
  key: SignalMetricKey,
  value: number | null | undefined,
  unitSystem: UnitSystem,
) {
  switch (key) {
    case "heartRate":
      return formatHeartRate(value);
    case "power":
      return formatPower(value);
    case "speed":
      return formatSpeed(value, unitSystem);
    case "elevation":
      return formatElevation(value, unitSystem);
  }
}

function useActivitySignalSeries(
  chartPoints: ActivityChartPoint[] | null | undefined,
  unitSystem: UnitSystem,
) {
  return useMemo<SignalSeries[]>(() => {
    const heartRateSeries = buildSeries(
      chartPoints,
      (point) => point.heart_rate_bpm,
    );
    const powerSeries = buildSeries(chartPoints, (point) => point.power_watts);
    const speedSeries = buildSeries(chartPoints, (point) => point.speed_mps);
    const elevationSeries = buildSeries(
      chartPoints,
      (point) => point.elevation_meters,
    );

    return [
      {
        key: "heartRate",
        label: "Heart rate",
        points: heartRateSeries,
        buttonClassName: "btn-error",
        dotClassName: "bg-error",
        strokeColor: "var(--color-error)",
        strokeWidth: 2,
        summary: `Peak ${formatHeartRate(maxSeriesValue(heartRateSeries))}`,
      },
      {
        key: "power",
        label: "Power",
        points: powerSeries,
        buttonClassName: "btn-warning",
        dotClassName: "bg-warning",
        strokeColor: "var(--color-warning)",
        strokeWidth: 1.75,
        summary: `Peak ${formatPower(maxSeriesValue(powerSeries))}`,
      },
      {
        key: "speed",
        label: "Speed",
        points: speedSeries,
        buttonClassName: "btn-info",
        dotClassName: "bg-info",
        strokeColor: "var(--color-info)",
        strokeWidth: 1.5,
        summary: `Top speed ${formatSpeed(
          maxSeriesValue(speedSeries),
          unitSystem,
        )}`,
      },
      {
        key: "elevation",
        label: "Elevation",
        points: elevationSeries,
        buttonClassName: "btn-success",
        dotClassName: "bg-success",
        strokeColor: "var(--color-success)",
        strokeWidth: 1.75,
        fillColor: "var(--color-success)",
        summary: `${formatElevation(
          minSeriesValue(elevationSeries),
          unitSystem,
        )} to ${formatElevation(maxSeriesValue(elevationSeries), unitSystem)}`,
      },
    ];
  }, [chartPoints, unitSystem]);
}

function SignalChartTooltip({
  active,
  label,
  payload,
  unitSystem,
}: {
  active?: boolean;
  label?: number;
  payload?: Array<{
    color?: string;
    dataKey?: string;
    value?: number;
  }>;
  unitSystem: UnitSystem;
}) {
  if (!active || !payload?.length) {
    return null;
  }

  const orderedItems = payload.flatMap((entry) => {
    if (
      entry.dataKey !== "heartRate" &&
      entry.dataKey !== "power" &&
      entry.dataKey !== "speed" &&
      entry.dataKey !== "elevation"
    ) {
      return [];
    }

    return [
      entry as { color?: string; dataKey: SignalMetricKey; value?: number },
    ];
  });

  return (
    <div className="rounded-box border border-base-300 bg-base-100 px-3 py-3 shadow-lg">
      <p className="text-sm font-semibold text-base-content">
        {formatElapsedAxisLabel(label ?? 0)}
      </p>
      <div className="mt-2 space-y-1.5 text-sm text-base-content/75">
        {orderedItems.map((entry) => (
          <div key={entry.dataKey} className="flex items-center gap-2">
            <span
              aria-hidden
              className="inline-block h-2.5 w-2.5 rounded-full"
              style={{ backgroundColor: entry.color }}
            />
            <span className="font-medium text-base-content">
              {signalMetricLabel(entry.dataKey)}
            </span>
            <span>
              {formatSignalValue(entry.dataKey, entry.value, unitSystem)}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

export default function ActivitySignalsCard({
  chartPoints,
  unitSystem,
}: {
  chartPoints: ActivityChartPoint[] | null | undefined;
  unitSystem: UnitSystem;
}) {
  const [visibleKeys, setVisibleKeys] = useState<SignalMetricKey[]>(
    DEFAULT_VISIBLE_SIGNAL_KEYS,
  );
  const series = useActivitySignalSeries(chartPoints, unitSystem);
  const availableSeries = series.filter((entry) => entry.points.length > 1);
  const visibleSeries = availableSeries.filter((entry) =>
    visibleKeys.includes(entry.key),
  );
  const signalXDomain = zeroBasedDomain(
    availableSeries.flatMap((entry) => entry.points.map((point) => point.x)),
  );
  const rows = buildSignalChartRows(availableSeries);

  function toggleSignalLayer(key: SignalMetricKey) {
    setVisibleKeys((current) => {
      if (current.includes(key)) {
        return current.filter((entry) => entry !== key);
      }

      return SIGNAL_KEY_ORDER.filter(
        (entry) => entry === key || current.includes(entry),
      );
    });
  }

  return (
    <AppCard>
      <CardHeader className="mb-3" title="Ride signals" />

      <div className="flex flex-wrap gap-2">
        {availableSeries.map((entry) => {
          const isVisible = visibleKeys.includes(entry.key);

          return (
            <button
              key={entry.key}
              type="button"
              className={`btn btn-sm ${
                isVisible ? entry.buttonClassName : "btn-ghost"
              }`}
              aria-pressed={isVisible}
              onClick={() => {
                toggleSignalLayer(entry.key);
              }}
            >
              {entry.label}
            </button>
          );
        })}
      </div>

      {visibleSeries.length > 0 ? (
        <>
          <div
            role="img"
            aria-label="Activity signals chart"
            className="mt-5 overflow-hidden rounded-box border border-base-300 bg-base-200 p-3"
          >
            <div className="h-[208px] w-full">
              <ResponsiveContainer
                width="100%"
                height="100%"
                minWidth={320}
                minHeight={208}
              >
                <ComposedChart
                  data={rows}
                  margin={{ top: 8, right: 8, bottom: 8, left: 0 }}
                >
                  <CartesianGrid
                    vertical={false}
                    stroke="var(--color-base-content)"
                    strokeOpacity={0.1}
                  />
                  <XAxis
                    axisLine={false}
                    dataKey="elapsedSeconds"
                    domain={signalXDomain}
                    includeHidden
                    tick={{ fill: "var(--color-base-content)", fontSize: 10 }}
                    tickFormatter={(value: number) =>
                      formatElapsedAxisLabel(value)
                    }
                    tickLine={false}
                    type="number"
                  />
                  {visibleSeries.map((entry) => (
                    <YAxis
                      key={`${entry.key}-axis`}
                      hide
                      yAxisId={entry.key}
                      domain={["dataMin", "dataMax"]}
                    />
                  ))}
                  <Tooltip
                    content={<SignalChartTooltip unitSystem={unitSystem} />}
                  />
                  {visibleSeries.map((entry) =>
                    entry.key === "elevation" ? (
                      <Area
                        key={entry.key}
                        type="linear"
                        dataKey={entry.key}
                        yAxisId={entry.key}
                        stroke={entry.strokeColor}
                        fill={entry.fillColor ?? entry.strokeColor}
                        fillOpacity={0.08}
                        strokeOpacity={0.35}
                        strokeWidth={entry.strokeWidth ?? 1.75}
                        dot={false}
                        connectNulls
                      />
                    ) : (
                      <Line
                        key={entry.key}
                        type="linear"
                        dataKey={entry.key}
                        yAxisId={entry.key}
                        stroke={entry.strokeColor}
                        strokeWidth={entry.strokeWidth ?? 2}
                        dot={false}
                        connectNulls
                      />
                    ),
                  )}
                </ComposedChart>
              </ResponsiveContainer>
            </div>
          </div>

          <div className="mt-4 flex flex-wrap gap-2">
            {visibleSeries.map((entry) => (
              <div
                key={`${entry.key}-summary`}
                className="badge badge-outline gap-2 px-3 py-3"
              >
                <span
                  aria-hidden
                  className={`inline-block h-2.5 w-2.5 rounded-full ${entry.dotClassName}`}
                />
                <span>{entry.summary}</span>
              </div>
            ))}
          </div>
        </>
      ) : (
        <div className="alert mt-5">
          <span>Turn on at least one signal layer to render the chart.</span>
        </div>
      )}
    </AppCard>
  );
}
