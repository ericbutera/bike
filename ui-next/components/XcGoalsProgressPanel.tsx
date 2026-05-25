"use client";

import { auth } from "@ericbutera/kaleido";
import { useQueryClient } from "@tanstack/react-query";
import Link from "next/link";
import { useEffect, useMemo, useRef, useState } from "react";
import toast from "react-hot-toast";
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
import {
  extractApiMessage,
  formatActivityTimestamp,
  formatDistance,
  formatDuration,
  formatElevation,
  formatSpeed,
  normalizeUnitSystem,
  type UnitSystem,
} from "../lib/activityFormatting";
import {
  useActivityProcessingState,
  useUpdateUserPreferences,
  useUserPreferences,
  useXcGoalProgress,
  type ActivityRideFocus,
  type TrainingGoalMetric,
  type TrainingRecommendation,
  type UserPreferences,
} from "../lib/queries";
import { hasConfiguredHeartRateZoneBounds } from "../lib/trainingProfile";
import AuthRequiredCard from "./AuthRequiredCard";

const Z2_COLOR = "#0f766e";
const CLIMB_COLOR = "#ea580c";
const DECOUPLING_COLOR = "#2563eb";
const GOAL_LINE_COLOR = "#dc2626";
const METERS_PER_MILE = 1609.344;
const FEET_PER_METER = 3.28084;

type GoalDistanceUnit = "mi" | "km";
type GoalElevationUnit = "ft" | "m";

type WeeklyChartPoint = {
  label: string;
  z2Hours: number;
  climbingGain: number;
  comparableRideCount: number;
  averageZ2SpeedMps?: number | null;
  climbingVerticalRateMetersPerHour?: number | null;
  averageAerobicDecouplingPercent?: number | null;
};

type DecouplingChartPoint = {
  label: string;
  title: string;
  aerobicDecouplingPercent: number;
};

function formatShortDate(value: string) {
  const date = parseDisplayDate(value);
  const dateOnly = isDateOnlyValue(value);

  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return date.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    timeZone: dateOnly ? "UTC" : undefined,
  });
}

function formatLongDate(value: string) {
  const date = parseDisplayDate(value);
  const dateOnly = isDateOnlyValue(value);

  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return date.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
    timeZone: dateOnly ? "UTC" : undefined,
  });
}

function isDateOnlyValue(value: string) {
  return /^(\d{4})-(\d{2})-(\d{2})$/.test(value);
}

function parseDisplayDate(value: string) {
  const dateOnlyMatch = value.match(/^(\d{4})-(\d{2})-(\d{2})$/);

  if (dateOnlyMatch) {
    return new Date(
      Date.UTC(
        Number(dateOnlyMatch[1]),
        Number(dateOnlyMatch[2]) - 1,
        Number(dateOnlyMatch[3]),
      ),
    );
  }

  return new Date(value);
}

function formatCompactMetric(value: number | null | undefined, digits = 1) {
  if (value == null || !Number.isFinite(value)) {
    return "--";
  }

  return value.toFixed(digits);
}

function parseOptionalNumberInput(value: string) {
  const trimmed = value.trim();

  if (!trimmed) {
    return null;
  }

  const parsed = Number(trimmed);
  return Number.isFinite(parsed) ? parsed : null;
}

function formatGoalMetricValue(
  goal: TrainingGoalMetric,
  value: number | null | undefined,
  unitSystem: UnitSystem,
) {
  if (value == null || !Number.isFinite(value)) {
    return "--";
  }

  switch (goal.unit) {
    case "seconds":
      return formatDuration(Math.round(value));
    case "meters":
      return formatElevation(value, unitSystem);
    case "percent":
      return `${value.toFixed(1)}%`;
    case "count":
      return Number.isInteger(value) ? `${value}` : value.toFixed(1);
    default:
      return formatCompactMetric(value);
  }
}

function formatRideFocusLabel(focus: ActivityRideFocus) {
  switch (focus) {
    case "xc_endurance":
      return "XC endurance";
    case "mixed_xc":
      return "Mixed XC";
    case "dh_session":
      return "DH session";
    default:
      return "Other";
  }
}

