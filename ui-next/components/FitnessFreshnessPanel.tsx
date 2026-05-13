"use client";

import { auth } from "@ericbutera/kaleido";
import { useMemo, useState } from "react";
import {
  Bar,
  CartesianGrid,
  ComposedChart,
  Line,
  ReferenceArea,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { extractApiMessage } from "../lib/activityFormatting";
import {
  type FitnessFreshnessPoint,
  useFitnessFreshness,
} from "../lib/queries";
import AuthRequiredCard from "./AuthRequiredCard";

const LOAD_COLOR = "#94a3b8";
const FITNESS_COLOR = "#2563eb";
const FATIGUE_COLOR = "#7c3aed";
const FORM_COLOR = "#059669";

const RANGE_PRESETS = [
  { key: "all", label: "All time" },
  { key: "3m", label: "3 months", months: 3 },
  { key: "6m", label: "6 months", months: 6 },
  { key: "1y", label: "Year", months: 12 },
  { key: "2y", label: "2 years", months: 24 },
] as const;

const ACTIVE_TOGGLE_CLASS =
  "badge-outline border-current bg-base-100 text-base-content";
const INACTIVE_TOGGLE_CLASS =
  "border-transparent bg-base-300/80 text-base-content/55 opacity-70";

type RangePresetKey = (typeof RANGE_PRESETS)[number]["key"];

function formatDateLabel(value: string, includeYear = false) {
  const date = new Date(`${value}T00:00:00Z`);

  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return date.toLocaleDateString(undefined, {
    month: "short",
    day: includeYear ? undefined : "numeric",
    year: includeYear ? "2-digit" : undefined,
  });
}

function formatTooltipDate(value: string) {
  const date = new Date(`${value}T00:00:00Z`);

  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return date.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

function formatMetric(value: number | null | undefined) {
  return typeof value === "number" ? value.toFixed(1) : "--";
}

function subtractMonths(date: Date, months: number) {
  const next = new Date(
    Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate()),
  );
  next.setUTCMonth(next.getUTCMonth() - months);
  return next;
}

function toDateParam(date: Date) {
  return date.toISOString().slice(0, 10);
}

function currentFormZone(form: number | null | undefined) {
  if (form == null) {
    return {
      label: "No form",
      badgeClassName: "badge-ghost",
      description: "Import more activities to establish a training history.",
    };
  }

  if (form <= -25) {
    return {
      label: "High risk",
      badgeClassName: "badge-error",
      description:
        "You are carrying deep fatigue. A recovery block is likely due.",
    };
  }

  if (form <= -10) {
    return {
      label: "Build",
      badgeClassName: "badge-warning",
      description:
        "Fatigue is above fitness, which is where most fitness gains happen.",
    };
  }

  if (form < 5) {
    return {
      label: "Neutral",
      badgeClassName: "badge-ghost",
      description: "A balanced zone between loading and recovering.",
    };
  }

  if (form < 20) {
    return {
      label: "Fresh",
      badgeClassName: "badge-success",
      description: "Fatigue is down while fitness is still elevated.",
    };
  }

  return {
    label: "Race ready",
    badgeClassName: "badge-info",
    description: "You are fresh and carrying good fitness for a goal effort.",
  };
}

function FitnessTooltip({
  active,
  payload,
  label,
}: {
  active?: boolean;
  payload?: Array<{ payload?: FitnessFreshnessPoint }>;
  label?: string;
}) {
  if (!active || !payload?.length || !label) {
    return null;
  }

  const point = payload[0]?.payload;

  if (!point) {
    return null;
  }

  return (
    <div className="rounded-box border border-base-300 bg-base-100 px-3 py-3 shadow-lg">
      <p className="text-sm font-semibold text-base-content">
        {formatTooltipDate(label)}
      </p>
      <div className="mt-2 space-y-1.5 text-sm text-base-content/75">
        <div className="flex items-center justify-between gap-4">
          <span>Training load</span>
          <span className="font-medium text-base-content">
            {formatMetric(point.training_load)}
          </span>
        </div>
        <div className="flex items-center justify-between gap-4">
          <span>Fitness</span>
          <span className="font-medium text-base-content">
            {formatMetric(point.fitness)}
          </span>
        </div>
        <div className="flex items-center justify-between gap-4">
          <span>Fatigue</span>
          <span className="font-medium text-base-content">
            {formatMetric(point.fatigue)}
          </span>
        </div>
        <div className="flex items-center justify-between gap-4">
          <span>Form</span>
          <span className="font-medium text-base-content">
            {formatMetric(point.form)}
          </span>
        </div>
      </div>
    </div>
  );
}

function FormTooltip({
  active,
  payload,
  label,
}: {
  active?: boolean;
  payload?: Array<{ payload?: FitnessFreshnessPoint }>;
  label?: string;
}) {
  if (!active || !payload?.length || !label) {
    return null;
  }

  const point = payload[0]?.payload;

  if (!point) {
    return null;
  }

  return (
    <div className="rounded-box border border-base-300 bg-base-100 px-3 py-3 shadow-lg">
      <p className="text-sm font-semibold text-base-content">
        {formatTooltipDate(label)}
      </p>
      <p className="mt-2 text-sm text-base-content/75">
        {`Form ${formatMetric(point.form)}`}
      </p>
    </div>
  );
}

function SummaryMetric({
  label,
  value,
  accent,
  description,
}: {
  label: string;
  value: string;
  accent?: string;
  description?: string;
}) {
  return (
    <div className="stat rounded-box bg-base-200 px-4 py-3 shadow-sm">
      <div className="stat-title">{label}</div>
      <div
        className="stat-value text-2xl"
        style={accent ? { color: accent } : undefined}
      >
        {value}
      </div>
      {description ? <div className="stat-desc">{description}</div> : null}
    </div>
  );
}

export default function FitnessFreshnessPanel() {
  const authApi = auth.useAuthApi();
  const { user, isLoading: isLoadingUser } = authApi.useCurrentUser();
  const [selectedRange, setSelectedRange] = useState<RangePresetKey>("6m");
  const [showFitness, setShowFitness] = useState(true);
  const [showFatigue, setShowFatigue] = useState(true);
  const endDate = useMemo(() => new Date(), []);
  const endDateParam = useMemo(() => toDateParam(endDate), [endDate]);
  const startDateParam = useMemo(() => {
    const preset = RANGE_PRESETS.find((entry) => entry.key === selectedRange);

    if (!preset || !("months" in preset)) {
      return undefined;
    }

    return toDateParam(subtractMonths(endDate, preset.months));
  }, [endDate, selectedRange]);
  const fitnessQuery = useFitnessFreshness({
    enabled: !!user,
    startDate: startDateParam,
    endDate: endDateParam,
  });
  const points = fitnessQuery.data?.points ?? [];
  const latestPoint = points.at(-1) ?? null;
  const lastSevenDayLoad = points
    .slice(-7)
    .reduce((total, point) => total + point.training_load, 0);
  const formZone = currentFormZone(latestPoint?.form);
  const formMin = Math.min(...points.map((point) => point.form), -30);
  const formMax = Math.max(...points.map((point) => point.form), 30);
  const formDomain: [number, number] = [
    Math.floor((formMin - 5) / 5) * 5,
    Math.ceil((formMax + 5) / 5) * 5,
  ];
  const includeYearLabels = selectedRange === "all" || selectedRange === "2y";

  if (isLoadingUser || fitnessQuery.isLoading) {
    return (
      <section className="card bg-base-100 shadow-xl">
        <div className="card-body items-center py-10">
          <span className="loading loading-spinner loading-md" />
        </div>
      </section>
    );
  }

  if (!user) {
    return (
      <AuthRequiredCard
        eyebrow="Fitness"
        title="Sign in to view fitness and freshness"
        description="Bike builds fitness, fatigue, and form from your recorded activity load over time."
      />
    );
  }

  if (fitnessQuery.isError) {
    return (
      <section className="card bg-base-100 shadow-xl">
        <div className="card-body">
          <div className="alert alert-error">
            {extractApiMessage(fitnessQuery.error)}
          </div>
        </div>
      </section>
    );
  }

  return (
    <section className="space-y-6">
      <div className="card bg-base-100 shadow-xl">
        <div className="card-body gap-6">
          <div className="flex flex-wrap items-start justify-between gap-4">
            <div>
              <p className="text-sm text-base-content/60">Training load</p>
              <h1 className="mt-2 text-4xl font-semibold">
                Fitness &amp; Freshness
              </h1>
              <p className="mt-3 max-w-3xl text-sm leading-6 text-base-content/70">
                Fitness tracks your 42-day exponentially weighted load average.
                Fatigue tracks your 7-day average. Form is fitness minus
                fatigue. Bike currently estimates daily load from duration plus
                average heart rate, then rolls it forward with the same CTL/ATL
                style model used by training tools.
              </p>
            </div>

            <div className="join">
              {RANGE_PRESETS.map((preset) => (
                <button
                  key={preset.key}
                  type="button"
                  className={`join-item btn btn-sm ${selectedRange === preset.key ? "btn-primary" : "btn-ghost"}`}
                  onClick={() => {
                    setSelectedRange(preset.key);
                  }}
                >
                  {preset.label}
                </button>
              ))}
            </div>
          </div>

          {points.length > 0 ? (
            <>
              <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
                <SummaryMetric
                  label="Current fitness"
                  value={formatMetric(latestPoint?.fitness)}
                  accent={FITNESS_COLOR}
                  description={`${fitnessQuery.data?.fitness_window_days ?? 42} day load average`}
                />
                <SummaryMetric
                  label="Current fatigue"
                  value={formatMetric(latestPoint?.fatigue)}
                  accent={FATIGUE_COLOR}
                  description={`${fitnessQuery.data?.fatigue_window_days ?? 7} day load average`}
                />
                <div className="stat rounded-box bg-base-200 px-4 py-3 shadow-sm">
                  <div className="stat-title">Current form</div>
                  <div className="flex items-center gap-3">
                    <div
                      className="stat-value text-2xl"
                      style={{ color: FORM_COLOR }}
                    >
                      {formatMetric(latestPoint?.form)}
                    </div>
                    <span className={`badge ${formZone.badgeClassName}`}>
                      {formZone.label}
                    </span>
                  </div>
                  <div className="stat-desc">{formZone.description}</div>
                </div>
                <SummaryMetric
                  label="Last 7 day load"
                  value={formatMetric(lastSevenDayLoad)}
                  description="Sum of estimated daily load across the trailing week"
                />
              </div>

              <div className="rounded-box border border-base-300 bg-base-200 p-4">
                <div className="mb-3 flex flex-wrap gap-2 text-xs text-base-content/75">
                  <span className="badge badge-outline gap-2 px-3 py-3">
                    <span
                      className="inline-block h-2.5 w-2.5 rounded-full"
                      style={{ backgroundColor: LOAD_COLOR }}
                    />
                    Load
                  </span>
                  <button
                    type="button"
                    className={`badge gap-2 px-3 py-3 transition ${showFitness ? ACTIVE_TOGGLE_CLASS : INACTIVE_TOGGLE_CLASS}`}
                    aria-pressed={showFitness}
                    onClick={() => {
                      setShowFitness((value) => !value);
                    }}
                  >
                    <span
                      className="inline-block h-2.5 w-2.5 rounded-full"
                      style={{ backgroundColor: FITNESS_COLOR }}
                    />
                    Fitness
                  </button>
                  <button
                    type="button"
                    className={`badge gap-2 px-3 py-3 transition ${showFatigue ? ACTIVE_TOGGLE_CLASS : INACTIVE_TOGGLE_CLASS}`}
                    aria-pressed={showFatigue}
                    onClick={() => {
                      setShowFatigue((value) => !value);
                    }}
                  >
                    <span
                      className="inline-block h-2.5 w-2.5 rounded-full"
                      style={{ backgroundColor: FATIGUE_COLOR }}
                    />
                    Fatigue
                  </button>
                </div>

                <div
                  role="img"
                  aria-label="Fitness and fatigue chart"
                  className="h-[360px] w-full"
                >
                  <ResponsiveContainer
                    width="100%"
                    height="100%"
                    minWidth={320}
                    minHeight={360}
                  >
                    <ComposedChart
                      data={points}
                      margin={{ top: 8, right: 12, bottom: 12, left: 0 }}
                    >
                      <CartesianGrid
                        vertical={false}
                        stroke="var(--color-base-content)"
                        strokeOpacity={0.1}
                      />
                      <XAxis
                        axisLine={false}
                        dataKey="date"
                        tick={{
                          fill: "var(--color-base-content)",
                          fontSize: 10,
                        }}
                        tickFormatter={(value: string) =>
                          formatDateLabel(value, includeYearLabels)
                        }
                        tickLine={false}
                        minTickGap={28}
                      />
                      <YAxis
                        axisLine={false}
                        tick={{
                          fill: "var(--color-base-content)",
                          fontSize: 10,
                        }}
                        tickLine={false}
                        width={54}
                      />
                      <YAxis hide yAxisId="load" />
                      <Tooltip content={<FitnessTooltip />} />
                      <Bar
                        dataKey="training_load"
                        yAxisId="load"
                        fill={LOAD_COLOR}
                        fillOpacity={0.35}
                        barSize={10}
                        radius={[4, 4, 0, 0]}
                      />
                      <Line
                        type="monotone"
                        dataKey="fitness"
                        hide={!showFitness}
                        stroke={FITNESS_COLOR}
                        strokeWidth={3}
                        dot={false}
                        activeDot={{
                          r: 5,
                          fill: FITNESS_COLOR,
                          stroke: "var(--color-base-100)",
                          strokeWidth: 1.25,
                        }}
                      />
                      <Line
                        type="monotone"
                        dataKey="fatigue"
                        hide={!showFatigue}
                        stroke={FATIGUE_COLOR}
                        strokeWidth={3}
                        dot={false}
                        activeDot={{
                          r: 5,
                          fill: FATIGUE_COLOR,
                          stroke: "var(--color-base-100)",
                          strokeWidth: 1.25,
                        }}
                      />
                    </ComposedChart>
                  </ResponsiveContainer>
                </div>
              </div>

              <div className="rounded-box border border-base-300 bg-base-200 p-4">
                <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
                  <div>
                    <h2 className="text-lg font-semibold text-base-content">
                      Form
                    </h2>
                    <p className="text-sm text-base-content/70">
                      Negative form means you are carrying fatigue. Positive
                      form means you are getting fresher.
                    </p>
                  </div>
                  <div className="flex flex-wrap gap-2 text-xs text-base-content/75">
                    <span className="badge badge-error badge-outline">
                      High risk &lt; -25
                    </span>
                    <span className="badge badge-warning badge-outline">
                      Build -25 to -10
                    </span>
                    <span className="badge badge-ghost">Neutral -10 to 5</span>
                    <span className="badge badge-success badge-outline">
                      Fresh 5 to 20
                    </span>
                    <span className="badge badge-info badge-outline">
                      Race ready &gt; 20
                    </span>
                  </div>
                </div>

                <div
                  role="img"
                  aria-label="Form chart"
                  className="h-[220px] w-full"
                >
                  <ResponsiveContainer
                    width="100%"
                    height="100%"
                    minWidth={320}
                    minHeight={220}
                  >
                    <ComposedChart
                      data={points}
                      margin={{ top: 8, right: 12, bottom: 12, left: 0 }}
                    >
                      <CartesianGrid
                        vertical={false}
                        stroke="var(--color-base-content)"
                        strokeOpacity={0.1}
                      />
                      <XAxis
                        axisLine={false}
                        dataKey="date"
                        tick={{
                          fill: "var(--color-base-content)",
                          fontSize: 10,
                        }}
                        tickFormatter={(value: string) =>
                          formatDateLabel(value, includeYearLabels)
                        }
                        tickLine={false}
                        minTickGap={28}
                      />
                      <YAxis
                        axisLine={false}
                        domain={formDomain}
                        tick={{
                          fill: "var(--color-base-content)",
                          fontSize: 10,
                        }}
                        tickLine={false}
                        width={54}
                      />
                      <ReferenceArea
                        y1={formDomain[0]}
                        y2={-25}
                        fill="#dc2626"
                        fillOpacity={0.08}
                      />
                      <ReferenceArea
                        y1={-25}
                        y2={-10}
                        fill="#f59e0b"
                        fillOpacity={0.1}
                      />
                      <ReferenceArea
                        y1={-10}
                        y2={5}
                        fill="#94a3b8"
                        fillOpacity={0.08}
                      />
                      <ReferenceArea
                        y1={5}
                        y2={20}
                        fill="#22c55e"
                        fillOpacity={0.08}
                      />
                      <ReferenceArea
                        y1={20}
                        y2={formDomain[1]}
                        fill="#0ea5e9"
                        fillOpacity={0.08}
                      />
                      <ReferenceLine
                        y={0}
                        stroke="#64748b"
                        strokeDasharray="4 4"
                      />
                      <Tooltip content={<FormTooltip />} />
                      <Line
                        type="monotone"
                        dataKey="form"
                        stroke={FORM_COLOR}
                        strokeWidth={3}
                        dot={false}
                        activeDot={{
                          r: 5,
                          fill: FORM_COLOR,
                          stroke: "var(--color-base-100)",
                          strokeWidth: 1.25,
                        }}
                      />
                    </ComposedChart>
                  </ResponsiveContainer>
                </div>
              </div>
            </>
          ) : (
            <div className="alert bg-base-200 text-base-content/80">
              <span>
                No activities are available yet. Import a few sessions with
                duration data to start building fitness and fatigue history.
              </span>
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
