"use client";

import Link from "next/link";
import { useMemo } from "react";
import {
  Bar,
  CartesianGrid,
  ComposedChart,
  Line,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { formatActivityTimestamp, formatDuration } from "../lib/activityFormatting";
import {
  type DhSegmentProgress,
  type DhSessionSummary,
  type TrainingGoalMetric,
  type TrainingRecommendation,
  useDhGoalProgress,
} from "../lib/queries";
import InfoTooltip from "./ui/InfoTooltip";
import { LoadingSpinner } from "./ui/QueryState";

const SESSION_BAR_COLOR = "#ea580c";
const FADE_LINE_COLOR = "#2563eb";
const GOAL_LINE_COLOR = "#dc2626";
const RECENT_SESSION_SHAPE_HELP_TEXT =
  "Watch lap count and average repeat fade across your latest DH days to see whether volume and consistency are moving together.";

type SessionChartPoint = {
  label: string;
  activityTitle: string;
  effortCount: number;
  averageRepeatFadePercent: number | null;
};

function formatShortDate(value: string) {
  const date = new Date(value);

  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return date.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });
}

function formatCompactMetric(value: number | null | undefined, digits = 1) {
  if (value == null || !Number.isFinite(value)) {
    return "--";
  }

  return value.toFixed(digits);
}

function formatPercent(value: number | null | undefined) {
  if (value == null || !Number.isFinite(value)) {
    return "--";
  }

  return `${value.toFixed(1)}%`;
}

function formatGoalMetricValue(goal: TrainingGoalMetric) {
  if (goal.current_value == null || !Number.isFinite(goal.current_value)) {
    return "--";
  }

  switch (goal.unit) {
    case "seconds":
      return formatDuration(Math.round(goal.current_value));
    case "percent":
      return formatPercent(goal.current_value);
    case "count":
      return Number.isInteger(goal.current_value)
        ? `${goal.current_value}`
        : goal.current_value.toFixed(1);
    case "meters":
      return formatCompactMetric(goal.current_value);
    default:
      return formatCompactMetric(goal.current_value);
  }
}

function formatGoalTargetValue(goal: TrainingGoalMetric) {
  switch (goal.unit) {
    case "seconds":
      return formatDuration(Math.round(goal.target_value));
    case "percent":
      return formatPercent(goal.target_value);
    case "count":
      return Number.isInteger(goal.target_value)
        ? `${goal.target_value}`
        : goal.target_value.toFixed(1);
    case "meters":
      return formatCompactMetric(goal.target_value);
    default:
      return formatCompactMetric(goal.target_value);
  }
}

function goalProgressClass(progressPercent: number | null | undefined) {
  if (progressPercent == null) {
    return "progress progress-neutral";
  }

  if (progressPercent >= 95) {
    return "progress progress-success";
  }

  if (progressPercent >= 60) {
    return "progress progress-warning";
  }

  return "progress progress-error";
}

function recommendationPriorityBadgeClass(
  priority: TrainingRecommendation["priority"],
) {
  switch (priority) {
    case "high":
      return "badge badge-error badge-outline uppercase";
    case "medium":
      return "badge badge-warning badge-outline uppercase";
    default:
      return "badge badge-ghost uppercase";
  }
}

function SummaryStat({
  label,
  value,
  detail,
}: {
  label: string;
  value: string;
  detail: string;
}) {
  return (
    <div className="rounded-box border border-base-300/80 bg-base-100/80 p-4 shadow-sm backdrop-blur">
      <p className="text-xs uppercase tracking-[0.2em] text-base-content/45">
        {label}
      </p>
      <p className="mt-2 text-2xl font-semibold text-base-content">{value}</p>
      <p className="mt-1 text-sm text-base-content/65">{detail}</p>
    </div>
  );
}

