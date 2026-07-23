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
  type TrainingMetricUnit,
  type TrainingRecommendation,
  type UserPreferences,
  type XcEventProfile,
  type XcRaceResult,
  type XcReadinessGate,
  type XcReadinessSummary,
  type XcSuggestedRide,
  type XcTrainingDeficit,
  type XcTrainingPurpose,
} from "../lib/queries";
import { hasConfiguredHeartRateZoneBounds } from "../lib/trainingProfile";
import AuthRequiredCard from "./AuthRequiredCard";

const Z2_COLOR = "#0f766e";
const CLIMB_COLOR = "#ea580c";
const DECOUPLING_COLOR = "#2563eb";
const DISTANCE_COLOR = "#475569";
const ZONE_COLORS = {
  z1: "#94a3b8",
  z2: "#0f766e",
  z3: "#f59e0b",
  z4: "#dc2626",
  z5: "#7f1d1d",
};
const METERS_PER_MILE = 1609.344;
const FEET_PER_METER = 3.28084;
const XC_GOAL_DISTANCE_MAX_MILES = 500;
const XC_GOAL_DISTANCE_MAX_METERS =
  XC_GOAL_DISTANCE_MAX_MILES * METERS_PER_MILE;
const XC_GOAL_DISTANCE_MAX_KILOMETERS =
  XC_GOAL_DISTANCE_MAX_METERS / 1000;
const XC_GOAL_ELEVATION_MAX_FEET = 25000;
const XC_GOAL_ELEVATION_MAX_METERS =
  XC_GOAL_ELEVATION_MAX_FEET / FEET_PER_METER;
const RIDE_BENCHMARK_PAGE_SIZE = 5;

type GoalDistanceUnit = "mi" | "km";
type GoalElevationUnit = "ft" | "m";

type WeeklyChartPoint = {
  label: string;
  rideCount: number;
  distanceMeters: number;
  distanceChartValue: number;
  climbingGainMeters: number;
  climbingGainChartValue: number;
  comparableRideCount: number;
  averageZ2SpeedMps?: number | null;
  averageZ2SpeedChartValue?: number | null;
  averageZ2SpeedIndex?: number | null;
  climbingVerticalRateMetersPerHour?: number | null;
  climbingVerticalRateChartValue?: number | null;
  climbingVerticalRateIndex?: number | null;
  averageAerobicDecouplingPercent?: number | null;
  aerobicDecouplingIndex?: number | null;
  z1Hours: number;
  z2ZoneHours: number;
  z3Hours: number;
  z4Hours: number;
  z5Hours: number;
};

type TrendSnapshot = {
  baseline: number;
  recent: number;
  delta: number;
  deltaPercent: number | null;
};

type TrendDirection = "improving" | "flat" | "declining";

type TrendSummary = {
  label: string;
  value: string;
  direction: TrendDirection | null;
  detail: string;
  targetDetail?: string;
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

function formatNumber(value: number, maximumFractionDigits: number) {
  return new Intl.NumberFormat(undefined, {
    maximumFractionDigits,
  }).format(value);
}

function parseOptionalNumberInput(value: string) {
  const trimmed = value.trim();

  if (!trimmed) {
    return null;
  }

  const parsed = Number(trimmed);
  return Number.isFinite(parsed) ? parsed : null;
}

function formatTargetFinishTimeInput(
  valueSeconds: number | null | undefined,
) {
  if (valueSeconds == null || !Number.isFinite(valueSeconds)) {
    return "";
  }

  const hours = valueSeconds / 3600;
  return Number.isInteger(hours) ? hours.toFixed(0) : hours.toFixed(1);
}

function finishTimeHoursToSeconds(value: number | null) {
  if (value == null || !Number.isFinite(value)) {
    return null;
  }

  return Math.round(value * 3600);
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
    case "meters_per_second":
      return formatSpeed(value, unitSystem);
    case "meters_per_hour":
      return formatClimbRate(value, unitSystem === "metric" ? "m" : "ft");
    case "meters_per_kilometer":
      return formatClimbDensity(
        value,
        1000,
        unitSystem === "metric" ? "km" : "mi",
        unitSystem === "metric" ? "m" : "ft",
      );
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
      return "badge badge-success badge-outline whitespace-nowrap font-medium";
    case "mixed_xc":
      return "badge badge-info badge-outline whitespace-nowrap font-medium";
    case "dh_session":
      return "badge badge-warning badge-outline whitespace-nowrap font-medium";
    default:
      return "badge badge-ghost whitespace-nowrap font-medium";
  }
}

function formatEventProfileLabel(profile: XcEventProfile | null | undefined) {
  switch (profile) {
    case "xc_marathon":
      return "XC marathon";
    case "technical_singletrack":
      return "Technical singletrack";
    case "endurance_mtb":
      return "Endurance MTB";
    case "ultra_mtb":
      return "Ultra MTB";
    case "custom":
      return "Custom";
    default:
      return "No profile";
  }
}

function formatTrainingPurposeLabel(purpose: XcTrainingPurpose) {
  switch (purpose) {
    case "base_endurance":
      return "Base endurance";
    case "climb_durability":
      return "Climb durability";
    case "tempo":
      return "Tempo";
    case "threshold":
      return "Threshold";
    case "punch_vo2":
      return "Punch / VO2";
    case "technical_fatigue":
      return "Technical fatigue";
    case "recovery":
      return "Recovery";
    case "data_quality":
      return "Data quality";
    default:
      return purpose;
  }
}

function trainingPurposeBadgeClass(purpose: XcTrainingPurpose) {
  switch (purpose) {
    case "base_endurance":
      return "badge badge-success badge-outline whitespace-nowrap";
    case "climb_durability":
      return "badge badge-warning badge-outline whitespace-nowrap";
    case "tempo":
      return "badge badge-info badge-outline whitespace-nowrap";
    case "threshold":
      return "badge badge-error badge-outline whitespace-nowrap";
    case "punch_vo2":
      return "badge badge-secondary badge-outline whitespace-nowrap";
    case "technical_fatigue":
      return "badge badge-primary badge-outline whitespace-nowrap";
    case "recovery":
      return "badge badge-accent badge-outline whitespace-nowrap";
    default:
      return "badge badge-ghost whitespace-nowrap";
  }
}

function formatReadinessStatusLabel(status: XcReadinessSummary["status"]) {
  switch (status) {
    case "on_track":
      return "On track";
    case "watch":
      return "Watch";
    case "falling_behind":
      return "Falling behind";
    case "missing_data":
      return "Missing data";
    default:
      return status;
  }
}

function readinessBadgeClass(status: XcReadinessSummary["status"]) {
  switch (status) {
    case "on_track":
      return "badge badge-success whitespace-nowrap gap-2 px-3 py-3";
    case "watch":
      return "badge badge-warning whitespace-nowrap gap-2 px-3 py-3";
    case "falling_behind":
      return "badge badge-error whitespace-nowrap gap-2 px-3 py-3";
    default:
      return "badge badge-ghost whitespace-nowrap gap-2 px-3 py-3";
  }
}

function readinessPanelClass(status: XcReadinessSummary["status"]) {
  switch (status) {
    case "on_track":
      return "border-success/35 bg-success/5";
    case "watch":
      return "border-warning/35 bg-warning/10";
    case "falling_behind":
      return "border-error/35 bg-error/10";
    default:
      return "border-base-300 bg-base-100";
  }
}

function recommendationPriorityBadgeClass(
  priority: TrainingRecommendation["priority"],
) {
  switch (priority) {
    case "high":
      return "badge badge-error badge-outline whitespace-nowrap uppercase";
    case "medium":
      return "badge badge-warning badge-outline whitespace-nowrap uppercase";
    default:
      return "badge badge-ghost whitespace-nowrap uppercase";
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
    return `${formatNumber(miles, miles >= 100 ? 0 : 1)} mi`;
  }

  const kilometers = value / 1000;
  return `${formatNumber(kilometers, kilometers >= 100 ? 0 : 1)} km`;
}