function rideFocusBadgeClass(focus: ActivityRideFocus) {
  switch (focus) {
    case "xc_endurance":
      return "badge badge-success badge-outline font-medium";
    case "mixed_xc":
      return "badge badge-info badge-outline font-medium";
    case "dh_session":
      return "badge badge-warning badge-outline font-medium";
    default:
      return "badge badge-ghost font-medium";
  }
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

function formatRouteFamily(value: string | null | undefined) {
  if (!value) {
    return "Ad hoc route";
  }

  return value
    .split("-")
    .filter(Boolean)
    .map((token) => token.charAt(0).toUpperCase() + token.slice(1))
    .join(" ");
}

function formatGoalDistance(
  value: number | null | undefined,
  unit: GoalDistanceUnit,
) {
  if (value == null || !Number.isFinite(value)) {
    return "--";
  }

  if (unit === "mi") {
    const miles = value / METERS_PER_MILE;
    return `${miles >= 100 ? miles.toFixed(0) : miles.toFixed(1)} mi`;
  }

  const kilometers = value / 1000;
  return `${kilometers >= 100 ? kilometers.toFixed(0) : kilometers.toFixed(1)} km`;
}

function formatGoalElevation(
  value: number | null | undefined,
  unit: GoalElevationUnit,
) {
  if (value == null || !Number.isFinite(value)) {
    return "--";
  }

  if (unit === "ft") {
    return `${Math.round(value * FEET_PER_METER)} ft`;
  }

  return `${Math.round(value)} m`;
}

function distanceToMeters(value: number, unit: GoalDistanceUnit) {
  return unit === "mi" ? value * METERS_PER_MILE : value * 1000;
}

function metersToDistanceInput(
  value: number | null | undefined,
  unit: GoalDistanceUnit,
) {
  if (value == null || !Number.isFinite(value)) {
    return "";
  }

  const next = unit === "mi" ? value / METERS_PER_MILE : value / 1000;
  return next >= 100 ? next.toFixed(0) : next.toFixed(1);
}

function elevationToMeters(value: number, unit: GoalElevationUnit) {
  return unit === "ft" ? value / FEET_PER_METER : value;
}

function metersToElevationInput(
  value: number | null | undefined,
  unit: GoalElevationUnit,
) {
  if (value == null || !Number.isFinite(value)) {
    return "";
  }

  return unit === "ft"
    ? Math.round(value * FEET_PER_METER).toString()
    : Math.round(value).toString();
}

function formatDaysRemaining(daysRemaining: number) {
  if (daysRemaining < 0) {
    return `${Math.abs(daysRemaining)} days past target`;
  }

  if (daysRemaining === 0) {
    return "Race day";
  }

  if (daysRemaining === 1) {
    return "1 day left";
  }

  return `${daysRemaining} days left`;
}

function isXcBackfillPendingStatus(status: string | null | undefined) {
  return status === "queued" || status === "waiting" || status === "running";
}

function describeXcBackfillMessage(
  status: string | null | undefined,
  processingMessage: string | null | undefined,
  processingSource: string | null | undefined,
  processingSourceLabel: string | null | undefined,
) {
  switch (status) {
    case "queued":
      return "Historical XC rides are queued for backfill now. Charts will populate as soon as the training analysis rebuild runs.";
    case "waiting":
      return processingMessage
        ? `Historical XC backfill is waiting for the current activity processing job to finish. ${processingMessage}`
        : "Historical XC backfill is waiting for the current activity processing job to finish.";
    case "running":
      return processingSource && processingSource !== "xc_training_backfill"
        ? `Historical XC backfill is still marked running, but the active processing lock belongs to ${processingSourceLabel ?? "another activity job"}. This usually means the earlier processing state needs to clear before XC status will settle.`
        : "Historical XC backfill is rebuilding historical training metrics.";
    case "failed":
      return "Historical XC backfill failed. Save the goal again or queue the user-id XC backfill from admin analytics.";
    default:
      return null;
  }
}

function formatWeeklyDistancePace(
  value: number | null | undefined,
  unit: GoalDistanceUnit,
) {
  const formatted = formatGoalDistance(value, unit);
  return formatted === "--" ? formatted : `${formatted}/wk`;
}

function formatWeeklyElevationPace(
  value: number | null | undefined,
  unit: GoalElevationUnit,
) {
  const formatted = formatGoalElevation(value, unit);
  return formatted === "--" ? formatted : `${formatted}/wk`;
}

function calculateInclusiveDaySpan(startValue: string, endValue: string) {
  const start = parseDisplayDate(startValue);
  const end = parseDisplayDate(endValue);

  if (Number.isNaN(start.getTime()) || Number.isNaN(end.getTime())) {
    return null;
  }

  const diffDays = Math.floor((end.getTime() - start.getTime()) / 86_400_000);
  if (diffDays < 0) {
    return null;
  }

  return diffDays + 1;
}

function averageNumberArray(values: number[]) {
  if (values.length === 0) {
    return null;
  }

  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

function buildTrend(values: number[]) {
  if (values.length < 2) {
    return null;
  }

  const sampleSize = Math.min(3, values.length);
  const baseline = averageNumberArray(values.slice(0, sampleSize));
  const recent = averageNumberArray(values.slice(-sampleSize));
  if (baseline == null || recent == null) {
    return null;
  }

  return {
    baseline,
    recent,
    delta: recent - baseline,
    deltaPercent: baseline > 0 ? ((recent - baseline) / baseline) * 100 : null,
  };
}

function calculateVerticalRate(
  elevationGainMeters: number | null | undefined,
  climbingTimeSeconds: number | null | undefined,
) {
  if (
    elevationGainMeters == null ||
    climbingTimeSeconds == null ||
    climbingTimeSeconds <= 0 ||
    elevationGainMeters <= 0
  ) {
    return null;
  }

  return (elevationGainMeters / climbingTimeSeconds) * 3600;
}

function formatClimbRate(
  valueMetersPerHour: number | null | undefined,
  unit: GoalElevationUnit,
) {
  if (valueMetersPerHour == null || !Number.isFinite(valueMetersPerHour)) {
    return "--";
  }

  if (unit === "ft") {
    return `${Math.round(valueMetersPerHour * FEET_PER_METER)} ft/h`;
  }

  return `${Math.round(valueMetersPerHour)} m/h`;
}

function formatClimbDensity(
  elevationGainMeters: number | null | undefined,
  distanceMeters: number | null | undefined,
  distanceUnit: GoalDistanceUnit,
  elevationUnit: GoalElevationUnit,
) {
  if (
    elevationGainMeters == null ||
    distanceMeters == null ||
    !Number.isFinite(elevationGainMeters) ||
    !Number.isFinite(distanceMeters) ||
    distanceMeters <= 0
  ) {
    return "--";
  }

  if (distanceUnit === "mi" || elevationUnit === "ft") {
    return `${Math.round((elevationGainMeters * FEET_PER_METER) / (distanceMeters / METERS_PER_MILE))} ft/mi`;
  }

  return `${Math.round(elevationGainMeters / (distanceMeters / 1000))} m/km`;
}

function buildPreferencesPayload(
  currentPreferences: UserPreferences | null,
  overrides: {
    xcGoalStartDate: string | null;
    xcGoalTargetDate: string | null;
    xcGoalTargetDistanceMeters: number | null;
    xcGoalTargetElevationGainMeters: number | null;
  },
): UserPreferences {
  return {
    unit_system: currentPreferences?.unit_system ?? "mixed",
    estimated_ftp_watts: currentPreferences?.estimated_ftp_watts ?? null,
    heart_rate_zone_bounds_bpm:
      currentPreferences?.heart_rate_zone_bounds_bpm ?? null,
    xc_goal_start_date: overrides.xcGoalStartDate,
    xc_goal_target_date: overrides.xcGoalTargetDate,
    xc_goal_target_distance_meters: overrides.xcGoalTargetDistanceMeters,
    xc_goal_target_elevation_gain_meters:
      overrides.xcGoalTargetElevationGainMeters,
  };
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

function GoalCard({
  goal,
  unitSystem,
}: {
  goal: TrainingGoalMetric;
  unitSystem: UnitSystem;
}) {
  const currentValue = formatGoalMetricValue(
    goal,
    goal.current_value,
    unitSystem,
  );
  const targetValue = formatGoalMetricValue(
    goal,
    goal.target_value,
    unitSystem,
  );
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
            {currentValue}
          </p>
        </div>
        <div className="text-right">
          <p className="text-sm text-base-content/60">Target</p>
          <p className="text-lg font-medium text-base-content">{targetValue}</p>
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

function WeeklyTrendTooltip({
  active,
  payload,
  unitSystem,
  goalElevationUnit,
}: {
  active?: boolean;
  payload?: Array<{ payload?: WeeklyChartPoint }>;
  unitSystem: UnitSystem;
  goalElevationUnit: GoalElevationUnit;
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
      <p className="text-sm font-semibold text-base-content">{point.label}</p>
      <div className="mt-2 space-y-1.5 text-sm text-base-content/75">
        <div className="flex items-center justify-between gap-4">
          <span>Z2</span>
          <span className="font-medium text-base-content">
            {point.z2Hours.toFixed(1)} h
          </span>
        </div>
        <div className="flex items-center justify-between gap-4">
          <span>Climbing</span>
          <span className="font-medium text-base-content">
            {Math.round(point.climbingGain)} m
          </span>
        </div>
        <div className="flex items-center justify-between gap-4">
          <span>Comparable rides</span>
          <span className="font-medium text-base-content">
            {point.comparableRideCount}
          </span>
        </div>
        {point.averageZ2SpeedMps != null ? (
          <div className="flex items-center justify-between gap-4">
            <span>Z2 speed</span>
            <span className="font-medium text-base-content">
              {formatSpeed(point.averageZ2SpeedMps, unitSystem)}
            </span>
          </div>
        ) : null}
        {point.climbingVerticalRateMetersPerHour != null ? (
          <div className="flex items-center justify-between gap-4">
            <span>Climb pace</span>
            <span className="font-medium text-base-content">
              {formatClimbRate(
                point.climbingVerticalRateMetersPerHour,
                goalElevationUnit,
              )}
            </span>
          </div>
        ) : null}
      </div>
    </div>
  );
}

function DecouplingTooltip({
  active,
  payload,
}: {
  active?: boolean;
  payload?: Array<{ payload?: DecouplingChartPoint }>;
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
      <p className="text-sm font-semibold text-base-content">{point.title}</p>
      <div className="mt-2 flex items-center justify-between gap-4 text-sm text-base-content/75">
        <span>Decoupling</span>
        <span className="font-medium text-base-content">
          {point.aerobicDecouplingPercent.toFixed(1)}%
        </span>
      </div>
    </div>
  );
}

function EmptyComparableState() {
  return (
    <div className="flex h-full min-h-[220px] items-center justify-center rounded-box border border-dashed border-base-300 bg-base-200/60 px-6 text-center text-sm leading-6 text-base-content/70">
      Repeat a comparable endurance route with heart-rate data to unlock the
      decoupling trend line.
    </div>
  );
}

export default function XcGoalsProgressPanel() {
  const authApi = auth.useAuthApi();
  const { user, isLoading: isLoadingUser } = authApi.useCurrentUser();
  const queryClient = useQueryClient();
  const preferencesQuery = useUserPreferences({
    enabled: !!user,
    refetchIntervalMs: user ? 5000 : false,
  });
  const progressQuery = useXcGoalProgress({ enabled: !!user });
  const processingStateQuery = useActivityProcessingState({
    enabled: !!user,
    refetchIntervalMs: user ? 5000 : false,
  });
  const updatePreferencesMutation = useUpdateUserPreferences();
  const unitSystem = normalizeUnitSystem(preferencesQuery.data?.unit_system);
  const heartRateZonesConfigured = hasConfiguredHeartRateZoneBounds(
    preferencesQuery.data?.heart_rate_zone_bounds_bpm,
  );
  const backfillStatus = preferencesQuery.data?.xc_goal_backfill_status ?? null;
  const backfillCompletedAt =
    preferencesQuery.data?.xc_goal_backfill_completed_at ?? null;
  const previousBackfillStatusRef = useRef<string | null>(null);
  const [goalStartDateDraft, setGoalStartDateDraft] = useState("");
  const [goalDateDraft, setGoalDateDraft] = useState("");
  const [goalDistanceDraft, setGoalDistanceDraft] = useState("");
  const [goalDistanceUnit, setGoalDistanceUnit] =
    useState<GoalDistanceUnit>("mi");
  const [goalElevationDraft, setGoalElevationDraft] = useState("");
  const [goalElevationUnit, setGoalElevationUnit] =
    useState<GoalElevationUnit>("ft");

  useEffect(() => {
    const nextGoalStartDateDraft =
      preferencesQuery.data?.xc_goal_start_date ?? "";
    const nextGoalDateDraft = preferencesQuery.data?.xc_goal_target_date ?? "";
    const nextGoalDistanceDraft = metersToDistanceInput(
      preferencesQuery.data?.xc_goal_target_distance_meters,
      goalDistanceUnit,
    );
    const nextGoalElevationDraft = metersToElevationInput(
      preferencesQuery.data?.xc_goal_target_elevation_gain_meters,
      goalElevationUnit,
    );

    setGoalStartDateDraft((currentValue) =>
      currentValue === nextGoalStartDateDraft
        ? currentValue
        : nextGoalStartDateDraft,
    );
    setGoalDateDraft((currentValue) =>
      currentValue === nextGoalDateDraft ? currentValue : nextGoalDateDraft,
    );
    setGoalDistanceDraft((currentValue) =>
      currentValue === nextGoalDistanceDraft
        ? currentValue
        : nextGoalDistanceDraft,
    );
    setGoalElevationDraft((currentValue) =>
      currentValue === nextGoalElevationDraft
        ? currentValue
        : nextGoalElevationDraft,
    );
  }, [
    goalDistanceUnit,
    goalElevationUnit,
    preferencesQuery.data?.xc_goal_start_date,
    preferencesQuery.data?.xc_goal_target_date,
    preferencesQuery.data?.xc_goal_target_distance_meters,
    preferencesQuery.data?.xc_goal_target_elevation_gain_meters,
  ]);

  useEffect(() => {
    const previousStatus = previousBackfillStatusRef.current;

    if (
      previousStatus &&
      previousStatus !== "completed" &&
      backfillStatus === "completed"
    ) {
      toast.success(
        "Historical XC backfill finished. Refreshing training charts.",
      );
      void Promise.all([
        queryClient.invalidateQueries({ queryKey: ["get", "/preferences"] }),
        queryClient.invalidateQueries({
          queryKey: ["get", "/training/xc-progress"],
        }),
        queryClient.invalidateQueries({
          queryKey: ["get", "/activity-imports/processing-state"],
        }),
      ]);
    }

    previousBackfillStatusRef.current = backfillStatus;
  }, [backfillStatus, queryClient]);

  const weeklyChartData = useMemo<WeeklyChartPoint[]>(() => {
    return (progressQuery.data?.weekly_progress ?? []).map((point) => ({
      label: formatShortDate(point.week_start),
      z2Hours: point.z2_time_seconds / 3600,
      climbingGain: point.climbing_elevation_gain_meters,
      comparableRideCount: point.comparable_ride_count,
      averageZ2SpeedMps: point.average_z2_speed_mps,
      climbingVerticalRateMetersPerHour:
        point.climbing_vertical_rate_meters_per_hour,
      averageAerobicDecouplingPercent: point.average_aerobic_decoupling_percent,
    }));
  }, [progressQuery.data?.weekly_progress]);

  const decouplingChartData = useMemo<DecouplingChartPoint[]>(() => {
    const eventGoal = progressQuery.data?.event_goal ?? null;

    if (eventGoal) {
      return (progressQuery.data?.weekly_progress ?? [])
        .filter((point) => point.average_aerobic_decoupling_percent != null)
        .map((point) => ({
          label: formatShortDate(point.week_start),
          title: `Week of ${formatLongDate(point.week_start)}`,
          aerobicDecouplingPercent:
            point.average_aerobic_decoupling_percent ?? 0,
        }));
    }

    return (progressQuery.data?.recent_rides ?? [])
      .filter((ride) => ride.aerobic_decoupling_percent != null)
      .slice()
      .reverse()
      .map((ride) => ({
        label: formatShortDate(ride.started_at),
        title: ride.activity_title,
        aerobicDecouplingPercent: ride.aerobic_decoupling_percent ?? 0,
      }));
  }, [
    progressQuery.data?.event_goal,
    progressQuery.data?.recent_rides,
    progressQuery.data?.weekly_progress,
  ]);

  const onTrackMetrics = useMemo(() => {
    const progress = progressQuery.data;
    const eventGoal = progress?.event_goal ?? null;

    if (!progress || !eventGoal) {
      return null;
    }

    const progressEndValue =
      eventGoal.days_remaining < 0
        ? eventGoal.target_date
        : progress.generated_at;
    const elapsedDays =
      calculateInclusiveDaySpan(eventGoal.start_date, progressEndValue) ?? 1;
    const totalWeeks = Math.max(eventGoal.training_window_days / 7, 1 / 7);
    const elapsedWeeks = Math.max(elapsedDays / 7, 1 / 7);
    const remainingDays = Math.max(eventGoal.days_remaining + 1, 0);
    const remainingWeeks =
      remainingDays > 0 ? Math.max(remainingDays / 7, 1 / 7) : 0;
    const remainingDistanceMeters = Math.max(
      eventGoal.target_distance_meters - eventGoal.counted_distance_meters,
      0,
    );
    const remainingElevationGainMeters = Math.max(
      eventGoal.target_elevation_gain_meters -
        eventGoal.counted_elevation_gain_meters,
      0,
    );
    const currentWeeklyDistanceMeters =
      eventGoal.counted_distance_meters / elapsedWeeks;
    const currentWeeklyElevationGainMeters =
      eventGoal.counted_elevation_gain_meters / elapsedWeeks;
    const targetWeeklyDistanceMeters =
      eventGoal.target_distance_meters / totalWeeks;
    const targetWeeklyElevationGainMeters =
      eventGoal.target_elevation_gain_meters / totalWeeks;
    const neededWeeklyDistanceMeters =
      remainingWeeks > 0 ? remainingDistanceMeters / remainingWeeks : 0;
    const neededWeeklyElevationGainMeters =
      remainingWeeks > 0 ? remainingElevationGainMeters / remainingWeeks : 0;
    const speedTrend = buildTrend(
      (progress.weekly_progress ?? [])
        .map((point) => point.average_z2_speed_mps)
        .filter(
          (value): value is number => value != null && Number.isFinite(value),
        ),
    );
    const decouplingTrend = buildTrend(
      (progress.weekly_progress ?? [])
        .map((point) => point.average_aerobic_decoupling_percent)
        .filter(
          (value): value is number => value != null && Number.isFinite(value),
        ),
    );
    const currentClimbDensity = formatClimbDensity(
      eventGoal.counted_elevation_gain_meters,
      eventGoal.counted_distance_meters,
      goalDistanceUnit,
      goalElevationUnit,
    );
    const targetClimbDensity = formatClimbDensity(
      eventGoal.target_elevation_gain_meters,
      eventGoal.target_distance_meters,
      goalDistanceUnit,
      goalElevationUnit,
    );

    return {
      distancePace: {
        label: "Distance pace",
        value: `${formatWeeklyDistancePace(currentWeeklyDistanceMeters, goalDistanceUnit)} / ${formatWeeklyDistancePace(targetWeeklyDistanceMeters, goalDistanceUnit)}`,
        detail:
          remainingDistanceMeters > 0 && remainingWeeks > 0
            ? `Current avg/week vs ${formatWeeklyDistancePace(neededWeeklyDistanceMeters, goalDistanceUnit)} needed from today`
            : "Distance target is already covered inside this block",
      },
      climbingPace: {
        label: "Climbing pace",
        value: `${formatWeeklyElevationPace(currentWeeklyElevationGainMeters, goalElevationUnit)} / ${formatWeeklyElevationPace(targetWeeklyElevationGainMeters, goalElevationUnit)}`,
        detail:
          remainingElevationGainMeters > 0 && remainingWeeks > 0
            ? `${currentClimbDensity} now vs ${targetClimbDensity} goal density; ${formatWeeklyElevationPace(neededWeeklyElevationGainMeters, goalElevationUnit)} needed from today`
            : `Current density ${currentClimbDensity} vs ${targetClimbDensity} goal density`,
      },
      z2Speed: {
        label: "Z2 speed trend",
        value: speedTrend ? formatSpeed(speedTrend.recent, unitSystem) : "--",
        detail: speedTrend
          ? `${speedTrend.delta >= 0 ? "+" : ""}${speedTrend.deltaPercent?.toFixed(1) ?? speedTrend.delta.toFixed(1)}${speedTrend.deltaPercent != null ? "%" : ""} vs opening block weeks`
          : heartRateZonesConfigured
            ? "Need at least two Z2 weeks with qualifying heart-rate samples in the block"
            : "Set heart rate zones on Account, then regenerate older rides to persist Z2 snapshots",
      },
      decoupling: {
        label: "Decoupling trend",
        value: decouplingTrend ? `${decouplingTrend.recent.toFixed(1)}%` : "--",
        detail: decouplingTrend
          ? `${decouplingTrend.delta <= 0 ? "Down" : "Up"} ${Math.abs(decouplingTrend.delta).toFixed(1)} pts vs opening block weeks`
          : heartRateZonesConfigured
            ? "Need comparable endurance repeats with enough Z2 time in the same route family"
            : "Set heart rate zones on Account, then regenerate older rides to compute decoupling",
      },
      currentClimbDensity,
      targetClimbDensity,
    };
  }, [
    goalDistanceUnit,
    goalElevationUnit,
    heartRateZonesConfigured,
    progressQuery.data,
    unitSystem,
  ]);

  if (isLoadingUser) {
    return (
      <section className="rounded-box border border-base-300 bg-base-100 shadow-sm">
        <div className="flex items-center justify-center py-16">
          <span className="loading loading-spinner loading-lg" />
        </div>
      </section>
    );
  }

  if (!user) {
    return (
      <AuthRequiredCard
        eyebrow="XC training"
        title="XC goals & progress"
        description="Sign in to view your endurance volume, climbing durability, and comparable-ride decoupling trends."
        loginLabel="Sign in to view XC progress"
        showSignup
      />
    );
  }

  if (progressQuery.isError) {
    return (
      <section className="space-y-4">
        <div className="rounded-box border border-error/30 bg-error/10 p-6 shadow-sm">
          <h2 className="text-xl font-semibold text-base-content">
            XC goals & progress
          </h2>
          <p className="mt-2 text-sm text-base-content/75">
            {extractApiMessage(progressQuery.error)}
          </p>
        </div>
      </section>
    );
  }

  if (progressQuery.isLoading || !progressQuery.data) {
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

  const progress = progressQuery.data;
  const eventGoal = progress.event_goal ?? null;
  const backfillMessage = describeXcBackfillMessage(
    backfillStatus,
    processingStateQuery.data?.message,
    processingStateQuery.data?.source,
    processingStateQuery.data?.source_label,
  );

  async function handleSaveGoal() {
    const parsedDistance = parseOptionalNumberInput(goalDistanceDraft);
    const parsedElevation = parseOptionalNumberInput(goalElevationDraft);

    if (
      !goalStartDateDraft.trim() &&
      !goalDateDraft.trim() &&
      parsedDistance == null &&
      parsedElevation == null
    ) {
      await handleClearGoal();
      return;
    }

    if (
      !goalStartDateDraft.trim() ||
      !goalDateDraft.trim() ||
      parsedDistance == null ||
      parsedElevation == null
    ) {
      toast.error(
        "Enter a training start date, target date, distance target, and climbing target.",
      );
      return;
    }

    if (goalStartDateDraft.trim() > goalDateDraft.trim()) {
      toast.error("Training start date must be on or before the target date.");
      return;
    }

    if (parsedDistance <= 0 || parsedElevation <= 0) {
      toast.error(
        "Goal distance and climbing targets must be greater than zero.",
      );
      return;
    }

    try {
      const updatedPreferences = await updatePreferencesMutation.updateAsync(
        buildPreferencesPayload(preferencesQuery.data ?? null, {
          xcGoalStartDate: goalStartDateDraft.trim(),
          xcGoalTargetDate: goalDateDraft.trim(),
          xcGoalTargetDistanceMeters: distanceToMeters(
            parsedDistance,
            goalDistanceUnit,
          ),
          xcGoalTargetElevationGainMeters: elevationToMeters(
            parsedElevation,
            goalElevationUnit,
          ),
        }),
      );
      if (
        isXcBackfillPendingStatus(updatedPreferences.xc_goal_backfill_status)
      ) {
        toast.success(
          updatedPreferences.xc_goal_backfill_status === "waiting"
            ? "XC event goal saved. Historical XC backfill will start after the current processing job finishes."
            : "XC event goal saved. Historical XC backfill queued.",
        );
      } else {
        toast.success("XC event goal saved.");
      }
    } catch (error) {
      toast.error(extractApiMessage(error));
    }
  }

  async function handleClearGoal() {
    try {
      await updatePreferencesMutation.updateAsync(
        buildPreferencesPayload(preferencesQuery.data ?? null, {
          xcGoalStartDate: null,
          xcGoalTargetDate: null,
          xcGoalTargetDistanceMeters: null,
          xcGoalTargetElevationGainMeters: null,
        }),
      );
      setGoalStartDateDraft("");
      setGoalDateDraft("");
      setGoalDistanceDraft("");
      setGoalElevationDraft("");
      toast.success("XC event goal cleared.");
    } catch (error) {
      toast.error(extractApiMessage(error));
    }
  }

  return (
    <section className="space-y-8">
      <div className="relative overflow-hidden rounded-[2rem] border border-emerald-500/15 bg-gradient-to-br from-emerald-500/10 via-base-100 to-amber-500/10 p-6 shadow-xl sm:p-8">
        <div className="absolute inset-y-0 right-0 hidden w-72 bg-[radial-gradient(circle_at_top_right,rgba(13,148,136,0.18),transparent_68%)] lg:block" />
        <div className="relative flex flex-col gap-6 lg:flex-row lg:items-end lg:justify-between">
          <div className="max-w-3xl">
            <p className="text-sm uppercase tracking-[0.24em] text-base-content/45">
              XC training
            </p>
            <h1 className="mt-3 text-4xl font-semibold tracking-tight text-base-content sm:text-5xl">
              XC goals & progress
            </h1>
            <p className="mt-4 max-w-2xl text-base leading-7 text-base-content/70 sm:text-lg">
              Track aerobic durability, weekly endurance load, climbing work,
              and training-block progress against your target event demands.
            </p>
          </div>

          <div className="flex flex-wrap items-center gap-2 text-sm text-base-content/65">
            {eventGoal ? (
              <>
                <span className="badge badge-outline gap-2 px-3 py-3">
                  Started {formatLongDate(eventGoal.start_date)}
                </span>
                <span className="badge badge-outline gap-2 px-3 py-3">
                  Target {formatLongDate(eventGoal.target_date)}
                </span>
                <span className="badge badge-outline gap-2 px-3 py-3">
                  {formatDaysRemaining(eventGoal.days_remaining)}
                </span>
              </>
            ) : (
              <span className="badge badge-outline gap-2 px-3 py-3">
                Set your September target
              </span>
            )}
            <span className="badge badge-outline gap-2 px-3 py-3">
              Updated {formatActivityTimestamp(progress.generated_at)}
            </span>
            {progressQuery.isFetching ? (
              <span className="loading loading-spinner loading-sm" />
            ) : null}
          </div>
        </div>

        {!heartRateZonesConfigured ? (
          <div className="relative mt-6 rounded-box border border-warning/30 bg-warning/10 p-4 text-sm leading-6 text-base-content/85">
            Heart rate zones are required for Z2 speed, aerobic decoupling, and
            Z2-based weekly endurance load. Save them on{" "}
            <Link href="/account" className="link link-primary link-hover">
              Account
            </Link>
            , then regenerate older rides so Bike can persist the per-ride zone
            snapshots those XC metrics need.
          </div>
        ) : null}

        <div className="mt-6 grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          {eventGoal ? (
            <>
              <SummaryStat
                label={onTrackMetrics?.distancePace.label ?? "Distance pace"}
                value={onTrackMetrics?.distancePace.value ?? "--"}
                detail={onTrackMetrics?.distancePace.detail ?? "--"}
              />
              <SummaryStat
                label={onTrackMetrics?.climbingPace.label ?? "Climbing pace"}
                value={onTrackMetrics?.climbingPace.value ?? "--"}
                detail={onTrackMetrics?.climbingPace.detail ?? "--"}
              />
              <SummaryStat
                label={onTrackMetrics?.z2Speed.label ?? "Z2 speed trend"}
                value={onTrackMetrics?.z2Speed.value ?? "--"}
                detail={onTrackMetrics?.z2Speed.detail ?? "--"}
              />
              <SummaryStat
                label={onTrackMetrics?.decoupling.label ?? "Decoupling trend"}
                value={onTrackMetrics?.decoupling.value ?? "--"}
                detail={onTrackMetrics?.decoupling.detail ?? "--"}
              />
            </>
          ) : (
            <>
              <SummaryStat
                label="Recent rides"
                value={`${progress.summary.recent_ride_count}`}
                detail="Endurance-focused rides in the current tracking window"
              />
              <SummaryStat
                label="Comparable rides"
                value={`${progress.summary.comparable_ride_count}`}
                detail="Repeatable rides with enough similarity for durability comparisons"
              />
              <SummaryStat
                label="Z2 volume"
                value={formatDuration(progress.summary.total_z2_time_seconds)}
                detail="Accumulated aerobic work in the recent window"
              />
              <SummaryStat
                label="Avg decoupling"
                value={
                  progress.summary.average_aerobic_decoupling_percent != null
                    ? `${progress.summary.average_aerobic_decoupling_percent.toFixed(1)}%`
                    : "--"
                }
                detail="Average first-half vs second-half efficiency drift"
              />
            </>
          )}
        </div>
      </div>

      <section className="rounded-box border border-base-300 bg-base-100 p-5 shadow-sm sm:p-6">
        <div className="grid gap-6 xl:grid-cols-[minmax(0,1.2fr)_minmax(0,0.8fr)] xl:items-start">
          <div>
            <div className="flex flex-wrap items-start justify-between gap-3">
              <div>
                <h2 className="text-xl font-semibold text-base-content">
                  Event target
                </h2>
                <p className="mt-1 text-sm text-base-content/70">
                  Store your XC training start date, target date, distance, and
                  climbing demand so this screen can count every qualifying XC
                  ride in that training block while keeping best-ride benchmarks
                  in view.
                </p>
              </div>
              {eventGoal ? (
                <span className="badge badge-success badge-outline gap-2 px-3 py-2">
                  {formatGoalDistance(
                    eventGoal.target_distance_meters,
                    goalDistanceUnit,
                  )}{" "}
                  ·{" "}
                  {formatGoalElevation(
                    eventGoal.target_elevation_gain_meters,
                    goalElevationUnit,
                  )}
                </span>
              ) : null}
            </div>

            {backfillMessage ? (
              <div
                className={`alert mt-4 text-sm ${
                  backfillStatus === "failed"
                    ? "alert-error"
                    : backfillStatus === "completed"
                      ? "alert-success"
                      : "alert-info"
                }`}
              >
                <span>{backfillMessage}</span>
              </div>
            ) : null}

            {backfillStatus === "completed" && backfillCompletedAt ? (
              <div className="alert alert-success mt-4 text-sm">
                <span>
                  Historical XC backfill completed{" "}
                  {formatActivityTimestamp(backfillCompletedAt)}.
                </span>
              </div>
            ) : null}

            <div className="mt-5 grid gap-4 xl:grid-cols-[minmax(0,0.8fr)_minmax(0,0.8fr)_minmax(0,1fr)_minmax(0,1fr)]">
              <label className="form-control gap-2">
                <span className="label-text font-medium">Training start</span>
                <input
                  type="date"
                  className="input input-bordered"
                  value={goalStartDateDraft}
                  onChange={(event) =>
                    setGoalStartDateDraft(event.target.value)
                  }
                />
              </label>

              <label className="form-control gap-2">
                <span className="label-text font-medium">Target date</span>
                <input
                  type="date"
                  className="input input-bordered"
                  value={goalDateDraft}
                  onChange={(event) => setGoalDateDraft(event.target.value)}
                />
              </label>

              <label className="form-control gap-2">
                <span className="label-text font-medium">Distance target</span>
                <div className="join w-full">
                  <input
                    type="number"
                    min="0"
                    step="0.1"
                    className="input input-bordered join-item w-full"
                    value={goalDistanceDraft}
                    onChange={(event) =>
                      setGoalDistanceDraft(event.target.value)
                    }
                    placeholder="100"
                  />
                  <select
                    className="select select-bordered join-item w-24"
                    value={goalDistanceUnit}
                    onChange={(event) =>
                      setGoalDistanceUnit(
                        event.target.value as GoalDistanceUnit,
                      )
                    }
                  >
                    <option value="mi">mi</option>
                    <option value="km">km</option>
                  </select>
                </div>
              </label>

              <label className="form-control gap-2">
                <span className="label-text font-medium">Climbing target</span>
                <div className="join w-full">
                  <input
                    type="number"
                    min="0"
                    step="1"
                    className="input input-bordered join-item w-full"
                    value={goalElevationDraft}
                    onChange={(event) =>
                      setGoalElevationDraft(event.target.value)
                    }
                    placeholder="13000"
                  />
                  <select
                    className="select select-bordered join-item w-24"
                    value={goalElevationUnit}
                    onChange={(event) =>
                      setGoalElevationUnit(
                        event.target.value as GoalElevationUnit,
                      )
                    }
                  >
                    <option value="ft">ft</option>
                    <option value="m">m</option>
                  </select>
                </div>
              </label>
            </div>

            <div className="mt-5 flex flex-wrap items-center gap-3">
              <button
                type="button"
                className="btn btn-primary"
                disabled={updatePreferencesMutation.isPending}
                onClick={handleSaveGoal}
              >
                {updatePreferencesMutation.isPending
                  ? "Saving target..."
                  : "Save XC target"}
              </button>
              <button
                type="button"
                className="btn btn-outline"
                disabled={updatePreferencesMutation.isPending}
                onClick={handleClearGoal}
              >
                Clear target
              </button>
            </div>
          </div>

          <div className="rounded-box border border-base-300 bg-base-200/60 p-4">
            <p className="text-xs uppercase tracking-[0.2em] text-base-content/45">
              Current read on the target
            </p>
            {eventGoal ? (
              <div className="mt-3 space-y-4">
                <p className="text-sm leading-6 text-base-content/70">
                  Counted {eventGoal.counted_ride_count} XC rides from{" "}
                  {formatLongDate(eventGoal.start_date)} through this current
                  snapshot of the training block.
                </p>
                <p className="text-sm leading-6 text-base-content/60">
                  Planned block length: {eventGoal.training_window_days} days.
                </p>
                {onTrackMetrics ? (
                  <p className="text-sm leading-6 text-base-content/60">
                    Current climb density {onTrackMetrics.currentClimbDensity}{" "}
                    vs {onTrackMetrics.targetClimbDensity} goal density.
                  </p>
                ) : null}
                <div>
                  <div className="flex items-center justify-between gap-3 text-sm text-base-content/65">
                    <span>Distance progress</span>
                    <span>
                      {`${eventGoal.counted_distance_progress_percent.toFixed(0)}%`}
                    </span>
                  </div>
                  <progress
                    className={goalProgressClass(
                      eventGoal.counted_distance_progress_percent,
                    )}
                    value={eventGoal.counted_distance_progress_percent}
                    max={100}
                  />
                  {eventGoal.best_distance_activity ? (
                    <p className="mt-2 text-sm text-base-content/70">
                      Best single distance ride in block:{" "}
                      <Link
                        href={`/activities/${eventGoal.best_distance_activity.activity_id}`}
                        className="link link-primary link-hover no-underline"
                      >
                        {eventGoal.best_distance_activity.activity_title}
                      </Link>
                    </p>
                  ) : null}
                </div>

                <div>
                  <div className="flex items-center justify-between gap-3 text-sm text-base-content/65">
                    <span>Climbing progress</span>
                    <span>
                      {`${eventGoal.counted_elevation_gain_progress_percent.toFixed(0)}%`}
                    </span>
                  </div>
                  <progress
                    className={goalProgressClass(
                      eventGoal.counted_elevation_gain_progress_percent,
                    )}
                    value={eventGoal.counted_elevation_gain_progress_percent}
                    max={100}
                  />
                  {eventGoal.best_elevation_activity ? (
                    <p className="mt-2 text-sm text-base-content/70">
                      Best single climbing ride in block:{" "}
                      <Link
                        href={`/activities/${eventGoal.best_elevation_activity.activity_id}`}
                        className="link link-primary link-hover no-underline"
                      >
                        {eventGoal.best_elevation_activity.activity_title}
                      </Link>
                    </p>
                  ) : null}
                </div>
              </div>
            ) : (
              <p className="mt-3 text-sm leading-6 text-base-content/70">
                Add a training start date plus target date, distance, and
                climbing. For Marji, that could be a June start with Sept 20,
                100 miles, and 13,000 feet.
              </p>
            )}
          </div>
        </div>
      </section>

      {!eventGoal ? (
        <div className="grid gap-4 xl:grid-cols-3">
          {progress.goals.map((goal) => (
            <GoalCard key={goal.key} goal={goal} unitSystem={unitSystem} />
          ))}
        </div>
      ) : null}

      <div className="grid gap-6 xl:grid-cols-[minmax(0,1.5fr)_minmax(0,0.9fr)]">
        <section className="rounded-box border border-base-300 bg-base-100 p-5 shadow-sm sm:p-6">
          <div className="mb-5 flex flex-wrap items-start justify-between gap-3">
            <div>
              <h2 className="text-xl font-semibold text-base-content">
                Weekly endurance load
              </h2>
              <p className="mt-1 text-sm text-base-content/70">
                {eventGoal
                  ? `Z2 hours and climbing gain across the current training block, starting ${formatLongDate(eventGoal.start_date)}. Hover to inspect Z2 speed and vertical rate week by week.`
                  : `Z2 hours and climbing gain over the last eight weeks. The summary cards above use a ${progress.summary.recent_window_days}-day XC snapshot.`}
              </p>
            </div>
            <div className="flex flex-wrap gap-2 text-xs text-base-content/70">
              <span className="badge badge-outline gap-2 px-3 py-2">
                <span
                  className="inline-block h-2.5 w-2.5 rounded-full"
                  style={{ backgroundColor: Z2_COLOR }}
                />
                Z2 hours
              </span>
              <span className="badge badge-outline gap-2 px-3 py-2">
                <span
                  className="inline-block h-2.5 w-2.5 rounded-full"
                  style={{ backgroundColor: CLIMB_COLOR }}
                />
                Climbing
              </span>
            </div>
          </div>

          <div
            role="img"
            aria-label="XC weekly progression chart"
            className="h-[320px] w-full"
          >
            <ResponsiveContainer
              width="100%"
              height="100%"
              minWidth={320}
              minHeight={320}
            >
              <ComposedChart
                data={weeklyChartData}
                margin={{ top: 8, right: 12, bottom: 8, left: 0 }}
              >
                <CartesianGrid
                  vertical={false}
                  stroke="var(--color-base-content)"
                  strokeOpacity={0.1}
                />
                <XAxis
                  axisLine={false}
                  dataKey="label"
                  tick={{ fill: "var(--color-base-content)", fontSize: 10 }}
                  tickLine={false}
                  minTickGap={20}
                />
                <YAxis
                  axisLine={false}
                  tick={{ fill: "var(--color-base-content)", fontSize: 10 }}
                  tickLine={false}
                  width={46}
                  tickFormatter={(value: number) => `${value.toFixed(1)}h`}
                />
                <YAxis
                  yAxisId="climbing"
                  orientation="right"
                  axisLine={false}
                  tick={{ fill: "var(--color-base-content)", fontSize: 10 }}
                  tickLine={false}
                  width={64}
                  tickFormatter={(value: number) =>
                    formatElevation(value, unitSystem)
                  }
                />
                <Tooltip
                  content={
                    <WeeklyTrendTooltip
                      unitSystem={unitSystem}
                      goalElevationUnit={goalElevationUnit}
                    />
                  }
                />
                <Bar
                  dataKey="z2Hours"
                  fill={Z2_COLOR}
                  fillOpacity={0.8}
                  radius={[6, 6, 0, 0]}
                  maxBarSize={28}
                />
                <Line
                  type="monotone"
                  dataKey="climbingGain"
                  yAxisId="climbing"
                  stroke={CLIMB_COLOR}
                  strokeWidth={3}
                  dot={{ r: 3, fill: CLIMB_COLOR, strokeWidth: 0 }}
                  activeDot={{ r: 5, fill: CLIMB_COLOR }}
                />
              </ComposedChart>
            </ResponsiveContainer>
          </div>
        </section>

        <section className="space-y-4 rounded-box border border-base-300 bg-base-100 p-5 shadow-sm sm:p-6">
          <div>
            <h2 className="text-xl font-semibold text-base-content">
              Next ride guidance
            </h2>
            <p className="mt-1 text-sm text-base-content/70">
              Deterministic nudges based on endurance volume, climbing work, and
              durability.
            </p>
          </div>

          <div className="space-y-3">
            {progress.recommendations.map((recommendation) => (
              <RecommendationCard
                key={recommendation.key}
                recommendation={recommendation}
              />
            ))}
          </div>

          {progress.summary.recent_ride_count === 0 ? (
            <div className="rounded-box border border-dashed border-base-300 bg-base-200/60 p-4 text-sm leading-6 text-base-content/70">
              Import a longer XC ride to seed this screen with baseline volume,
              route-family comparisons, and race-readiness benchmarks.
              <div className="mt-3">
                <Link href="/upload" className="btn btn-sm btn-primary">
                  Upload activity
                </Link>
              </div>
            </div>
          ) : null}
        </section>
      </div>

      <div className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_minmax(0,1.2fr)]">
        <section className="rounded-box border border-base-300 bg-base-100 p-5 shadow-sm sm:p-6">
          <div className="mb-5 flex flex-wrap items-start justify-between gap-3">
            <div>
              <h2 className="text-xl font-semibold text-base-content">
                Comparable ride decoupling
              </h2>
              <p className="mt-1 text-sm text-base-content/70">
                {eventGoal
                  ? "Lower is better. This chart now follows the full training block so you can see whether durability is improving week over week."
                  : "Lower is better. The red line marks the current v1 target."}
              </p>
            </div>
            <span className="badge badge-outline gap-2 px-3 py-2">
              Goal 5.0%
            </span>
          </div>

          {decouplingChartData.length > 0 ? (
            <div
              role="img"
              aria-label="XC decoupling trend chart"
              className="h-[260px] w-full"
            >
              <ResponsiveContainer
                width="100%"
                height="100%"
                minWidth={320}
                minHeight={260}
              >
                <ComposedChart
                  data={decouplingChartData}
                  margin={{ top: 8, right: 12, bottom: 8, left: 0 }}
                >
                  <CartesianGrid
                    vertical={false}
                    stroke="var(--color-base-content)"
                    strokeOpacity={0.1}
                  />
                  <XAxis
                    axisLine={false}
                    dataKey="label"
                    tick={{ fill: "var(--color-base-content)", fontSize: 10 }}
                    tickLine={false}
                    minTickGap={20}
                  />
                  <YAxis
                    axisLine={false}
                    tick={{ fill: "var(--color-base-content)", fontSize: 10 }}
                    tickLine={false}
                    width={56}
                    tickFormatter={(value: number) => `${value.toFixed(0)}%`}
                  />
                  <Tooltip content={<DecouplingTooltip />} />
                  <ReferenceLine
                    y={5}
                    stroke={GOAL_LINE_COLOR}
                    strokeDasharray="4 4"
                  />
                  <Line
                    type="monotone"
                    dataKey="aerobicDecouplingPercent"
                    stroke={DECOUPLING_COLOR}
                    strokeWidth={3}
                    dot={{ r: 4, fill: DECOUPLING_COLOR, strokeWidth: 0 }}
                    activeDot={{ r: 6, fill: DECOUPLING_COLOR }}
                  />
                </ComposedChart>
              </ResponsiveContainer>
            </div>
          ) : (
            <EmptyComparableState />
          )}
        </section>

        <section className="rounded-box border border-base-300 bg-base-100 p-5 shadow-sm sm:p-6">
          <div className="mb-5 flex flex-wrap items-start justify-between gap-3">
            <div>
              <h2 className="text-xl font-semibold text-base-content">
                {eventGoal
                  ? "Training-block ride benchmarks"
                  : "Recent ride benchmarks"}
              </h2>
              <p className="mt-1 text-sm text-base-content/70">
                {eventGoal
                  ? "Latest qualifying endurance rides inside the saved training block, including Z2 speed, climb totals, and ride vertical rate."
                  : "Recent endurance rides and the metrics that feed the XC screen."}
              </p>
            </div>
            <span className="badge badge-outline gap-2 px-3 py-2">
              {progress.recent_rides.length} rides shown
            </span>
          </div>

          <div className="overflow-x-auto" aria-label="XC recent rides table">
            <table className="table table-sm">
              <thead>
                <tr>
                  <th>Ride</th>
                  <th>Focus</th>
                  <th>Date</th>
                  <th>Z2</th>
                  <th>Z2 speed</th>
                  <th>Climb</th>
                  <th>Vertical rate</th>
                  <th>Distance</th>
                  <th>Decoupling</th>
                </tr>
              </thead>
              <tbody>
                {progress.recent_rides.map((ride) => (
                  <tr key={ride.activity_id}>
                    <td>
                      <div className="space-y-1">
                        <Link
                          href={`/activities/${ride.activity_id}`}
                          className="link link-primary link-hover font-medium no-underline"
                        >
                          {ride.activity_title}
                        </Link>
                        <div className="text-xs text-base-content/55">
                          {formatRouteFamily(ride.route_family_key)}
                        </div>
                      </div>
                    </td>
                    <td>
                      <span className={rideFocusBadgeClass(ride.ride_focus)}>
                        {formatRideFocusLabel(ride.ride_focus)}
                      </span>
                    </td>
                    <td className="whitespace-nowrap text-sm text-base-content/70">
                      {formatShortDate(ride.started_at)}
                    </td>
                    <td className="whitespace-nowrap">
                      {formatDuration(ride.z2_time_seconds)}
                    </td>
                    <td className="whitespace-nowrap">
                      {formatSpeed(ride.z2_average_speed_mps, unitSystem)}
                    </td>
                    <td className="whitespace-nowrap">
                      {formatElevation(
                        ride.climbing_elevation_gain_meters ??
                          ride.elevation_gain_meters,
                        unitSystem,
                      )}
                    </td>
                    <td className="whitespace-nowrap">
                      {formatClimbRate(
                        calculateVerticalRate(
                          ride.climbing_elevation_gain_meters,
                          ride.climbing_time_seconds,
                        ),
                        goalElevationUnit,
                      )}
                    </td>
                    <td className="whitespace-nowrap">
                      {formatDistance(ride.distance_meters, unitSystem)}
                    </td>
                    <td className="whitespace-nowrap font-medium">
                      {ride.aerobic_decoupling_percent != null
                        ? `${ride.aerobic_decoupling_percent.toFixed(1)}%`
                        : "--"}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>
      </div>
    </section>
  );
}