function GoalCard({ goal }: { goal: TrainingGoalMetric }) {
  const progressPercent = goal.progress_percent ?? 0;

  return (
    <article className="rounded-box border border-base-300 bg-base-100 p-5 shadow-sm">
      <div className="flex items-start justify-between gap-3">
        <div>
          <p className="text-sm font-medium text-base-content/55">Goal</p>
          <h2 className="mt-1 text-xl font-semibold text-base-content">
            {goal.label}
          </h2>
        </div>
        <span className="badge badge-outline uppercase">
          {goal.direction === "at_least" ? "Build" : "Cap"}
        </span>
      </div>

      <div className="mt-5 flex items-end justify-between gap-4">
        <div>
          <p className="text-sm text-base-content/60">Current</p>
          <p className="text-2xl font-semibold text-base-content">
            {formatGoalMetricValue(goal)}
          </p>
        </div>
        <div className="text-right">
          <p className="text-sm text-base-content/60">Target</p>
          <p className="text-lg font-medium text-base-content">
            {formatGoalTargetValue(goal)}
          </p>
        </div>
      </div>

      <div className="mt-5 space-y-2">
        <div className="flex items-center justify-between text-sm text-base-content/65">
          <span>Progress</span>
          <span>
            {goal.progress_percent != null
              ? `${progressPercent.toFixed(0)}%`
              : "--"}
          </span>
        </div>
        <progress
          className={goalProgressClass(goal.progress_percent)}
          value={progressPercent}
          max={100}
        />
      </div>
    </article>
  );
}

function RecommendationCard({
  recommendation,
}: {
  recommendation: TrainingRecommendation;
}) {
  return (
    <article className="rounded-box border border-base-300 bg-base-100 p-4 shadow-sm">
      <div className="flex items-start justify-between gap-3">
        <h3 className="text-base font-semibold text-base-content">
          {recommendation.title}
        </h3>
        <span
          className={recommendationPriorityBadgeClass(recommendation.priority)}
        >
          {recommendation.priority}
        </span>
      </div>
      <p className="mt-2 text-sm leading-6 text-base-content/70">
        {recommendation.detail}
      </p>
    </article>
  );
}

function SessionTrendTooltip({
  active,
  payload,
}: {
  active?: boolean;
  payload?: Array<{ payload?: SessionChartPoint }>;
}) {
  if (!active || !payload?.length) {
    return null;
  }

  const point = payload[0]?.payload;
  if (!point) {
    return null;
  }

  return (
    <div className="rounded-box border border-base-300 bg-base-100 px-3 py-3 shadow-lg">
      <p className="text-sm font-semibold text-base-content">
        {point.activityTitle}
      </p>
      <div className="mt-2 space-y-1.5 text-sm text-base-content/75">
        <div className="flex items-center justify-between gap-4">
          <span>Efforts</span>
          <span className="font-medium text-base-content">
            {point.effortCount}
          </span>
        </div>
        <div className="flex items-center justify-between gap-4">
          <span>Avg fade</span>
          <span className="font-medium text-base-content">
            {formatPercent(point.averageRepeatFadePercent)}
          </span>
        </div>
      </div>
    </div>
  );
}

function EmptySessionsState() {
  return (
    <div className="flex h-full min-h-[240px] items-center justify-center rounded-box border border-dashed border-base-300 bg-base-200/60 px-6 text-center text-sm leading-6 text-base-content/70">
      Mark a few segments as DH and record repeat laps to unlock recent session
      shape and consistency tracking.
    </div>
  );
}