function formatGoalElevation(
  value: number | null | undefined,
  unit: GoalElevationUnit,
) {
  if (value == null || !Number.isFinite(value)) {
    return "--";
  }

  if (unit === "ft") {
    return `${formatNumber(Math.round(value * FEET_PER_METER), 0)} ft`;
  }

  return `${formatNumber(Math.round(value), 0)} m`;
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

function averageNumberArray(values: number[]) {
  if (values.length === 0) {
    return null;
  }

  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

function buildTrend(values: number[]): TrendSnapshot | null {
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

function firstFiniteValue(values: Array<number | null | undefined>) {
  return values.find(
    (value): value is number => value != null && Number.isFinite(value),
  );
}

function indexedTrendValue(
  value: number | null | undefined,
  baseline: number | null | undefined,
) {
  if (
    value == null ||
    baseline == null ||
    !Number.isFinite(value) ||
    !Number.isFinite(baseline) ||
    baseline <= 0
  ) {
    return null;
  }

  return (value / baseline) * 100;
}

function classifyTrend(
  trend: TrendSnapshot | null,
  options: { lowerIsBetter?: boolean; flatThresholdPercent?: number } = {},
): TrendDirection | null {
  if (!trend) {
    return null;
  }

  const flatThresholdPercent = options.flatThresholdPercent ?? 3;
  const isFlat =
    trend.deltaPercent != null
      ? Math.abs(trend.deltaPercent) < flatThresholdPercent
      : Math.abs(trend.delta) < 0.01;

  if (isFlat) {
    return "flat";
  }

  const movedUp = trend.delta > 0;
  return options.lowerIsBetter
    ? movedUp
      ? "declining"
      : "improving"
    : movedUp
      ? "improving"
      : "declining";
}

function formatTrendDirection(direction: TrendDirection | null) {
  switch (direction) {
    case "improving":
      return "Improving";
    case "flat":
      return "Flat";
    case "declining":
      return "Declining";
    default:
      return "Needs data";
  }
}

function trendDirectionClass(direction: TrendDirection | null) {
  switch (direction) {
    case "improving":
      return "text-success";
    case "flat":
      return "text-warning";
    case "declining":
      return "text-error";
    default:
      return "text-base-content/45";
  }
}

function formatTrendChange(
  trend: TrendSnapshot | null,
  formatter: (value: number) => string,
) {
  if (!trend) {
    return "Need at least two weeks of usable data.";
  }

  if (Math.abs(trend.delta) < 0.01) {
    return "Flat vs opening block weeks.";
  }

  return `${trend.delta > 0 ? "Up" : "Down"} ${formatter(
    Math.abs(trend.delta),
  )} vs opening block weeks.`;
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

function distanceChartValue(valueMeters: number, unit: GoalDistanceUnit) {
  return unit === "mi" ? valueMeters / METERS_PER_MILE : valueMeters / 1000;
}

function elevationChartValue(valueMeters: number, unit: GoalElevationUnit) {
  return unit === "ft" ? valueMeters * FEET_PER_METER : valueMeters;
}

function speedChartValue(
  valueMetersPerSecond: number | null | undefined,
  unitSystem: UnitSystem,
) {
  if (
    valueMetersPerSecond == null ||
    !Number.isFinite(valueMetersPerSecond)
  ) {
    return null;
  }

  return unitSystem === "metric"
    ? valueMetersPerSecond * 3.6
    : valueMetersPerSecond * 2.236936;
}

function trendWholePercentPoints(value: number) {
  return `${Math.round(value)} pts`;
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

function formatTrainingMetricValue(
  unit: TrainingMetricUnit | null | undefined,
  value: number | null | undefined,
  unitSystem: UnitSystem,
  distanceUnit: GoalDistanceUnit,
  elevationUnit: GoalElevationUnit,
) {
  if (value == null || !Number.isFinite(value) || !unit) {
    return "--";
  }

  switch (unit) {
    case "seconds":
      return formatDuration(Math.round(value));
    case "meters":
      return formatGoalElevation(value, elevationUnit);
    case "percent":
      return `${value.toFixed(1)}%`;
    case "count":
      return Number.isInteger(value) ? `${value}` : value.toFixed(1);
    case "meters_per_second":
      return formatSpeed(value, unitSystem);
    case "meters_per_kilometer":
      return formatClimbDensity(value, 1000, distanceUnit, elevationUnit);
    case "meters_per_hour":
      return formatClimbRate(value, elevationUnit);
    default:
      return formatCompactMetric(value);
  }
}

function formatReadinessGateValue(
  gate: XcReadinessGate,
  value: number | null | undefined,
  unitSystem: UnitSystem,
  distanceUnit: GoalDistanceUnit,
  elevationUnit: GoalElevationUnit,
) {
  if (value == null || !Number.isFinite(value)) {
    return "--";
  }

  switch (gate.key) {
    case "long_ride_distance":
      return formatGoalDistance(value, distanceUnit);
    case "big_climb_day":
      return formatGoalElevation(value, elevationUnit);
    default:
      return formatTrainingMetricValue(
        gate.unit,
        value,
        unitSystem,
        distanceUnit,
        elevationUnit,
      );
  }
}

function formatDeficitGapValue(
  deficit: Pick<XcTrainingDeficit, "key" | "gap_unit" | "gap_value">,
  unitSystem: UnitSystem,
  distanceUnit: GoalDistanceUnit,
  elevationUnit: GoalElevationUnit,
) {
  if (deficit.gap_value == null || !Number.isFinite(deficit.gap_value)) {
    return "--";
  }

  if (deficit.gap_unit === "meters") {
    switch (deficit.key) {
      case "long_ride":
        return formatGoalDistance(deficit.gap_value, distanceUnit);
      case "big_climb_day":
        return formatGoalElevation(deficit.gap_value, elevationUnit);
      default:
        break;
    }
  }

  return formatTrainingMetricValue(
    deficit.gap_unit,
    deficit.gap_value,
    unitSystem,
    distanceUnit,
    elevationUnit,
  );
}

function formatRecommendationGapValue(
  recommendation: TrainingRecommendation,
  unitSystem: UnitSystem,
  distanceUnit: GoalDistanceUnit,
  elevationUnit: GoalElevationUnit,
) {
  if (
    recommendation.gap_value == null ||
    !Number.isFinite(recommendation.gap_value)
  ) {
    return "--";
  }

  if (recommendation.gap_unit === "meters") {
    switch (recommendation.key) {
      case "increase_endurance_volume":
        return formatGoalDistance(recommendation.gap_value, distanceUnit);
      case "add_climbing_endurance":
        return formatGoalElevation(recommendation.gap_value, elevationUnit);
      default:
        break;
    }
  }

  return formatTrainingMetricValue(
    recommendation.gap_unit,
    recommendation.gap_value,
    unitSystem,
    distanceUnit,
    elevationUnit,
  );
}

function formatSuggestedRideDuration(ride: XcSuggestedRide) {
  function formatCompactDuration(value: number) {
    const hours = Math.floor(value / 3600);
    const minutes = Math.round((value % 3600) / 60);

    if (hours > 0 && minutes > 0) {
      return `${hours}h ${minutes}m`;
    }

    if (hours > 0) {
      return `${hours}h`;
    }

    return `${minutes}m`;
  }

  const min = ride.duration_seconds_min;
  const max = ride.duration_seconds_max;

  if (min != null && max != null) {
    return min === max
      ? formatCompactDuration(min)
      : `${formatCompactDuration(min)}-${formatCompactDuration(max)}`;
  }

  if (min != null) {
    return `${formatCompactDuration(min)}+`;
  }

  if (max != null) {
    return `Up to ${formatCompactDuration(max)}`;
  }

  return "--";
}

function formatSuggestedRideDistance(
  ride: XcSuggestedRide,
  distanceUnit: GoalDistanceUnit,
) {
  const min = ride.distance_meters_min;
  const max = ride.distance_meters_max;

  if (min != null && max != null) {
    return min === max
      ? formatGoalDistance(min, distanceUnit)
      : `${formatGoalDistance(min, distanceUnit)}-${formatGoalDistance(
          max,
          distanceUnit,
        )}`;
  }

  if (min != null) {
    return `${formatGoalDistance(min, distanceUnit)}+`;
  }

  if (max != null) {
    return `Up to ${formatGoalDistance(max, distanceUnit)}`;
  }

  return "--";
}

function buildPreferencesPayload(
  currentPreferences: UserPreferences | null,
  overrides: {
    xcGoalEventName: string | null;
    xcGoalStartDate: string | null;
    xcGoalTargetDate: string | null;
    xcGoalTargetDistanceMeters: number | null;
    xcGoalTargetElevationGainMeters: number | null;
    xcGoalTargetFinishTimeSeconds: number | null;
    xcGoalEventProfile: XcEventProfile | null;
  },
): UserPreferences {
  return {
    unit_system: currentPreferences?.unit_system ?? "mixed",
    estimated_ftp_watts: currentPreferences?.estimated_ftp_watts ?? null,
    heart_rate_zone_bounds_bpm:
      currentPreferences?.heart_rate_zone_bounds_bpm ?? null,
    xc_goal_event_name: overrides.xcGoalEventName,
    xc_goal_start_date: overrides.xcGoalStartDate,
    xc_goal_target_date: overrides.xcGoalTargetDate,
    xc_goal_target_distance_meters: overrides.xcGoalTargetDistanceMeters,
    xc_goal_target_elevation_gain_meters:
      overrides.xcGoalTargetElevationGainMeters,
    xc_goal_target_finish_time_seconds:
      overrides.xcGoalTargetFinishTimeSeconds,
    xc_goal_event_profile: overrides.xcGoalEventProfile,
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

function TrendSummaryItem({ summary }: { summary: TrendSummary }) {
  return (
    <div className="border-t border-base-300/70 pt-3">
      <div className="flex items-center justify-between gap-3">
        <p className="text-xs uppercase tracking-[0.18em] text-base-content/45">
          {summary.label}
        </p>
        <span
          className={`whitespace-nowrap text-xs font-semibold uppercase ${trendDirectionClass(
            summary.direction,
          )}`}
        >
          {formatTrendDirection(summary.direction)}
        </span>
      </div>
      <p className="mt-2 text-xl font-semibold text-base-content">
        {summary.value}
      </p>
      <p className="mt-1 text-sm leading-6 text-base-content/65">
        {summary.detail}
      </p>
      {summary.targetDetail ? (
        <p className="mt-1 text-sm leading-6 text-base-content/55">
          {summary.targetDetail}
        </p>
      ) : null}
    </div>
  );
}

function TargetFact({
  label,
  value,
}: {
  label: string;
  value: string | null | undefined;
}) {
  return (
    <div>
      <dt className="text-xs uppercase tracking-[0.16em] text-base-content/45">
        {label}
      </dt>
      <dd className="mt-1 font-medium text-base-content">{value || "--"}</dd>
    </div>
  );
}

function SuggestedRideDetails({
  ride,
  goalDistanceUnit,
  goalElevationUnit,
}: {
  ride: XcSuggestedRide;
  goalDistanceUnit: GoalDistanceUnit;
  goalElevationUnit: GoalElevationUnit;
}) {
  const durationValue = formatSuggestedRideDuration(ride);
  const distanceValue =
    ride.distance_meters_min != null || ride.distance_meters_max != null
      ? formatSuggestedRideDistance(ride, goalDistanceUnit)
      : null;
  const climbValue =
    ride.climbing_elevation_gain_meters != null
      ? formatGoalElevation(
          ride.climbing_elevation_gain_meters,
          goalElevationUnit,
        )
      : null;
  const rows = [
    { label: "Time", value: durationValue },
    { label: "Distance", value: distanceValue },
    { label: "Climb", value: climbValue },
    { label: "Intensity", value: ride.intensity },
    { label: "Terrain", value: ride.terrain },
  ].filter(
    (row): row is { label: string; value: string } =>
      row.value != null && row.value !== "--",
  );

  return (
    <div className="mt-4 space-y-3 text-sm text-base-content/70">
      <dl className="space-y-2">
        {rows.map((row) => (
          <div key={row.label} className="grid gap-1 sm:grid-cols-[7rem_1fr]">
            <dt className="text-xs uppercase tracking-[0.16em] text-base-content/45">
              {row.label}
            </dt>
            <dd className="font-medium leading-5 text-base-content">
              {row.value}
            </dd>
          </div>
        ))}
      </dl>
      <p className="mt-2 text-sm leading-6 text-base-content/70">
        {ride.detail}
      </p>
    </div>
  );
}

function ReadinessOverview({
  readiness,
  deficits,
  unitSystem,
  goalDistanceUnit,
  goalElevationUnit,
}: {
  readiness: XcReadinessSummary;
  deficits: XcTrainingDeficit[];
  unitSystem: UnitSystem;
  goalDistanceUnit: GoalDistanceUnit;
  goalElevationUnit: GoalElevationUnit;
}) {
  const primaryDeficit = deficits[0] ?? null;

  return (
    <section
      className={`rounded-box border p-4 shadow-sm sm:p-5 ${readinessPanelClass(
        readiness.status,
      )}`}
    >
      <div className="grid gap-4 xl:grid-cols-[minmax(0,0.85fr)_minmax(280px,1.15fr)]">
        <div>
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div>
              <p className="text-sm uppercase tracking-[0.2em] text-base-content/45">
                Quick status
              </p>
              <h2 className="mt-1 text-xl font-semibold text-base-content">
                {readiness.title}
              </h2>
            </div>
            <span className={readinessBadgeClass(readiness.status)}>
              {formatReadinessStatusLabel(readiness.status)}
            </span>
          </div>
          <p className="mt-2 text-sm leading-6 text-base-content/75">
            {readiness.reason}
          </p>
          {readiness.missing_most ? (
            <p className="mt-2 text-sm font-medium text-base-content">
              Missing most: {readiness.missing_most}
            </p>
          ) : null}
        </div>

        <div className="rounded-box border border-base-300/80 bg-base-100/80 p-3 sm:p-4">
          <p className="text-sm font-semibold text-base-content">
            What am I missing?
          </p>
          {primaryDeficit ? (
            <div className="mt-3 space-y-3">
              <div>
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <h3 className="font-semibold text-base-content">
                    {primaryDeficit.title}
                  </h3>
                  <span
                    className={recommendationPriorityBadgeClass(
                      primaryDeficit.priority,
                    )}
                  >
                    {primaryDeficit.priority}
                  </span>
              </div>
              <p className="mt-2 text-sm leading-6 text-base-content/70">
                {primaryDeficit.detail}
              </p>
              {primaryDeficit.gap_value != null ? (
                <p className="mt-2 text-sm font-medium text-base-content">
                    Gap:{" "}
                    {formatDeficitGapValue(
                      primaryDeficit,
                      unitSystem,
                      goalDistanceUnit,
                      goalElevationUnit,
                    )}
                  </p>
                ) : null}
              </div>
            </div>
          ) : (
            <p className="mt-3 text-sm leading-6 text-base-content/70">
              No major limiter is flagged right now. Keep stacking event-like
              endurance and climbing work while freshness stays manageable.
            </p>
          )}
        </div>
      </div>

      <div className="mt-4 grid gap-3 md:grid-cols-2 xl:grid-cols-4">
        {readiness.gates.map((gate) => {
          const progressPercent = gate.progress_percent ?? 0;
          const gapLabel = gate.direction === "at_most" ? "Reduce" : "Need";

          return (
            <div
              key={gate.key}
              className="rounded-box border border-base-300/80 bg-base-100/75 p-4"
            >
              <div className="flex items-start justify-between gap-3">
                <div
                  className="tooltip tooltip-right cursor-help text-left"
                  data-tip={gate.detail}
                >
                  <h3
                    className="font-semibold text-base-content underline decoration-dotted underline-offset-4"
                    title={gate.detail}
                  >
                    {gate.label}
                  </h3>
                </div>
                <span className={readinessBadgeClass(gate.status)}>
                  {formatReadinessStatusLabel(gate.status)}
                </span>
              </div>
              <div className="mt-3 text-sm text-base-content/70">
                <span className="font-medium text-base-content">
                  {formatReadinessGateValue(
                    gate,
                    gate.current_value,
                    unitSystem,
                    goalDistanceUnit,
                    goalElevationUnit,
                  )}
                </span>
                <span> / </span>
                <span>
                  {formatReadinessGateValue(
                    gate,
                    gate.target_value,
                    unitSystem,
                    goalDistanceUnit,
                    goalElevationUnit,
                  )}
                </span>
              </div>
              <progress
                className={`mt-3 ${goalProgressClass(gate.progress_percent)}`}
                value={progressPercent}
                max={100}
              />
              {gate.gap_value != null ? (
                <p className="mt-2 text-sm font-medium text-base-content">
                  {gapLabel}{" "}
                  {formatReadinessGateValue(
                    gate,
                    gate.gap_value,
                    unitSystem,
                    goalDistanceUnit,
                    goalElevationUnit,
                  )}
                </p>
              ) : null}
            </div>
          );
        })}
      </div>
    </section>
  );
}

function formatRaceComparison(value: number | null | undefined) {
  if (value == null || !Number.isFinite(value)) {
    return "--";
  }

  return `${value.toFixed(0)}%`;
}

function RaceResultCard({
  race,
  goalDistanceUnit,
  goalElevationUnit,
  unitSystem,
}: {
  race: XcRaceResult;
  goalDistanceUnit: GoalDistanceUnit;
  goalElevationUnit: GoalElevationUnit;
  unitSystem: UnitSystem;
}) {
  function RaceMetric({
    label,
    value,
    detail,
  }: {
    label: string;
    value: string;
    detail: string;
  }) {
    return (
      <div className="border-l border-base-300 pl-3">
        <p className="text-xs uppercase tracking-[0.18em] text-base-content/45">
          {label}
        </p>
        <p className="mt-1 text-xl font-semibold text-base-content">{value}</p>
        <p className="mt-1 text-sm text-base-content/60">{detail}</p>
      </div>
    );
  }

  return (
    <article className="rounded-box border border-base-300 bg-base-100 p-5 shadow-sm sm:p-6">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <div className="flex flex-wrap items-center gap-2">
            <span className="badge badge-error badge-outline whitespace-nowrap">
              Race
            </span>
            <span className="text-sm text-base-content/55">
              {formatLongDate(race.started_at)}
            </span>
          </div>
          <h2 className="mt-2 text-xl font-semibold text-base-content">
            <Link
              href={`/activities/${race.activity_id}`}
              className="link link-primary link-hover no-underline"
            >
              {race.activity_title}
            </Link>
          </h2>
        </div>
        <div className="text-right">
          <p className="text-2xl font-semibold text-base-content">
            {formatDistance(race.distance_meters, unitSystem)}
          </p>
          <p className="text-sm text-base-content/60">
            {formatElevation(race.elevation_gain_meters, unitSystem)}
          </p>
        </div>
      </div>

      <div className="mt-5 grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        <RaceMetric
          label="Race pace"
          value={formatSpeed(race.average_speed_mps, unitSystem)}
          detail={`${formatDuration(race.moving_time_seconds ?? 0)} moving time`}
        />
        <RaceMetric
          label="Race density"
          value={formatClimbDensity(
            race.elevation_gain_meters,
            race.distance_meters,
            goalDistanceUnit,
            goalElevationUnit,
          )}
          detail="Climbing demand per distance"
        />
        <RaceMetric
          label="4-week Z2"
          value={formatDuration(race.prior_training_z2_time_seconds)}
          detail={`${race.prior_training_ride_count} qualifying rides before race day`}
        />
        <RaceMetric
          label="4-week climbing"
          value={formatElevation(
            race.prior_training_climbing_elevation_gain_meters,
            unitSystem,
          )}
          detail={`Race was ${formatRaceComparison(race.race_vs_best_training_elevation_percent)} of best prior climb ride`}
        />
      </div>

      <div className="mt-5 border-l-2 border-primary/35 bg-base-200/50 px-4 py-3">
        <h3 className="font-semibold text-base-content">
          {race.insight_title}
        </h3>
        <p className="mt-1 text-sm leading-6 text-base-content/70">
          {race.insight_detail}
        </p>
        <dl className="mt-3 grid gap-2 text-xs text-base-content/65 sm:grid-cols-3">
          <div>
            <dt className="uppercase tracking-[0.14em] text-base-content/45">
              Distance vs best training
            </dt>
            <dd className="mt-1 font-medium text-base-content">
              {formatRaceComparison(race.race_vs_best_training_distance_percent)}
            </dd>
          </div>
          <div>
            <dt className="uppercase tracking-[0.14em] text-base-content/45">
              Avg Z2 speed before race
            </dt>
            <dd className="mt-1 font-medium text-base-content">
              {formatSpeed(race.prior_training_average_z2_speed_mps, unitSystem)}
            </dd>
          </div>
          <div>
            <dt className="uppercase tracking-[0.14em] text-base-content/45">
              Avg decoupling before race
            </dt>
            <dd className="mt-1 font-medium text-base-content">
              {race.prior_training_average_aerobic_decoupling_percent != null
                ? `${race.prior_training_average_aerobic_decoupling_percent.toFixed(1)}%`
                : "--"}
            </dd>
          </div>
        </dl>
      </div>
    </article>
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
        <span className="badge badge-outline whitespace-nowrap uppercase">
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
  unitSystem,
  goalDistanceUnit,
  goalElevationUnit,
}: {
  recommendation: TrainingRecommendation;
  unitSystem: UnitSystem;
  goalDistanceUnit: GoalDistanceUnit;
  goalElevationUnit: GoalElevationUnit;
}) {
  return (
    <article className="rounded-box border border-base-300 bg-base-100 p-4 shadow-sm">
      <div className="flex items-start justify-between gap-3">
        <div>
          <h3 className="text-base font-semibold text-base-content">
            {recommendation.title}
          </h3>
          <div className="mt-2 flex flex-wrap gap-2">
            {recommendation.purpose ? (
              <span
                className={trainingPurposeBadgeClass(recommendation.purpose)}
              >
                {formatTrainingPurposeLabel(recommendation.purpose)}
              </span>
            ) : null}
          </div>
        </div>
        <span
          className={recommendationPriorityBadgeClass(recommendation.priority)}
        >
          {recommendation.priority}
        </span>
      </div>
      <p className="mt-2 text-sm leading-6 text-base-content/70">
        {recommendation.detail}
      </p>
      {recommendation.limiter ? (
        <p className="mt-2 text-sm text-base-content/70">
          <span className="font-medium text-base-content">Limiter:</span>{" "}
          {recommendation.limiter}
        </p>
      ) : null}
      {recommendation.gap_value != null ? (
        <p className="mt-2 text-sm font-medium text-base-content">
          Gap:{" "}
          {formatRecommendationGapValue(
            recommendation,
            unitSystem,
            goalDistanceUnit,
            goalElevationUnit,
          )}
        </p>
      ) : null}
      {recommendation.suggested_ride ? (
        <SuggestedRideDetails
          ride={recommendation.suggested_ride}
          goalDistanceUnit={goalDistanceUnit}
          goalElevationUnit={goalElevationUnit}
        />
      ) : null}
    </article>
  );
}

function VolumeTrendTooltip({
  active,
  payload,
  goalDistanceUnit,
  goalElevationUnit,
}: {
  active?: boolean;
  payload?: Array<{ payload?: WeeklyChartPoint }>;
  goalDistanceUnit: GoalDistanceUnit;
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
          <span>Distance</span>
          <span className="font-medium text-base-content">
            {formatGoalDistance(point.distanceMeters, goalDistanceUnit)}
          </span>
        </div>
        <div className="flex items-center justify-between gap-4">
          <span>Climbing</span>
          <span className="font-medium text-base-content">
            {formatGoalElevation(point.climbingGainMeters, goalElevationUnit)}
          </span>
        </div>
        <div className="flex items-center justify-between gap-4">
          <span>Rides</span>
          <span className="font-medium text-base-content">
            {point.rideCount}
          </span>
        </div>
      </div>
    </div>
  );
}

function PaceDurabilityTooltip({
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
            <span>Climb rate</span>
            <span className="font-medium text-base-content">
              {formatClimbRate(
                point.climbingVerticalRateMetersPerHour,
                goalElevationUnit,
              )}
            </span>
          </div>
        ) : null}
        {point.averageAerobicDecouplingPercent != null ? (
          <div className="flex items-center justify-between gap-4">
            <span>Decoupling</span>
            <span className="font-medium text-base-content">
              {point.averageAerobicDecouplingPercent.toFixed(1)}%
            </span>
          </div>
        ) : null}
        <div className="flex items-center justify-between gap-4">
          <span>Comparable rides</span>
          <span className="font-medium text-base-content">
            {point.comparableRideCount}
          </span>
        </div>
      </div>
    </div>
  );
}

function ZoneTrendTooltip({
  active,
  payload,
}: {
  active?: boolean;
  payload?: Array<{ payload?: WeeklyChartPoint }>;
}) {
  if (!active || !payload?.length) {
    return null;
  }

  const point = payload[0]?.payload;
  if (!point) {
    return null;
  }

  const rows = [
    ["Z1", point.z1Hours],
    ["Z2", point.z2ZoneHours],
    ["Z3", point.z3Hours],
    ["Z4", point.z4Hours],
    ["Z5", point.z5Hours],
  ] as const;

  return (
    <div className="rounded-box border border-base-300 bg-base-100 px-3 py-3 shadow-lg">
      <p className="text-sm font-semibold text-base-content">{point.label}</p>
      <div className="mt-2 space-y-1.5 text-sm text-base-content/75">
        {rows.map(([label, hours]) => (
          <div key={label} className="flex items-center justify-between gap-4">
            <span>{label}</span>
            <span className="font-medium text-base-content">
              {formatDuration(Math.round(hours * 3600))}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

function EmptyTrendState({ message }: { message: string }) {
  return (
    <div className="flex h-full min-h-[260px] items-center justify-center rounded-box border border-dashed border-base-300 bg-base-200/60 px-6 text-center text-sm leading-6 text-base-content/70">
      {message}
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
  const [goalEventNameDraft, setGoalEventNameDraft] = useState("");
  const [goalStartDateDraft, setGoalStartDateDraft] = useState("");
  const [goalDateDraft, setGoalDateDraft] = useState("");
  const [goalDistanceDraft, setGoalDistanceDraft] = useState("");
  const [goalDistanceUnit, setGoalDistanceUnit] =
    useState<GoalDistanceUnit>("mi");
  const [goalElevationDraft, setGoalElevationDraft] = useState("");
  const [goalElevationUnit, setGoalElevationUnit] =
    useState<GoalElevationUnit>("ft");
  const [goalFinishTimeDraft, setGoalFinishTimeDraft] = useState("");
  const [goalEventProfileDraft, setGoalEventProfileDraft] = useState<
    XcEventProfile | ""
  >("");
  const [isEditingGoal, setIsEditingGoal] = useState(false);
  const [rideBenchmarkPage, setRideBenchmarkPage] = useState(0);

  useEffect(() => {
    const nextGoalEventNameDraft =
      preferencesQuery.data?.xc_goal_event_name ?? "";
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
    const nextGoalFinishTimeDraft = formatTargetFinishTimeInput(
      preferencesQuery.data?.xc_goal_target_finish_time_seconds,
    );
    const nextGoalEventProfileDraft =
      preferencesQuery.data?.xc_goal_event_profile ?? "";

    setGoalEventNameDraft((currentValue) =>
      currentValue === nextGoalEventNameDraft
        ? currentValue
        : nextGoalEventNameDraft,
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
    setGoalFinishTimeDraft((currentValue) =>
      currentValue === nextGoalFinishTimeDraft
        ? currentValue
        : nextGoalFinishTimeDraft,
    );
    setGoalEventProfileDraft((currentValue) =>
      currentValue === nextGoalEventProfileDraft
        ? currentValue
        : nextGoalEventProfileDraft,
    );
  }, [
    goalDistanceUnit,
    goalElevationUnit,
    preferencesQuery.data?.xc_goal_event_name,
    preferencesQuery.data?.xc_goal_start_date,
    preferencesQuery.data?.xc_goal_target_date,
    preferencesQuery.data?.xc_goal_target_distance_meters,
    preferencesQuery.data?.xc_goal_target_elevation_gain_meters,
    preferencesQuery.data?.xc_goal_target_finish_time_seconds,
    preferencesQuery.data?.xc_goal_event_profile,
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
    const points = (progressQuery.data?.weekly_progress ?? []).map((point) => {
      const averageZ2SpeedChartValue = speedChartValue(
        point.average_z2_speed_mps,
        unitSystem,
      );
      const climbingVerticalRateChartValue =
        point.climbing_vertical_rate_meters_per_hour == null
          ? null
          : elevationChartValue(
              point.climbing_vertical_rate_meters_per_hour,
              goalElevationUnit,
            );

      return {
        label: formatShortDate(point.week_start),
        rideCount: point.ride_count,
        distanceMeters: point.distance_meters,
        distanceChartValue: distanceChartValue(
          point.distance_meters,
          goalDistanceUnit,
        ),
        climbingGainMeters: point.climbing_elevation_gain_meters,
        climbingGainChartValue: elevationChartValue(
          point.climbing_elevation_gain_meters,
          goalElevationUnit,
        ),
        comparableRideCount: point.comparable_ride_count,
        averageZ2SpeedMps: point.average_z2_speed_mps,
        averageZ2SpeedChartValue,
        climbingVerticalRateMetersPerHour:
          point.climbing_vertical_rate_meters_per_hour,
        climbingVerticalRateChartValue,
        averageAerobicDecouplingPercent:
          point.average_aerobic_decoupling_percent,
        z1Hours: point.z1_seconds / 3600,
        z2ZoneHours: point.z2_zone_seconds / 3600,
        z3Hours: point.z3_seconds / 3600,
        z4Hours: point.z4_seconds / 3600,
        z5Hours: point.z5_seconds / 3600,
      };
    });

    const z2SpeedBaseline = firstFiniteValue(
      points.map((point) => point.averageZ2SpeedChartValue),
    );
    const climbingRateBaseline = firstFiniteValue(
      points.map((point) => point.climbingVerticalRateChartValue),
    );
    const decouplingBaseline = firstFiniteValue(
      points.map((point) => point.averageAerobicDecouplingPercent),
    );

    return points.map((point) => ({
      ...point,
      averageZ2SpeedIndex: indexedTrendValue(
        point.averageZ2SpeedChartValue,
        z2SpeedBaseline,
      ),
      climbingVerticalRateIndex: indexedTrendValue(
        point.climbingVerticalRateChartValue,
        climbingRateBaseline,
      ),
      aerobicDecouplingIndex: indexedTrendValue(
        point.averageAerobicDecouplingPercent,
        decouplingBaseline,
      ),
    }));
  }, [
    goalDistanceUnit,
    goalElevationUnit,
    progressQuery.data?.weekly_progress,
    unitSystem,
  ]);

  const trendSummaries = useMemo<TrendSummary[]>(() => {
    const progress = progressQuery.data;
    const eventGoal = progress?.event_goal ?? null;
    const weekPoints = progress?.weekly_progress ?? [];
    const distanceTrend = buildTrend(
      weekPoints.map((point) => point.distance_meters),
    );
    const climbingTrend = buildTrend(
      weekPoints.map((point) => point.climbing_elevation_gain_meters),
    );
    const speedTrend = buildTrend(
      weekPoints
        .map((point) => point.average_z2_speed_mps)
        .filter(
          (value): value is number => value != null && Number.isFinite(value),
        ),
    );
    const climbingRateTrend = buildTrend(
      weekPoints
        .map((point) => point.climbing_vertical_rate_meters_per_hour)
        .filter(
          (value): value is number => value != null && Number.isFinite(value),
        ),
    );
    const decouplingTrend = buildTrend(
      weekPoints
        .map((point) => point.average_aerobic_decoupling_percent)
        .filter(
          (value): value is number => value != null && Number.isFinite(value),
        ),
    );
    const z2ShareTrend = buildTrend(
      weekPoints
        .map((point) => {
          const totalZoneSeconds =
            point.z1_seconds +
            point.z2_zone_seconds +
            point.z3_seconds +
            point.z4_seconds +
            point.z5_seconds;

          return totalZoneSeconds > 0
            ? (point.z2_zone_seconds / totalZoneSeconds) * 100
            : null;
        })
        .filter(
          (value): value is number => value != null && Number.isFinite(value),
        ),
    );

    return [
      {
        label: "Weekly distance",
        value: distanceTrend
          ? `${formatGoalDistance(distanceTrend.recent, goalDistanceUnit)}/wk`
          : "--",
        direction: classifyTrend(distanceTrend),
        detail: formatTrendChange(
          distanceTrend,
          (value) => `${formatGoalDistance(value, goalDistanceUnit)}/wk`,
        ),
        targetDetail: eventGoal
          ? `Event distance: ${formatGoalDistance(
              eventGoal.target_distance_meters,
              goalDistanceUnit,
            )}; the long-ride gate checks single-day readiness.`
          : undefined,
      },
      {
        label: "Weekly climbing",
        value: climbingTrend
          ? `${formatGoalElevation(climbingTrend.recent, goalElevationUnit)}/wk`
          : "--",
        direction: classifyTrend(climbingTrend),
        detail: formatTrendChange(
          climbingTrend,
          (value) => `${formatGoalElevation(value, goalElevationUnit)}/wk`,
        ),
        targetDetail: eventGoal
          ? `Event climbing: ${formatGoalElevation(
              eventGoal.target_elevation_gain_meters,
              goalElevationUnit,
            )}; the big-climb gate checks single-day readiness.`
          : undefined,
      },
      {
        label: "Z2 speed",
        value: speedTrend ? formatSpeed(speedTrend.recent, unitSystem) : "--",
        direction: classifyTrend(speedTrend),
        detail: formatTrendChange(speedTrend, (value) =>
          formatSpeed(value, unitSystem),
        ),
        targetDetail:
          eventGoal?.target_finish_speed_mps != null
            ? `Target elapsed speed: ${formatSpeed(
                eventGoal.target_finish_speed_mps,
                unitSystem,
              )}; Z2 should trend comfortably above it.`
            : heartRateZonesConfigured
              ? "Add target finish time to compare Z2 speed against event pace."
              : "Set HR zones, then regenerate older rides for Z2 speed."
      },
      {
        label: "Climb rate",
        value: climbingRateTrend
          ? formatClimbRate(climbingRateTrend.recent, goalElevationUnit)
          : "--",
        direction: classifyTrend(climbingRateTrend),
        detail: formatTrendChange(climbingRateTrend, (value) =>
          formatClimbRate(value, goalElevationUnit),
        ),
        targetDetail: eventGoal
          ? `Event density: ${formatClimbDensity(
              eventGoal.target_elevation_gain_meters,
              eventGoal.target_distance_meters,
              goalDistanceUnit,
              goalElevationUnit,
            )}; pair rate with repeatable climbing days.`
          : undefined,
      },
      {
        label: "Aerobic decoupling",
        value: decouplingTrend
          ? `${decouplingTrend.recent.toFixed(1)}%`
          : "--",
        direction: classifyTrend(decouplingTrend, { lowerIsBetter: true }),
        detail: formatTrendChange(
          decouplingTrend,
          (value) => `${value.toFixed(1)} pts`,
        ),
        targetDetail:
          "Coach-style durability check: aim for roughly 5% or lower on comparable endurance rides.",
      },
      {
        label: "Zone 2 share",
        value: z2ShareTrend ? `${Math.round(z2ShareTrend.recent)}%` : "--",
        direction: classifyTrend(z2ShareTrend),
        detail: formatTrendChange(z2ShareTrend, trendWholePercentPoints),
        targetDetail: heartRateZonesConfigured
          ? "Uses persisted HR-zone buckets, separate from the Z2 speed sample filter."
          : "Set HR zones to make time-in-zone trends reliable.",
      },
    ];
  }, [
    goalDistanceUnit,
    goalElevationUnit,
    heartRateZonesConfigured,
    progressQuery.data,
    unitSystem,
  ]);

  const targetReadMetrics = useMemo(() => {
    const progress = progressQuery.data;
    const eventGoal = progress?.event_goal ?? null;

    if (!progress || !eventGoal) {
      return null;
    }

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
      currentClimbDensity,
      targetClimbDensity,
    };
  }, [
    goalDistanceUnit,
    goalElevationUnit,
    progressQuery.data,
  ]);
  const rideBenchmarkTotalPages = Math.max(
    Math.ceil(
      (progressQuery.data?.recent_rides.length ?? 0) /
        RIDE_BENCHMARK_PAGE_SIZE,
    ),
    1,
  );
  const rideBenchmarkStartIndex =
    rideBenchmarkPage * RIDE_BENCHMARK_PAGE_SIZE;
  const visibleRideBenchmarks = useMemo(() => {
    return (progressQuery.data?.recent_rides ?? []).slice(
      rideBenchmarkStartIndex,
      rideBenchmarkStartIndex + RIDE_BENCHMARK_PAGE_SIZE,
    );
  }, [progressQuery.data?.recent_rides, rideBenchmarkStartIndex]);

  useEffect(() => {
    setRideBenchmarkPage((currentPage) =>
      Math.min(currentPage, rideBenchmarkTotalPages - 1),
    );
  }, [rideBenchmarkTotalPages]);

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
  const hasPaceTrendData = weeklyChartData.some(
    (point) =>
      point.averageZ2SpeedIndex != null ||
      point.climbingVerticalRateIndex != null ||
      point.aerobicDecouplingIndex != null,
  );
  const hasZoneTrendData = weeklyChartData.some(
    (point) =>
      point.z1Hours +
        point.z2ZoneHours +
        point.z3Hours +
        point.z4Hours +
        point.z5Hours >
      0,
  );

  function resetGoalDraftsFromPreferences(
    preferences: UserPreferences | null | undefined,
  ) {
    setGoalEventNameDraft(preferences?.xc_goal_event_name ?? "");
    setGoalStartDateDraft(preferences?.xc_goal_start_date ?? "");
    setGoalDateDraft(preferences?.xc_goal_target_date ?? "");
    setGoalDistanceDraft(
      metersToDistanceInput(
        preferences?.xc_goal_target_distance_meters,
        goalDistanceUnit,
      ),
    );
    setGoalElevationDraft(
      metersToElevationInput(
        preferences?.xc_goal_target_elevation_gain_meters,
        goalElevationUnit,
      ),
    );
    setGoalFinishTimeDraft(
      formatTargetFinishTimeInput(
        preferences?.xc_goal_target_finish_time_seconds,
      ),
    );
    setGoalEventProfileDraft(preferences?.xc_goal_event_profile ?? "");
  }

  async function handleSaveGoal() {
    const parsedDistance = parseOptionalNumberInput(goalDistanceDraft);
    const parsedElevation = parseOptionalNumberInput(goalElevationDraft);
    const parsedFinishTimeHours = parseOptionalNumberInput(goalFinishTimeDraft);
    const hasEventDetails =
      goalEventNameDraft.trim().length > 0 ||
      goalEventProfileDraft !== "" ||
      parsedFinishTimeHours != null;

    if (
      !hasEventDetails &&
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

    if (parsedFinishTimeHours != null && parsedFinishTimeHours <= 0) {
      toast.error("Target finish time must be greater than zero hours.");
      return;
    }

    const targetDistanceMeters = distanceToMeters(
      parsedDistance,
      goalDistanceUnit,
    );
    const targetElevationGainMeters = elevationToMeters(
      parsedElevation,
      goalElevationUnit,
    );

    if (targetDistanceMeters > XC_GOAL_DISTANCE_MAX_METERS) {
      toast.error("Goal distance must be 500 mi / 805 km or less.");
      return;
    }

    if (targetElevationGainMeters > XC_GOAL_ELEVATION_MAX_METERS) {
      toast.error("Climbing target must be 25,000 ft / 7,620 m or less.");
      return;
    }

    try {
      const updatedPreferences = await updatePreferencesMutation.updateAsync(
        buildPreferencesPayload(preferencesQuery.data ?? null, {
          xcGoalEventName: goalEventNameDraft.trim() || null,
          xcGoalStartDate: goalStartDateDraft.trim(),
          xcGoalTargetDate: goalDateDraft.trim(),
          xcGoalTargetDistanceMeters: targetDistanceMeters,
          xcGoalTargetElevationGainMeters: targetElevationGainMeters,
          xcGoalTargetFinishTimeSeconds: finishTimeHoursToSeconds(
            parsedFinishTimeHours,
          ),
          xcGoalEventProfile: goalEventProfileDraft || null,
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
      setIsEditingGoal(false);
    } catch (error) {
      toast.error(extractApiMessage(error));
    }
  }

  async function handleClearGoal() {
    try {
      await updatePreferencesMutation.updateAsync(
        buildPreferencesPayload(preferencesQuery.data ?? null, {
          xcGoalEventName: null,
          xcGoalStartDate: null,
          xcGoalTargetDate: null,
          xcGoalTargetDistanceMeters: null,
          xcGoalTargetElevationGainMeters: null,
          xcGoalTargetFinishTimeSeconds: null,
          xcGoalEventProfile: null,
        }),
      );
      setGoalEventNameDraft("");
      setGoalStartDateDraft("");
      setGoalDateDraft("");
      setGoalDistanceDraft("");
      setGoalElevationDraft("");
      setGoalFinishTimeDraft("");
      setGoalEventProfileDraft("");
      setIsEditingGoal(false);
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

          <div className="flex flex-wrap items-center gap-x-4 gap-y-2 text-sm text-base-content/65">
            {eventGoal ? (
              <>
                {eventGoal.event_name ? (
                  <span className="font-medium text-base-content">
                    {eventGoal.event_name}
                  </span>
                ) : null}
                <span className="whitespace-nowrap">
                  Started {formatLongDate(eventGoal.start_date)}
                </span>
                <span className="whitespace-nowrap">
                  Target {formatLongDate(eventGoal.target_date)}
                </span>
                <span className="whitespace-nowrap">
                  {formatDaysRemaining(eventGoal.days_remaining)}
                </span>
              </>
            ) : (
              <span className="font-medium text-base-content">
                Set an event target
              </span>
            )}
            <span className="whitespace-nowrap">
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

        {!eventGoal ? (
          <div className="mt-6 grid gap-4 md:grid-cols-2 xl:grid-cols-4">
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
          </div>
        ) : null}
      </div>

      {eventGoal && progress.readiness ? (
        <ReadinessOverview
          readiness={progress.readiness}
          deficits={progress.deficits}
          unitSystem={unitSystem}
          goalDistanceUnit={goalDistanceUnit}
          goalElevationUnit={goalElevationUnit}
        />
      ) : null}

      {progress.race_results.length > 0 ? (
        <section className="space-y-4">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div>
              <h2 className="text-xl font-semibold text-base-content">
                Race result insights
              </h2>
              <p className="mt-1 text-sm text-base-content/70">
                Race-flagged activities are treated as outcomes and compared
                with the training rides that led into them.
              </p>
            </div>
            <span className="badge badge-outline whitespace-nowrap gap-2 px-3 py-2">
              {progress.race_results.length} race
              {progress.race_results.length === 1 ? "" : "s"}
            </span>
          </div>
          <div className="space-y-4">
            {progress.race_results.map((race) => (
              <RaceResultCard
                key={race.activity_id}
                race={race}
                goalDistanceUnit={goalDistanceUnit}
                goalElevationUnit={goalElevationUnit}
                unitSystem={unitSystem}
              />
            ))}
          </div>
        </section>
      ) : null}

      {!eventGoal ? (
        <div className="grid gap-4 xl:grid-cols-3">
          {progress.goals.map((goal) => (
            <GoalCard key={goal.key} goal={goal} unitSystem={unitSystem} />
          ))}
        </div>
      ) : null}

      <section className="rounded-box border border-base-300 bg-base-100 p-5 shadow-sm sm:p-6">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <h2 className="text-xl font-semibold text-base-content">
              Trends over time
            </h2>
            <p className="mt-1 text-sm text-base-content/70">
              {eventGoal
                ? `Weekly trends across the active training block, starting ${formatLongDate(eventGoal.start_date)}. Status compares recent weeks with the opening block.`
                : `Weekly trends over the last eight weeks. Status compares recent weeks with the opening block.`}
            </p>
          </div>
          <p className="whitespace-nowrap text-sm text-base-content/60">
            {weeklyChartData.length} week
            {weeklyChartData.length === 1 ? "" : "s"} tracked
          </p>
        </div>

        <div className="mt-5 grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          {trendSummaries.map((summary) => (
            <TrendSummaryItem key={summary.label} summary={summary} />
          ))}
        </div>

        <div className="mt-6 grid gap-6 xl:grid-cols-2">
          <div className="min-w-0 border-t border-base-300/70 pt-5">
            <div className="flex flex-wrap items-start justify-between gap-3">
              <div>
                <h3 className="font-semibold text-base-content">
                  Weekly volume trend
                </h3>
                <p className="mt-1 text-sm text-base-content/65">
                  Distance and elevation gain by week, without treating total
                  block mileage as event-day readiness.
                </p>
              </div>
              <div className="flex flex-wrap gap-x-4 gap-y-2 text-xs text-base-content/65">
                <span className="inline-flex items-center gap-2 whitespace-nowrap">
                  <span
                    className="inline-block h-2.5 w-2.5 rounded-full"
                    style={{ backgroundColor: DISTANCE_COLOR }}
                  />
                  Distance
                </span>
                <span className="inline-flex items-center gap-2 whitespace-nowrap">
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
              aria-label="XC weekly distance and climbing trend chart"
              className="mt-4 h-[300px] w-full"
            >
              <ResponsiveContainer
                width="100%"
                height="100%"
                minWidth={320}
                minHeight={300}
              >
                <ComposedChart
                  data={weeklyChartData}
                  margin={{ top: 8, right: 10, bottom: 8, left: 0 }}
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
                    tickFormatter={(value: number) =>
                      `${formatNumber(value, value >= 100 ? 0 : 1)} ${goalDistanceUnit}`
                    }
                  />
                  <YAxis
                    yAxisId="climbing"
                    orientation="right"
                    axisLine={false}
                    tick={{ fill: "var(--color-base-content)", fontSize: 10 }}
                    tickLine={false}
                    width={64}
                    tickFormatter={(value: number) =>
                      `${formatNumber(value, value >= 100 ? 0 : 1)} ${goalElevationUnit}`
                    }
                  />
                  <Tooltip
                    content={
                      <VolumeTrendTooltip
                        goalDistanceUnit={goalDistanceUnit}
                        goalElevationUnit={goalElevationUnit}
                      />
                    }
                  />
                  <Bar
                    dataKey="distanceChartValue"
                    fill={DISTANCE_COLOR}
                    fillOpacity={0.72}
                    radius={[5, 5, 0, 0]}
                    maxBarSize={28}
                  />
                  <Line
                    type="monotone"
                    dataKey="climbingGainChartValue"
                    yAxisId="climbing"
                    stroke={CLIMB_COLOR}
                    strokeWidth={3}
                    dot={{ r: 3, fill: CLIMB_COLOR, strokeWidth: 0 }}
                    activeDot={{ r: 5, fill: CLIMB_COLOR }}
                  />
                </ComposedChart>
              </ResponsiveContainer>
            </div>
          </div>

          <div className="min-w-0 border-t border-base-300/70 pt-5">
            <div className="flex flex-wrap items-start justify-between gap-3">
              <div>
                <h3 className="font-semibold text-base-content">
                  Durability trend
                </h3>
                <p className="mt-1 text-sm text-base-content/65">
                  Z2 speed, climbing vertical rate, and aerobic drift indexed to
                  the first usable week so the direction is easy to compare.
                </p>
              </div>
              <div className="flex flex-wrap gap-x-4 gap-y-2 text-xs text-base-content/65">
                <span className="inline-flex items-center gap-2 whitespace-nowrap">
                  <span
                    className="inline-block h-2.5 w-2.5 rounded-full"
                    style={{ backgroundColor: Z2_COLOR }}
                  />
                  Z2 speed
                </span>
                <span className="inline-flex items-center gap-2 whitespace-nowrap">
                  <span
                    className="inline-block h-2.5 w-2.5 rounded-full"
                    style={{ backgroundColor: CLIMB_COLOR }}
                  />
                  Climb rate
                </span>
                <span className="inline-flex items-center gap-2 whitespace-nowrap">
                  <span
                    className="inline-block h-2.5 w-2.5 rounded-full"
                    style={{ backgroundColor: DECOUPLING_COLOR }}
                  />
                  Decoupling
                </span>
              </div>
            </div>

            {hasPaceTrendData ? (
              <div
                role="img"
                aria-label="XC Z2 speed climbing rate and decoupling trend chart"
                className="mt-4 h-[300px] w-full"
              >
                <ResponsiveContainer
                  width="100%"
                  height="100%"
                  minWidth={320}
                  minHeight={300}
                >
                  <ComposedChart
                    data={weeklyChartData}
                    margin={{ top: 8, right: 10, bottom: 8, left: 0 }}
                  >
                    <CartesianGrid
                      vertical={false}
                      stroke="var(--color-base-content)"
                      strokeOpacity={0.1}
                    />
                    <XAxis
                      axisLine={false}
                      dataKey="label"
                      tick={{
                        fill: "var(--color-base-content)",
                        fontSize: 10,
                      }}
                      tickLine={false}
                      minTickGap={20}
                    />
                    <YAxis
                      axisLine={false}
                      tick={{
                        fill: "var(--color-base-content)",
                        fontSize: 10,
                      }}
                      tickLine={false}
                      width={50}
                      tickFormatter={(value: number) => `${value.toFixed(0)}`}
                    />
                    <Tooltip
                      content={
                        <PaceDurabilityTooltip
                          unitSystem={unitSystem}
                          goalElevationUnit={goalElevationUnit}
                        />
                      }
                    />
                    <ReferenceLine
                      y={100}
                      stroke="var(--color-base-content)"
                      strokeDasharray="4 4"
                      strokeOpacity={0.25}
                    />
                    <Line
                      type="monotone"
                      dataKey="averageZ2SpeedIndex"
                      stroke={Z2_COLOR}
                      strokeWidth={3}
                      dot={{ r: 3, fill: Z2_COLOR, strokeWidth: 0 }}
                      activeDot={{ r: 5, fill: Z2_COLOR }}
                      connectNulls
                    />
                    <Line
                      type="monotone"
                      dataKey="climbingVerticalRateIndex"
                      stroke={CLIMB_COLOR}
                      strokeWidth={3}
                      dot={{ r: 3, fill: CLIMB_COLOR, strokeWidth: 0 }}
                      activeDot={{ r: 5, fill: CLIMB_COLOR }}
                      connectNulls
                    />
                    <Line
                      type="monotone"
                      dataKey="aerobicDecouplingIndex"
                      stroke={DECOUPLING_COLOR}
                      strokeWidth={3}
                      dot={{ r: 3, fill: DECOUPLING_COLOR, strokeWidth: 0 }}
                      activeDot={{ r: 5, fill: DECOUPLING_COLOR }}
                      connectNulls
                    />
                  </ComposedChart>
                </ResponsiveContainer>
              </div>
            ) : (
              <EmptyTrendState message="Repeat comparable endurance rides with heart-rate data to unlock Z2 speed, climb-rate, and decoupling trends." />
            )}
          </div>
        </div>

        <div className="mt-6 border-t border-base-300/70 pt-5">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div>
              <h3 className="font-semibold text-base-content">
                Time in zones
              </h3>
              <p className="mt-1 text-sm text-base-content/65">
                Weekly heart-rate zone mix shows whether the block is mostly
                aerobic or drifting into too much intensity.
              </p>
            </div>
            <div className="flex flex-wrap gap-x-4 gap-y-2 text-xs text-base-content/65">
              {(["z1", "z2", "z3", "z4", "z5"] as const).map((zone) => (
                <span
                  key={zone}
                  className="inline-flex items-center gap-2 whitespace-nowrap uppercase"
                >
                  <span
                    className="inline-block h-2.5 w-2.5 rounded-full"
                    style={{ backgroundColor: ZONE_COLORS[zone] }}
                  />
                  {zone}
                </span>
              ))}
            </div>
          </div>

          {hasZoneTrendData ? (
            <div
              role="img"
              aria-label="XC weekly time in zones chart"
              className="mt-4 h-[280px] w-full"
            >
              <ResponsiveContainer
                width="100%"
                height="100%"
                minWidth={320}
                minHeight={280}
              >
                <ComposedChart
                  data={weeklyChartData}
                  margin={{ top: 8, right: 10, bottom: 8, left: 0 }}
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
                    width={48}
                    tickFormatter={(value: number) => `${value.toFixed(1)}h`}
                  />
                  <Tooltip content={<ZoneTrendTooltip />} />
                  <Bar
                    dataKey="z1Hours"
                    stackId="zones"
                    fill={ZONE_COLORS.z1}
                    fillOpacity={0.86}
                    maxBarSize={34}
                  />
                  <Bar
                    dataKey="z2ZoneHours"
                    stackId="zones"
                    fill={ZONE_COLORS.z2}
                    fillOpacity={0.86}
                    maxBarSize={34}
                  />
                  <Bar
                    dataKey="z3Hours"
                    stackId="zones"
                    fill={ZONE_COLORS.z3}
                    fillOpacity={0.86}
                    maxBarSize={34}
                  />
                  <Bar
                    dataKey="z4Hours"
                    stackId="zones"
                    fill={ZONE_COLORS.z4}
                    fillOpacity={0.86}
                    maxBarSize={34}
                  />
                  <Bar
                    dataKey="z5Hours"
                    stackId="zones"
                    fill={ZONE_COLORS.z5}
                    fillOpacity={0.86}
                    radius={[5, 5, 0, 0]}
                    maxBarSize={34}
                  />
                </ComposedChart>
              </ResponsiveContainer>
            </div>
          ) : (
            <EmptyTrendState message="Save heart-rate zones on Account and regenerate older rides to populate weekly time-in-zone history." />
          )}
        </div>
      </section>

      <div className="grid gap-6 xl:grid-cols-[minmax(0,0.85fr)_minmax(0,1.2fr)]">
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
                unitSystem={unitSystem}
                goalDistanceUnit={goalDistanceUnit}
                goalElevationUnit={goalElevationUnit}
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
            <p className="whitespace-nowrap text-sm text-base-content/60">
              {progress.recent_rides.length} ride
              {progress.recent_rides.length === 1 ? "" : "s"} available
            </p>
          </div>

          <div
            className="max-h-[420px] overflow-auto rounded-box border border-base-300"
            aria-label="XC recent rides table"
          >
            <table className="table table-sm">
              <thead>
                <tr>
                  <th>Ride</th>
                  <th>Focus</th>
                  <th>Useful for</th>
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
                {visibleRideBenchmarks.map((ride) => (
                  <tr key={ride.activity_id}>
                    <td className="whitespace-nowrap">
                      <div className="space-y-1">
                        <Link
                          href={`/activities/${ride.activity_id}`}
                          className="link link-primary link-hover whitespace-nowrap font-medium no-underline"
                          title={ride.activity_title}
                        >
                          {ride.activity_title}
                        </Link>
                        <div className="whitespace-nowrap text-xs text-base-content/55">
                          {formatRouteFamily(ride.route_family_key)}
                        </div>
                      </div>
                    </td>
                    <td>
                      <span className={rideFocusBadgeClass(ride.ride_focus)}>
                        {formatRideFocusLabel(ride.ride_focus)}
                      </span>
                    </td>
                    <td className="whitespace-nowrap">
                      <span
                        className="tooltip tooltip-left"
                        data-tip={ride.training_purpose_detail}
                      >
                        <span
                          className={trainingPurposeBadgeClass(
                            ride.training_purpose,
                          )}
                          title={ride.training_purpose_detail}
                        >
                          {formatTrainingPurposeLabel(ride.training_purpose)}
                        </span>
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
          <div className="mt-4 flex flex-wrap items-center justify-between gap-3 text-sm text-base-content/65">
            <p>
              Showing{" "}
              {progress.recent_rides.length > 0 ? rideBenchmarkStartIndex + 1 : 0}
              -
              {Math.min(
                rideBenchmarkStartIndex + RIDE_BENCHMARK_PAGE_SIZE,
                progress.recent_rides.length,
              )}{" "}
              of {progress.recent_rides.length}
            </p>
            <div className="join">
              <button
                type="button"
                className="btn btn-sm join-item"
                disabled={rideBenchmarkPage === 0}
                onClick={() =>
                  setRideBenchmarkPage((currentPage) =>
                    Math.max(currentPage - 1, 0),
                  )
                }
              >
                Previous
              </button>
              <span className="btn btn-sm btn-disabled join-item text-base-content/70">
                {rideBenchmarkPage + 1} / {rideBenchmarkTotalPages}
              </span>
              <button
                type="button"
                className="btn btn-sm join-item"
                disabled={rideBenchmarkPage >= rideBenchmarkTotalPages - 1}
                onClick={() =>
                  setRideBenchmarkPage((currentPage) =>
                    Math.min(currentPage + 1, rideBenchmarkTotalPages - 1),
                  )
                }
              >
                Next
              </button>
            </div>
          </div>
        </section>
      </div>

      <section className="rounded-box border border-base-300 bg-base-100 p-5 shadow-sm sm:p-6">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <h2 className="text-xl font-semibold text-base-content">
              Event target
            </h2>
            <p className="mt-1 text-sm text-base-content/70">
              This target drives the readiness checks. Keep it stable unless
              your race, date, course distance, or climbing demand changes.
            </p>
          </div>
          {!isEditingGoal ? (
            <button
              type="button"
              className="btn btn-outline btn-sm"
              onClick={() => {
                resetGoalDraftsFromPreferences(preferencesQuery.data ?? null);
                setIsEditingGoal(true);
              }}
            >
              {eventGoal ? "Edit target" : "Set target"}
            </button>
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

        {!isEditingGoal ? (
          eventGoal ? (
            <div className="mt-5 space-y-4">
              <dl className="grid gap-4 text-sm text-base-content/70 sm:grid-cols-2 xl:grid-cols-4">
                <TargetFact label="Event" value={eventGoal.event_name} />
                <TargetFact
                  label="Profile"
                  value={formatEventProfileLabel(eventGoal.event_profile)}
                />
                <TargetFact
                  label="Training start"
                  value={formatLongDate(eventGoal.start_date)}
                />
                <TargetFact
                  label="Target date"
                  value={formatLongDate(eventGoal.target_date)}
                />
                <TargetFact
                  label="Time left"
                  value={formatDaysRemaining(eventGoal.days_remaining)}
                />
                <TargetFact
                  label="Qualifying rides"
                  value={`${eventGoal.counted_ride_count}`}
                />
                <TargetFact
                  label="Distance"
                  value={formatGoalDistance(
                    eventGoal.target_distance_meters,
                    goalDistanceUnit,
                  )}
                />
                <TargetFact
                  label="Climbing"
                  value={formatGoalElevation(
                    eventGoal.target_elevation_gain_meters,
                    goalElevationUnit,
                  )}
                />
                <TargetFact
                  label="Target finish"
                  value={
                    eventGoal.target_finish_time_seconds
                      ? formatDuration(eventGoal.target_finish_time_seconds)
                      : null
                  }
                />
                <TargetFact
                  label="Target pace"
                  value={
                    eventGoal.target_finish_speed_mps
                      ? formatSpeed(
                          eventGoal.target_finish_speed_mps,
                          unitSystem,
                        )
                      : null
                  }
                />
                <TargetFact
                  label="Target density"
                  value={targetReadMetrics?.targetClimbDensity}
                />
                <TargetFact
                  label="Current density"
                  value={targetReadMetrics?.currentClimbDensity}
                />
              </dl>

              <p className="text-sm leading-6 text-base-content/60">
                Training block: {formatLongDate(eventGoal.start_date)} to{" "}
                {formatLongDate(eventGoal.target_date)} (
                {eventGoal.training_window_days} days). This target is the
                course demand model used by Quick status, Trends, and Next ride
                guidance.
              </p>
            </div>
          ) : (
            <div className="mt-5 rounded-box border border-dashed border-base-300 bg-base-200/60 p-4 text-sm leading-6 text-base-content/70">
              No event target is saved. Add one when you want `/xc` to compare
              recent rides against a specific race distance, climbing demand,
              date, and optional finish-time goal.
            </div>
          )
        ) : (
          <>
            <div className="mt-5 grid gap-4 md:grid-cols-2 xl:grid-cols-6">
              <label className="form-control gap-2 xl:col-span-2">
                <span className="label-text font-medium">Event name</span>
                <input
                  type="text"
                  className="input input-bordered"
                  value={goalEventNameDraft}
                  onChange={(event) =>
                    setGoalEventNameDraft(event.target.value)
                  }
                  placeholder="Lumberjack 100"
                />
              </label>

              <label className="form-control gap-2 xl:col-span-2">
                <span className="label-text font-medium">Event profile</span>
                <select
                  className="select select-bordered"
                  value={goalEventProfileDraft}
                  onChange={(event) =>
                    setGoalEventProfileDraft(
                      event.target.value as XcEventProfile | "",
                    )
                  }
                >
                  <option value="">No profile</option>
                  <option value="xc_marathon">XC marathon</option>
                  <option value="technical_singletrack">
                    Technical singletrack
                  </option>
                  <option value="endurance_mtb">Endurance MTB</option>
                  <option value="ultra_mtb">Ultra MTB</option>
                  <option value="custom">Custom</option>
                </select>
              </label>

              <label className="form-control gap-2 xl:col-span-2">
                <span className="label-text font-medium">
                  Target finish time
                </span>
                <div className="join w-full">
                  <input
                    type="number"
                    min="0"
                    step="0.1"
                    className="input input-bordered join-item w-full"
                    value={goalFinishTimeDraft}
                    onChange={(event) =>
                      setGoalFinishTimeDraft(event.target.value)
                    }
                    placeholder="12"
                  />
                  <span className="btn btn-disabled join-item w-20 border-base-300 bg-base-200 text-base-content/65">
                    hrs
                  </span>
                </div>
              </label>

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

              <label className="form-control gap-2 xl:col-span-2">
                <span className="label-text font-medium">Distance target</span>
                <div className="join w-full">
                  <input
                    type="number"
                    min="0"
                    max={
                      goalDistanceUnit === "mi"
                        ? XC_GOAL_DISTANCE_MAX_MILES
                        : Math.round(XC_GOAL_DISTANCE_MAX_KILOMETERS)
                    }
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

              <label className="form-control gap-2 xl:col-span-2">
                <span className="label-text font-medium">Climbing target</span>
                <div className="join w-full">
                  <input
                    type="number"
                    min="0"
                    max={
                      goalElevationUnit === "ft"
                        ? XC_GOAL_ELEVATION_MAX_FEET
                        : Math.round(XC_GOAL_ELEVATION_MAX_METERS)
                    }
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
                  : "Save target"}
              </button>
              <button
                type="button"
                className="btn btn-outline"
                disabled={updatePreferencesMutation.isPending}
                onClick={() => {
                  resetGoalDraftsFromPreferences(
                    preferencesQuery.data ?? null,
                  );
                  setIsEditingGoal(false);
                }}
              >
                Cancel
              </button>
              {eventGoal ? (
                <button
                  type="button"
                  className="btn btn-ghost text-error"
                  disabled={updatePreferencesMutation.isPending}
                  onClick={handleClearGoal}
                >
                  Clear target
                </button>
              ) : null}
            </div>
          </>
        )}
      </section>
    </section>
  );
}