function SegmentBenchmarkCard({ segment }: { segment: DhSegmentProgress }) {
  return (
    <article className="rounded-box border border-base-300 bg-base-100 p-5 shadow-sm">
      <div className="flex items-start justify-between gap-3">
        <div>
          <Link
            href={`/segments/${segment.segment_id}`}
            className="text-lg font-semibold text-base-content transition hover:text-primary"
          >
            {segment.segment_title}
          </Link>
          <p className="mt-1 text-sm text-base-content/65">
            {segment.effort_count} recorded DH efforts
          </p>
        </div>
        <span className="badge badge-outline">#{segment.segment_id}</span>
      </div>

      <div className="mt-5 grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        <div>
          <p className="text-xs uppercase tracking-[0.18em] text-base-content/45">
            PR
          </p>
          <p className="mt-2 text-lg font-semibold text-base-content">
            {segment.personal_record_duration_seconds != null
              ? formatDuration(segment.personal_record_duration_seconds)
              : "--"}
          </p>
        </div>
        <div>
          <p className="text-xs uppercase tracking-[0.18em] text-base-content/45">
            Recent best
          </p>
          <p className="mt-2 text-lg font-semibold text-base-content">
            {segment.recent_best_duration_seconds != null
              ? formatDuration(segment.recent_best_duration_seconds)
              : "--"}
          </p>
        </div>
        <div>
          <p className="text-xs uppercase tracking-[0.18em] text-base-content/45">
            Rolling top 3
          </p>
          <p className="mt-2 text-lg font-semibold text-base-content">
            {segment.rolling_top_3_average_duration_seconds != null
              ? formatDuration(
                  Math.round(segment.rolling_top_3_average_duration_seconds),
                )
              : "--"}
          </p>
        </div>
        <div>
          <p className="text-xs uppercase tracking-[0.18em] text-base-content/45">
            Repeat fade
          </p>
          <p className="mt-2 text-lg font-semibold text-base-content">
            {formatPercent(segment.repeat_fade_percent)}
          </p>
        </div>
      </div>

      <div className="mt-4 flex flex-wrap items-center gap-3 text-sm text-base-content/70">
        <span>Top-3 gap {formatPercent(segment.top_3_pr_gap_percent)}</span>
        {segment.latest_activity_id && segment.latest_activity_title ? (
          <>
            <span className="text-base-content/35">•</span>
            <Link
              href={`/activities/${segment.latest_activity_id}`}
              className="link link-primary link-hover no-underline"
            >
              {segment.latest_activity_title}
            </Link>
            {segment.latest_activity_started_at ? (
              <span>
                {formatActivityTimestamp(segment.latest_activity_started_at)}
              </span>
            ) : null}
          </>
        ) : null}
      </div>
    </article>
  );
}

export default function DhGoalsProgressPanel() {
  const progressQuery = useDhGoalProgress();

  const sessionChartData = useMemo<SessionChartPoint[]>(() => {
    return (progressQuery.data?.recent_sessions ?? [])
      .slice()
      .reverse()
      .map((session) => ({
        label: formatShortDate(session.started_at),
        activityTitle: session.activity_title,
        effortCount: session.effort_count,
        averageRepeatFadePercent: session.average_repeat_fade_percent ?? null,
      }));
  }, [progressQuery.data?.recent_sessions]);

  if (progressQuery.isLoading) {
    return (
      <section className="space-y-6">
        <div className="rounded-box border border-base-300 bg-base-100 p-6 shadow-sm">
          <div className="skeleton h-5 w-24" />
          <div className="mt-3 skeleton h-10 w-80 max-w-full" />
          <div className="mt-3 skeleton h-5 w-full max-w-2xl" />
        </div>
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          {Array.from({ length: 4 }).map((_, index) => (
            <div
              key={index}
              className="rounded-box border border-base-300 bg-base-100 p-5 shadow-sm"
            >
              <div className="skeleton h-4 w-24" />
              <div className="mt-4 skeleton h-8 w-28" />
              <div className="mt-3 skeleton h-4 w-32" />
            </div>
          ))}
        </div>
      </section>
    );
  }

  if (!progressQuery.data) {
    return null;
  }

  const progress = progressQuery.data;
  const hasDhSegments = progress.summary.segment_count > 0;

  return (
    <section className="space-y-8">
      <div className="relative overflow-hidden rounded-[2rem] border border-orange-500/15 bg-gradient-to-br from-orange-500/10 via-base-100 to-sky-500/10 p-6 shadow-xl sm:p-8">
        <div className="absolute inset-y-0 right-0 hidden w-72 bg-[radial-gradient(circle_at_top_right,rgba(37,99,235,0.18),transparent_68%)] lg:block" />
        <div className="relative flex flex-col gap-6 lg:flex-row lg:items-end lg:justify-between">
          <div className="max-w-3xl">
            <p className="text-sm uppercase tracking-[0.24em] text-base-content/45">
              DH training
            </p>
            <h1 className="mt-3 text-4xl font-semibold tracking-tight text-base-content sm:text-5xl">
              DH goals & progress
            </h1>
            <p className="mt-4 max-w-2xl text-base leading-7 text-base-content/70 sm:text-lg">
              Track downhill laps, repeat-fade control, and segment consistency
              across the trails you explicitly tag as DH.
            </p>
          </div>

          <div className="flex flex-wrap items-center gap-2 text-sm text-base-content/65">
            <span className="badge badge-outline gap-2 px-3 py-3">
              {progress.summary.segment_count} DH segments
            </span>
            <span className="badge badge-outline gap-2 px-3 py-3">
              {progress.summary.session_count} recent sessions
            </span>
            <span className="badge badge-outline gap-2 px-3 py-3">
              Updated {formatActivityTimestamp(progress.generated_at)}
            </span>
            {progressQuery.isFetching ? (
              <LoadingSpinner size="sm" />
            ) : null}
          </div>
        </div>

        <div className="mt-6 grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          <SummaryStat
            label="Tagged segments"
            value={`${progress.summary.segment_count}`}
            detail={
              hasDhSegments
                ? "Segments currently included in DH analytics"
                : "No segments marked as DH yet"
            }
          />
          <SummaryStat
            label="Lap density"
            value={
              progress.summary.average_efforts_per_session != null
                ? progress.summary.average_efforts_per_session.toFixed(1)
                : "--"
            }
            detail="Average timed efforts per DH session"
          />
          <SummaryStat
            label="Repeat fade"
            value={formatPercent(progress.summary.average_repeat_fade_percent)}
            detail="Average first-to-last lap drift across recent sessions"
          />
          <SummaryStat
            label="Top-3 gap"
            value={formatPercent(progress.summary.average_top_3_gap_percent)}
            detail="Rolling consistency gap versus your PRs"
          />
        </div>
      </div>

      {!hasDhSegments ? (
        <section className="rounded-box border border-dashed border-base-300 bg-base-100 p-6 shadow-sm">
          <h2 className="text-xl font-semibold text-base-content">
            Start by tagging DH segments
          </h2>
          <p className="mt-2 max-w-2xl text-sm leading-6 text-base-content/70">
            This screen only tracks segments explicitly marked as DH. Open your
            key downhill trails in the segment editor, set the mode to DH, then
            record a few repeat laps to populate consistency and session trend
            data.
          </p>
        </section>
      ) : null}

      <section className="grid gap-4 xl:grid-cols-3">
        {progress.goals.map((goal) => (
          <GoalCard key={goal.key} goal={goal} />
        ))}
      </section>

      <section className="grid gap-6 xl:grid-cols-[minmax(0,1.15fr)_minmax(0,0.85fr)] xl:items-start">
        <article className="rounded-box border border-base-300 bg-base-100 p-5 shadow-sm sm:p-6">
          <div className="flex items-start justify-between gap-4">
            <div>
              <div className="flex items-center gap-2">
                <h2 className="text-xl font-semibold text-base-content">
                  Recent session shape
                </h2>
                <InfoTooltip
                  label="Recent session shape details"
                  tip={RECENT_SESSION_SHAPE_HELP_TEXT}
                />
              </div>
            </div>
          </div>

          <div className="mt-6 h-[320px]">
            {sessionChartData.length > 0 ? (
              <div
                className="h-full w-full"
                role="img"
                aria-label="DH recent sessions chart"
              >
                <ResponsiveContainer width="100%" height="100%">
                  <ComposedChart data={sessionChartData}>
                    <CartesianGrid strokeDasharray="3 3" strokeOpacity={0.2} />
                    <XAxis
                      dataKey="label"
                      tickLine={false}
                      axisLine={false}
                      minTickGap={24}
                    />
                    <YAxis
                      yAxisId="efforts"
                      allowDecimals={false}
                      tickLine={false}
                      axisLine={false}
                    />
                    <YAxis
                      yAxisId="fade"
                      orientation="right"
                      tickFormatter={(value) => `${value}%`}
                      tickLine={false}
                      axisLine={false}
                    />
                    <Tooltip content={<SessionTrendTooltip />} />
                    <ReferenceLine
                      yAxisId="fade"
                      y={5}
                      stroke={GOAL_LINE_COLOR}
                      strokeDasharray="4 4"
                    />
                    <Bar
                      yAxisId="efforts"
                      dataKey="effortCount"
                      fill={SESSION_BAR_COLOR}
                      radius={[8, 8, 0, 0]}
                      name="Efforts"
                    />
                    <Line
                      yAxisId="fade"
                      type="monotone"
                      dataKey="averageRepeatFadePercent"
                      stroke={FADE_LINE_COLOR}
                      strokeWidth={3}
                      dot={{ r: 3 }}
                      activeDot={{ r: 5 }}
                      name="Average fade"
                    />
                  </ComposedChart>
                </ResponsiveContainer>
              </div>
            ) : (
              <EmptySessionsState />
            )}
          </div>
        </article>

        <article className="rounded-box border border-base-300 bg-base-100 p-5 shadow-sm sm:p-6">
          <div className="flex items-start justify-between gap-4">
            <div>
              <h2 className="text-xl font-semibold text-base-content">
                Next ride guidance
              </h2>
              <p className="mt-1 text-sm text-base-content/70">
                Deterministic recommendations based on DH-tagged segment depth,
                repeat fade, and session density.
              </p>
            </div>
          </div>

          <div className="mt-5 space-y-4">
            {progress.recommendations.map((recommendation) => (
              <RecommendationCard
                key={recommendation.key}
                recommendation={recommendation}
              />
            ))}
          </div>
        </article>
      </section>

      <section className="space-y-4">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h2 className="text-xl font-semibold text-base-content">
              Segment benchmarks
            </h2>
            <p className="mt-1 text-sm text-base-content/70">
              Compare each DH segment’s PR, current top-3 window, and fade so
              you can see whether your pace is sharp and repeatable.
            </p>
          </div>
        </div>

        <div className="grid gap-4 xl:grid-cols-2">
          {progress.segments.map((segment) => (
            <SegmentBenchmarkCard key={segment.segment_id} segment={segment} />
          ))}
        </div>
      </section>

      <section className="rounded-box border border-base-300 bg-base-100 p-5 shadow-sm sm:p-6">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h2 className="text-xl font-semibold text-base-content">
              Recent sessions
            </h2>
            <p className="mt-1 text-sm text-base-content/70">
              Session-level rollups across your latest DH ride days.
            </p>
          </div>
        </div>

        <div
          className="mt-5 overflow-x-auto"
          aria-label="DH recent sessions table"
        >
          <table className="table table-zebra">
            <thead>
              <tr>
                <th>Activity</th>
                <th>Date</th>
                <th>Segments</th>
                <th>Efforts</th>
                <th>Fastest</th>
                <th>Avg fade</th>
              </tr>
            </thead>
            <tbody>
              {progress.recent_sessions.map((session) => (
                <tr key={session.activity_id}>
                  <td>
                    <Link
                      href={`/activities/${session.activity_id}`}
                      className="link link-primary link-hover no-underline"
                    >
                      {session.activity_title}
                    </Link>
                  </td>
                  <td>{formatActivityTimestamp(session.started_at)}</td>
                  <td>{session.segment_count}</td>
                  <td>{session.effort_count}</td>
                  <td>
                    {session.fastest_effort_duration_seconds != null
                      ? formatDuration(session.fastest_effort_duration_seconds)
                      : "--"}
                  </td>
                  <td>{formatPercent(session.average_repeat_fade_percent)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>
    </section>
  );
}
