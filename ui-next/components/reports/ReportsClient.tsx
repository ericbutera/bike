"use client";

import { faDownload } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { useRouter, useSearchParams } from "next/navigation";
import { useEffect, useRef, useState } from "react";
import toast from "react-hot-toast";
import {
  formatElevation,
  formatElevationRate,
  formatSpeed as formatActivitySpeed,
  type UnitSystem,
} from "../../lib/activityFormatting";
import {
  useRideSummaryReport,
  useTrainingReportDefinitions,
  useTrainingReports,
  useUserPreferences,
  type ClimbingReport,
  type CompareRideCandidate,
  type CompareRidesReport,
  type EnduranceReport,
  type FatigueReport,
  type HourlyDurability,
  type ReassessmentAbilityEstimate,
  type ReassessmentSignal,
  type ReassessmentReport,
  type ReassessmentTarget,
  type ReassessmentTargetSource,
  type ReassessmentVerdict,
  type ReassessmentWindow,
  type RideSummaryReport,
  type TrainingReportBoundary,
  type TrainingReportPoint,
} from "../../lib/queries";
import { useUnitPreferences } from "../../lib/unitPreferences";
import InfoTooltip from "../ui/InfoTooltip";
import Charts, { chartBucketLabel } from "./Charts";
import TimeRangeSelector, {
  TIME_RANGE_OPTIONS,
  TimeRange,
} from "./TimeRangeSelector";
import {
  DEFAULT_REPORT_ID,
  findReportDefinition,
  toReportDefinitions,
  type ReportDefinition,
  type ReportId,
} from "./reportDefinitions";

const REPORTS_HELP_TEXT =
  "Generate ad-hoc ride analysis for the selected range.";
const CLIMBS_PER_PAGE = 10;
const DEFAULT_REPORT_RANGE: TimeRange = "week";

export default function ReportsClient() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const { unitSystem } = useUnitPreferences();
  const reportRequestLockRef = useRef(false);
  const initialRange = parseTimeRange(searchParams.get("range"));
  const [range, setRange] = useState<TimeRange>(initialRange);
  const [selectedReportId, setSelectedReportId] = useState<ReportId>(
    findReportDefinition(searchParams.get("report")).id,
  );
  const [selectedActivityIds, setSelectedActivityIds] = useState<number[]>(
    parseActivityIds(searchParams.get("activity_ids")),
  );
  const [isReportDrawerOpen, setIsReportDrawerOpen] = useState(false);
  const definitionsQuery = useTrainingReportDefinitions();
  const reportDefinitions = toReportDefinitions(definitionsQuery.data?.reports);
  const selectedReport = findReportDefinition(
    selectedReportId,
    reportDefinitions,
  );
  const isAggregateTrends = selectedReport.id === "aggregate_trends";
  const focusedBucketReport = focusedBucketReportFor(selectedReport.id);
  const isBucketReport = isAggregateTrends || focusedBucketReport != null;
  const isRideSummary = selectedReport.id === "ride_summary";
  const isReassessment = selectedReport.id === "reassessment";
  const standaloneReportId = standaloneReportIdFor(selectedReport.id);
  const isStandaloneReport = standaloneReportId != null;
  const preferencesQuery = useUserPreferences({ enabled: isReassessment });
  const todayDate = formatDateInput(new Date());
  const intervalDateRange = dateRangeForTimeRange(range);
  const startDate = intervalDateRange.startDate;
  const endDate = intervalDateRange.endDate;
  const reassessmentStartDate =
    preferencesQuery.data.xc_goal_start_date ?? undefined;
  const reportStartDate = isReassessment
    ? (reassessmentStartDate ?? todayDate)
    : startDate;
  const reportEndDate = isReassessment ? todayDate : endDate;
  const reportBoundary = reportBoundaryForTimeRange(range);
  const canonicalQuery = canonicalReportQuery({
    range,
    selectedActivityIds,
    selectedReportId,
    isReassessment,
  });
  const isCanonicalQuery = canonicalQuery === searchParams.toString();
  const {
    data,
    isLoading,
    isFetching,
    isError: isReportError,
    error: reportError,
  } = useTrainingReports(reportBoundary, {
    enabled: isBucketReport && isCanonicalQuery,
    startDate: startDate || undefined,
    endDate,
  });
  const reportQuery = useRideSummaryReport({
    report: standaloneReportId ?? "ride_summary",
    boundary: reportBoundary,
    startDate: reportStartDate || undefined,
    endDate: reportEndDate,
    activityIds: selectedActivityIds,
    enabled:
      isStandaloneReport &&
      isCanonicalQuery &&
      (!isReassessment || !!reassessmentStartDate),
  });

  const points = data?.points ?? [];
  const reportErrorMessage = reportGenerationErrorMessage(reportError);
  const standaloneReportErrorMessage = reportGenerationErrorMessage(
    reportQuery.error,
  );
  const isReportRequestActive =
    !isCanonicalQuery ||
    (isBucketReport && (isLoading || isFetching)) ||
    (isStandaloneReport && (reportQuery.isLoading || reportQuery.isFetching));

  useEffect(() => {
    reportRequestLockRef.current = isReportRequestActive;
  }, [isReportRequestActive]);

  useEffect(() => {
    if (
      !isReportError ||
      !reportError ||
      !isBucketReport ||
      !isReportGenerationInProgressError(reportError, reportErrorMessage)
    ) {
      return;
    }

    toast.error(reportErrorMessage, {
      id: "report-generation-in-progress",
      duration: 7000,
    });
  }, [isBucketReport, isReportError, reportError, reportErrorMessage]);

  useEffect(() => {
    if (
      !reportQuery.isError ||
      !reportQuery.error ||
      !isStandaloneReport ||
      !isReportGenerationInProgressError(
        reportQuery.error,
        standaloneReportErrorMessage,
      )
    ) {
      return;
    }

    toast.error(standaloneReportErrorMessage, {
      id: "report-generation-in-progress",
      duration: 7000,
    });
  }, [
    isStandaloneReport,
    reportQuery.error,
    reportQuery.isError,
    standaloneReportErrorMessage,
  ]);

  useEffect(() => {
    if (canonicalQuery !== searchParams.toString()) {
      router.replace(`/training/reports?${canonicalQuery}`, { scroll: false });
    }
  }, [canonicalQuery, router, searchParams]);

  function handleRangeChange(nextRange: TimeRange) {
    if (nextRange === range) {
      return;
    }

    if (isReportRequestActive || reportRequestLockRef.current) {
      showReportRequestInProgressToast();
      return;
    }

    reportRequestLockRef.current = true;
    setRange(nextRange);
  }

  function handleReportSelect(reportId: ReportId) {
    if (reportId === selectedReportId) {
      setIsReportDrawerOpen(false);
      return;
    }

    if (isReportRequestActive || reportRequestLockRef.current) {
      showReportRequestInProgressToast();
      setIsReportDrawerOpen(false);
      return;
    }

    reportRequestLockRef.current = true;
    setRange(DEFAULT_REPORT_RANGE);
    setSelectedReportId(reportId);
    setIsReportDrawerOpen(false);
  }

  function handleSelectedActivityIdsChange(activityIds: number[]) {
    if (isReportRequestActive || reportRequestLockRef.current) {
      showReportRequestInProgressToast();
      return;
    }

    reportRequestLockRef.current = true;
    setSelectedActivityIds(activityIds);
  }

  return (
    <div className="drawer min-h-[calc(100vh-4rem)] lg:drawer-open">
      <input
        id="reports-drawer"
        type="checkbox"
        className="drawer-toggle"
        checked={isReportDrawerOpen}
        onChange={(event) => {
          setIsReportDrawerOpen(event.target.checked);
        }}
      />
      <div className="drawer-content min-w-0">
        <div className="min-w-0 px-4 py-5 sm:px-6 lg:px-8 xl:px-10">
          <div className="grid w-full gap-6">
            <div className="flex flex-wrap items-end justify-between gap-4">
              <div className="flex items-center gap-3">
                <label
                  htmlFor="reports-drawer"
                  className="btn btn-square btn-ghost lg:hidden"
                  aria-label="Open reports drawer"
                >
                  <svg
                    xmlns="http://www.w3.org/2000/svg"
                    viewBox="0 0 24 24"
                    strokeLinejoin="round"
                    strokeLinecap="round"
                    strokeWidth="2"
                    fill="none"
                    stroke="currentColor"
                    className="size-5"
                    aria-hidden="true"
                  >
                    <path d="M4 4m0 2a2 2 0 0 1 2 -2h12a2 2 0 0 1 2 2v12a2 2 0 0 1 -2 2h-12a2 2 0 0 1 -2 -2z" />
                    <path d="M9 4v16" />
                    <path d="M14 10l2 2l-2 2" />
                  </svg>
                </label>
                <div className="flex items-center gap-2">
                  <h1 className="text-2xl font-semibold">Reports</h1>
                  <InfoTooltip
                    label="Reports details"
                    tip={REPORTS_HELP_TEXT}
                  />
                </div>
              </div>
            </div>

            {isReassessment &&
            !reassessmentStartDate &&
            !preferencesQuery.isLoading ? (
              <div className="alert">
                <span>
                  Save an XC training start date before generating a
                  reassessment.
                </span>
              </div>
            ) : null}

            {focusedBucketReport ? (
              <FocusedBucketReportView
                report={selectedReport}
                reportType={focusedBucketReport}
                points={points}
                range={range}
                onRangeChange={handleRangeChange}
                isLoading={isLoading}
                isFetching={isFetching}
                isError={isReportError}
                errorMessage={reportErrorMessage}
              />
            ) : (
              <section className="min-w-0 rounded-lg border border-base-300 bg-base-100 p-5">
                <div className="flex flex-wrap items-start justify-between gap-4">
                  <div>
                    <div className="flex items-center gap-2">
                      <h2 className="text-lg font-semibold">
                        {selectedReport.name}
                      </h2>
                      <InfoTooltip
                        label={`${selectedReport.name} details`}
                        tip={selectedReport.purpose}
                      />
                    </div>
                  </div>
                  {!isReassessment ? (
                    <form
                      className="min-w-52"
                      onSubmit={(event) => event.preventDefault()}
                    >
                      <TimeRangeSelector
                        value={range}
                        onChange={handleRangeChange}
                      />
                    </form>
                  ) : null}
                </div>

                {isAggregateTrends ? (
                  <AggregateTrendsReport
                    points={points}
                    range={range}
                    isLoading={isLoading}
                    isFetching={isFetching}
                  />
                ) : isRideSummary ? (
                  <RideSummaryReportView
                    summary={reportQuery.data?.ride_summary ?? null}
                    startDate={startDate}
                    endDate={endDate}
                    isLoading={reportQuery.isLoading}
                    isFetching={reportQuery.isFetching}
                  />
                ) : selectedReport.id === "endurance" ? (
                  <EnduranceReportView
                    report={reportQuery.data?.endurance ?? null}
                    unitSystem={unitSystem}
                    isLoading={reportQuery.isLoading}
                  />
                ) : selectedReport.id === "climbing" ? (
                  <ClimbingReportView
                    report={reportQuery.data?.climbing ?? null}
                    unitSystem={unitSystem}
                    isLoading={reportQuery.isLoading}
                  />
                ) : selectedReport.id === "fatigue" ? (
                  <FatigueReportView
                    report={reportQuery.data?.fatigue ?? null}
                    unitSystem={unitSystem}
                    isLoading={reportQuery.isLoading}
                  />
                ) : selectedReport.id === "compare_rides" ? (
                  <CompareRidesReportView
                    report={reportQuery.data?.compare_rides ?? null}
                    selectedActivityIds={selectedActivityIds}
                    onSelectedActivityIdsChange={
                      handleSelectedActivityIdsChange
                    }
                    isLoading={reportQuery.isLoading}
                  />
                ) : selectedReport.id === "reassessment" ? (
                  <ReassessmentReportView
                    report={reportQuery.data?.reassessment ?? null}
                    isLoading={reportQuery.isLoading}
                  />
                ) : (
                  <PlannedReport report={selectedReport} />
                )}
              </section>
            )}
          </div>
        </div>
      </div>

      <div className="drawer-side z-40">
        <label
          htmlFor="reports-drawer"
          aria-label="Close reports drawer"
          className="drawer-overlay"
        />
        <aside className="min-h-full w-80 max-w-[85vw] border-r border-base-300 bg-base-100">
          <ReportMenu
            selectedReportId={selectedReport.id}
            reports={reportDefinitions}
            onSelect={handleReportSelect}
          />
        </aside>
      </div>
    </div>
  );
}

function RideSummaryReportView({
  summary,
  startDate,
  endDate,
  isLoading,
  isFetching,
}: {
  summary: RideSummaryReport | null;
  startDate: string;
  endDate: string;
  isLoading: boolean;
  isFetching: boolean;
}) {
  if (isLoading) {
    return (
      <div className="alert mt-4">
        <span>Generating ride summary...</span>
      </div>
    );
  }

  if (!summary || summary.activity_count === 0) {
    return (
      <div className="alert mt-4">
        <span>
          No rides found from {startDate} through {endDate}.
        </span>
      </div>
    );
  }

  const zoneTotal =
    summary.z1_seconds +
    summary.z2_seconds +
    summary.z3_seconds +
    summary.z4_seconds +
    summary.z5_seconds;

  return (
    <div className="mt-5 grid gap-5">
      <div className="flex flex-wrap items-center justify-between gap-3 text-sm text-base-content/60">
        <span>
          {summary.activity_count} rides from {startDate} through {endDate}
        </span>
        {isFetching ? <span>Refreshing...</span> : null}
      </div>

      <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        <ReportMetricCard
          label="Distance"
          value={formatNumber(summary.total_distance_miles)}
          unit="mi"
        />
        <ReportMetricCard
          label="Elevation"
          value={formatNumber(summary.total_elevation_gain_feet)}
          unit="ft"
        />
        <ReportMetricCard
          label="Moving time"
          value={formatDuration(summary.total_moving_seconds)}
        />
        <ReportMetricCard
          label="Stopped time"
          value={formatDuration(summary.total_stopped_seconds)}
        />
        <ReportMetricCard
          label="Avg speed"
          value={formatOptionalNumber(summary.average_speed_mph)}
          unit={summary.average_speed_mph == null ? undefined : "mph"}
        />
        <ReportMetricCard
          label="Avg HR"
          value={formatOptionalNumber(summary.average_heart_rate_bpm)}
          unit={summary.average_heart_rate_bpm == null ? undefined : "bpm"}
        />
        <ReportMetricCard
          label="Max HR"
          value={summary.max_heart_rate_bpm?.toString() ?? "n/a"}
          unit={summary.max_heart_rate_bpm == null ? undefined : "bpm"}
        />
        <ReportMetricCard
          label="Climb density"
          value={formatOptionalNumber(summary.climbing_density_feet_per_hour)}
          unit={
            summary.climbing_density_feet_per_hour == null ? undefined : "ft/h"
          }
        />
      </div>

      <section className="rounded-lg border border-base-300 p-4">
        <h3 className="text-base font-semibold">Heart Rate Zones</h3>
        <div className="mt-4">
          <div
            className="flex h-8 overflow-hidden rounded border border-base-300 bg-base-200"
            aria-label="Heart-rate zone distribution"
          >
            {heartRateZoneRows(summary).map((zone) => (
              <div
                key={zone.label}
                className={zone.color}
                style={{
                  width:
                    zoneTotal > 0
                      ? `${(zone.seconds / zoneTotal) * 100}%`
                      : "20%",
                }}
                title={`${zone.label}: ${formatDuration(zone.seconds)} (${formatZonePercent(zone.seconds, zoneTotal)})`}
              />
            ))}
          </div>
          <div className="mt-3 grid gap-2 sm:grid-cols-5">
            {heartRateZoneRows(summary).map((zone) => (
              <div key={zone.label} className="min-w-0 text-sm">
                <div className="flex items-center gap-2">
                  <span className={`h-2.5 w-2.5 rounded-sm ${zone.color}`} />
                  <span className="font-medium">{zone.label}</span>
                </div>
                <div className="mt-1 text-xs text-base-content/60">
                  {formatZonePercent(zone.seconds, zoneTotal)} ·{" "}
                  {formatDuration(zone.seconds)}
                </div>
              </div>
            ))}
          </div>
        </div>
      </section>

      {summary.data_quality_flags.length > 0 ? (
        <div className="alert alert-warning">
          <div>
            <div className="font-medium">Data quality</div>
            <ul className="mt-1 list-disc pl-5 text-sm">
              {summary.data_quality_flags.map((flag) => (
                <li key={flag}>{flag}</li>
              ))}
            </ul>
          </div>
        </div>
      ) : null}
    </div>
  );
}

function ReportMetricCard({
  label,
  value,
  unit,
  guide,
}: {
  label: string;
  value: string;
  unit?: string;
  guide?: string;
}) {
  return (
    <div className="rounded-lg border border-base-300 bg-base-200/40 p-4">
      <div className="text-xs font-medium uppercase text-base-content/50">
        {label}
      </div>
      <div className="mt-2 flex items-baseline gap-2">
        <span className="text-2xl font-semibold">{value}</span>
        {unit ? (
          <span className="text-sm text-base-content/60">{unit}</span>
        ) : null}
      </div>
      {guide ? (
        <div className="mt-2 text-xs leading-4 text-base-content/55">
          {guide}
        </div>
      ) : null}
    </div>
  );
}

function MetricHeader({
  label,
  unit,
  tip,
}: {
  label: string;
  unit?: string;
  tip?: string;
}) {
  return (
    <div className="whitespace-nowrap">
      <div className="flex items-center gap-1 font-semibold">
        {label}
        {unit ? (
          <span className="font-normal text-base-content/60">({unit})</span>
        ) : null}
        {tip ? (
          <InfoTooltip label={`${label} details`} tip={tip} position="bottom" />
        ) : null}
      </div>
    </div>
  );
}

function heartRateZoneRows(summary: RideSummaryReport) {
  return [
    {
      label: "Z1",
      seconds: summary.z1_seconds,
      color: "bg-violet-500",
    },
    {
      label: "Z2",
      seconds: summary.z2_seconds,
      color: "bg-emerald-500",
    },
    {
      label: "Z3",
      seconds: summary.z3_seconds,
      color: "bg-amber-400",
    },
    {
      label: "Z4",
      seconds: summary.z4_seconds,
      color: "bg-rose-500",
    },
    {
      label: "Z5",
      seconds: summary.z5_seconds,
      color: "bg-sky-500",
    },
  ];
}

function EnduranceReportView({
  report,
  unitSystem,
  isLoading,
}: {
  report: EnduranceReport | null;
  unitSystem: UnitSystem;
  isLoading: boolean;
}) {
  if (isLoading)
    return <LoadingReport label="Generating endurance report..." />;
  if (!report || report.activity_count === 0)
    return <EmptyReport label="No rides with usable point data found." />;

  return (
    <div className="mt-5 grid gap-5">
      <div className="grid gap-3 sm:grid-cols-3">
        <ReportMetricCard
          label="Median decoupling"
          value={formatOptionalNumber(report.median_aerobic_decoupling_percent)}
          unit={
            report.median_aerobic_decoupling_percent == null ? undefined : "%"
          }
          guide="<5 good; >10 fade"
        />
        <ReportMetricCard
          label="Median late fade"
          value={formatOptionalNumber(report.median_late_speed_change_percent)}
          unit={
            report.median_late_speed_change_percent == null ? undefined : "%"
          }
          guide="closer to 0 is better"
        />
        <ReportMetricCard
          label="Median fatigue"
          value={formatOptionalNumber(report.median_fatigue_index)}
          unit={report.median_fatigue_index == null ? undefined : "/100"}
          guide="<15 steady; 25+ fade"
        />
      </div>
      <div className="overflow-x-auto">
        <table className="table table-zebra">
          <thead>
            <tr>
              <th>
                <MetricHeader label="Ride" />
              </th>
              <th>
                <MetricHeader label="Duration" tip="Elapsed ride duration." />
              </th>
              <th>
                <MetricHeader
                  label="Drift"
                  unit="%"
                  tip="Aerobic decoupling: second-half efficiency compared with first-half efficiency. Lower is better; under 5% is good, over 10% suggests fade. Negative means efficiency improved in the second half."
                />
              </th>
              <th>
                <MetricHeader
                  label="Eff 1H"
                  tip="First-half efficiency: average speed divided by average heart rate. Unit is speed per bpm. Higher is better, but compare similar terrain."
                />
              </th>
              <th>
                <MetricHeader
                  label="Eff 2H"
                  tip="Second-half efficiency: average speed divided by average heart rate. Unit is speed per bpm. Higher is better, but compare similar terrain."
                />
              </th>
              <th>
                <MetricHeader
                  label="Late speed"
                  unit="%"
                  tip="Late-ride speed change versus the early steady portion. Near 0% is steady; negative means fading; positive can mean finishing faster or easier terrain."
                />
              </th>
              <th>
                <MetricHeader
                  label="Late HR"
                  unit="%"
                  tip="Late-ride heart-rate change versus the early steady portion. Use with late speed: slower speed at similar or higher HR is fatigue evidence."
                />
              </th>
              <th>
                <MetricHeader
                  label="Fatigue"
                  unit="/100"
                  tip="Deterministic fade score from hourly efficiency, speed, climb rate, and stops. Under 15 is steady; 25+ is a fade signal; higher is worse."
                />
              </th>
              <th>
                <MetricHeader
                  label="Hours"
                  tip="Number of hourly rows available for the ride."
                />
              </th>
            </tr>
          </thead>
          <tbody>
            {report.rides.map((ride) => (
              <tr key={ride.activity_id}>
                <td>{ride.title}</td>
                <td>{formatDuration(ride.elapsed_seconds)}</td>
                <td>{formatPercent(ride.aerobic_decoupling_percent)}</td>
                <td>
                  {formatOptionalNumber(ride.first_half_efficiency_mps_per_bpm)}
                </td>
                <td>
                  {formatOptionalNumber(
                    ride.second_half_efficiency_mps_per_bpm,
                  )}
                </td>
                <td>{formatPercent(ride.late_speed_change_percent)}</td>
                <td>{formatPercent(ride.late_heart_rate_change_percent)}</td>
                <td>{formatFatigueIndex(ride.fatigue_index)}</td>
                <td>{ride.hourly.length}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div className="grid gap-3">
        {report.rides.map((ride) => (
          <details
            key={ride.activity_id}
            className="rounded-lg border border-base-300 p-4"
          >
            <summary className="cursor-pointer text-base font-semibold">
              Hourly durability: {ride.title}
            </summary>
            <HourlyTable rows={ride.hourly} unitSystem={unitSystem} />
          </details>
        ))}
      </div>
    </div>
  );
}

function ClimbingReportView({
  report,
  unitSystem,
  isLoading,
}: {
  report: ClimbingReport | null;
  unitSystem: UnitSystem;
  isLoading: boolean;
}) {
  const [climbPage, setClimbPage] = useState(1);

  useEffect(() => {
    setClimbPage(1);
  }, [report?.climb_count]);

  if (isLoading) return <LoadingReport label="Generating climbing report..." />;
  if (!report || report.climb_count === 0)
    return <EmptyReport label="No sustained climbs found." />;

  const summaryRows = [
    ["Longest climb", report.longest_climb],
    ["Fastest vertical rate", report.fastest_vertical_rate],
    ["Median climb", report.median_climb],
    ["95th percentile climb", report.percentile_95_climb],
    ["First-half median", report.first_half_median],
    ["Second-half median", report.second_half_median],
    ["Best climb", report.best_climb],
    ["Worst climb", report.worst_climb],
  ] as const;
  const totalPages = Math.max(
    1,
    Math.ceil(report.climbs.length / CLIMBS_PER_PAGE),
  );
  const safePage = Math.min(climbPage, totalPages);
  const startIndex = (safePage - 1) * CLIMBS_PER_PAGE;
  const visibleClimbs = report.climbs.slice(
    startIndex,
    startIndex + CLIMBS_PER_PAGE,
  );

  return (
    <div className="mt-5 grid gap-5">
      <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        {summaryRows.map(([label, climb]) => (
          <ReportMetricCard
            key={label}
            label={label}
            value={
              climb
                ? formatElevationRate(
                    climb.vertical_rate_meters_per_hour,
                    unitSystem,
                  )
                : "n/a"
            }
            guide="higher is better"
          />
        ))}
      </div>
      <div className="overflow-x-auto">
        <table className="table table-zebra">
          <thead>
            <tr>
              <th>
                <MetricHeader label="Ride" />
              </th>
              <th>
                <MetricHeader label="#" />
              </th>
              <th>
                <MetricHeader
                  label="Gain"
                  unit={elevationUnit(unitSystem)}
                  tip="Elevation gained during the detected climb. Use as context when comparing vertical rate."
                />
              </th>
              <th>
                <MetricHeader label="Duration" tip="Climb duration." />
              </th>
              <th>
                <MetricHeader
                  label="VAM"
                  unit={elevationRateUnit(unitSystem)}
                  tip="Vertical ascent rate for the climb. Higher is better on similar climb grades and surfaces."
                />
              </th>
              <th>
                <MetricHeader
                  label="Grade"
                  unit="%"
                  tip="Average grade for the climb. Steeper climbs usually reduce speed and change HR response."
                />
              </th>
              <th>
                <MetricHeader
                  label="Avg HR"
                  unit="bpm"
                  tip="Average heart rate during the climb. Context for whether the effort was aerobic, tempo, or harder."
                />
              </th>
              <th>
                <MetricHeader
                  label="Cad"
                  unit="rpm"
                  tip="Average cadence during the climb, when cadence data is available."
                />
              </th>
              <th>
                <MetricHeader
                  label="Power"
                  unit="W"
                  tip="Average power during the climb, when power data is available."
                />
              </th>
              <th>
                <MetricHeader
                  label="HR rec"
                  unit="bpm"
                  tip="Heart-rate drop after the summit. Higher recovery is generally better when the post-climb terrain is comparable."
                />
              </th>
              <th>
                <MetricHeader
                  label="Drop"
                  tip="Time after the summit to drop 10 bpm and 15 bpm. Lower is generally better."
                />
              </th>
              <th>
                <MetricHeader
                  label="Descent"
                  tip="Whether the climb immediately rolls into a descent, which affects HR recovery."
                />
              </th>
              <th>
                <MetricHeader
                  label="Half"
                  tip="Whether the climb ended in the first or second half of the ride."
                />
              </th>
            </tr>
          </thead>
          <tbody>
            {visibleClimbs.map((climb) => (
              <tr key={`${climb.activity_id}-${climb.climb_number}`}>
                <td>{climb.activity_title}</td>
                <td>{climb.climb_number}</td>
                <td>{formatElevation(climb.gain_meters, unitSystem)}</td>
                <td>{formatDuration(climb.duration_seconds)}</td>
                <td>
                  {formatElevationRate(
                    climb.vertical_rate_meters_per_hour,
                    unitSystem,
                  )}
                </td>
                <td>{formatPercent(climb.average_grade_percent)}</td>
                <td>{formatOptionalNumber(climb.average_heart_rate_bpm)}</td>
                <td>{formatOptionalNumber(climb.average_cadence_rpm)}</td>
                <td>
                  {climb.average_power_watts == null
                    ? "n/a"
                    : `${formatNumber(climb.average_power_watts)} W`}
                </td>
                <td>
                  {formatRecovery(
                    climb.heart_rate_recovery_30_seconds_bpm,
                    climb.heart_rate_recovery_60_seconds_bpm,
                  )}
                </td>
                <td>
                  {formatDropTimes(
                    climb.seconds_to_drop_10_bpm,
                    climb.seconds_to_drop_15_bpm,
                  )}
                </td>
                <td>
                  {climb.summit_immediately_enters_descent ? "Yes" : "No"}
                </td>
                <td>{climb.first_or_second_half}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <PaginationControls
        page={safePage}
        totalPages={totalPages}
        totalItems={report.climbs.length}
        pageSize={CLIMBS_PER_PAGE}
        onPageChange={setClimbPage}
      />
    </div>
  );
}

function FatigueReportView({
  report,
  unitSystem,
  isLoading,
}: {
  report: FatigueReport | null;
  unitSystem: UnitSystem;
  isLoading: boolean;
}) {
  if (isLoading) return <LoadingReport label="Generating fatigue report..." />;
  if (!report || report.activity_count === 0)
    return <EmptyReport label="No rides with usable point data found." />;

  return (
    <div className="mt-5 grid min-w-0 gap-5">
      {report.rides.map((ride) => (
        <section
          key={ride.activity_id}
          className="min-w-0 overflow-hidden rounded-lg border border-base-300 p-4"
        >
          <div className="flex min-w-0 flex-wrap items-center gap-3">
            <h3 className="min-w-0 flex-1 truncate text-base font-semibold">
              {ride.title}
            </h3>
            <span className="badge badge-outline">
              {ride.fatigue_start_hour
                ? `Fade begins hour ${ride.fatigue_start_hour}`
                : "No clear fade"}
            </span>
            <span className="badge badge-neutral">
              Worst fatigue {formatFatigueIndex(ride.worst_fatigue_index)}
            </span>
          </div>
          <HourlyTable rows={ride.hourly} unitSystem={unitSystem} />
        </section>
      ))}
    </div>
  );
}

function CompareRidesReportView({
  report,
  selectedActivityIds,
  onSelectedActivityIdsChange,
  isLoading,
}: {
  report: CompareRidesReport | null;
  selectedActivityIds: number[];
  onSelectedActivityIdsChange: (activityIds: number[]) => void;
  isLoading: boolean;
}) {
  if (isLoading)
    return <LoadingReport label="Loading compare ride candidates..." />;
  if (!report) return <EmptyReport label="No compare ride data found." />;

  function toggleActivity(activityId: number) {
    onSelectedActivityIdsChange(
      selectedActivityIds.includes(activityId)
        ? selectedActivityIds.filter((id) => id !== activityId)
        : [...selectedActivityIds, activityId],
    );
  }

  return (
    <div className="mt-5 grid gap-5">
      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          className="btn btn-sm btn-outline"
          onClick={() => {
            onSelectedActivityIdsChange(
              topCandidates(report.candidates, "duration", 4),
            );
          }}
        >
          Latest long rides
        </button>
        <button
          type="button"
          className="btn btn-sm btn-outline"
          onClick={() => {
            onSelectedActivityIdsChange(
              topCandidates(report.candidates, "distance", 4),
            );
          }}
        >
          Longest rides
        </button>
        <button
          type="button"
          className="btn btn-sm btn-outline"
          onClick={() => {
            onSelectedActivityIdsChange(
              topCandidates(report.candidates, "elevation", 4),
            );
          }}
        >
          Biggest climbs
        </button>
        <button
          type="button"
          className="btn btn-sm btn-ghost"
          disabled={selectedActivityIds.length === 0}
          onClick={() => {
            onSelectedActivityIdsChange([]);
          }}
        >
          Clear selection
        </button>
      </div>

      {report.selected_rides.length >= 2 ? (
        <div className="overflow-x-auto rounded-lg border border-base-300">
          <table className="table table-zebra">
            <thead>
              <tr>
                <th>Metric</th>
                {report.selected_rides.map((ride) => (
                  <th key={ride.activity_id}>
                    <div className="min-w-40">
                      <div>{ride.title}</div>
                      <div className="text-xs font-normal text-base-content/50">
                        {formatShortDate(ride.started_at)}
                      </div>
                    </div>
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {report.metrics.map((metric) => (
                <tr key={metric.key}>
                  <td>
                    <MetricHeader
                      label={metric.label}
                      unit={metric.unit ?? undefined}
                      tip={compareMetricGuide(metric.direction, metric.key)}
                    />
                    {metric.trend ? (
                      <div
                        className={
                          "mt-1 text-xs font-medium " +
                          trendClassName(metric.trend.interpretation)
                        }
                      >
                        Latest vs first: {metric.trend.display}
                        {metric.trend.interpretation === "route_sensitive"
                          ? " route-sensitive"
                          : ""}
                      </div>
                    ) : null}
                  </td>
                  {report.selected_rides.map((ride) => {
                    const value = metric.values.find(
                      (candidate) => candidate.activity_id === ride.activity_id,
                    );
                    return (
                      <td key={ride.activity_id}>{value?.display ?? "n/a"}</td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="alert">
          <span>
            Select at least two rides to generate the comparison table.
          </span>
        </div>
      )}

      <section className="rounded-lg border border-base-300 p-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <h3 className="text-base font-semibold">Candidate Rides</h3>
          <span className="text-sm text-base-content/60">
            {selectedActivityIds.length} selected
          </span>
        </div>
        <div className="mt-4 overflow-x-auto">
          <table className="table table-zebra">
            <thead>
              <tr>
                <th>
                  <MetricHeader label="Select" />
                </th>
                <th>
                  <MetricHeader label="Ride" />
                </th>
                <th>
                  <MetricHeader label="Date" />
                </th>
                <th>
                  <MetricHeader
                    label="Distance"
                    unit="mi"
                    tip="Ride distance. Route-sensitive."
                  />
                </th>
                <th>
                  <MetricHeader
                    label="Elevation"
                    unit="ft"
                    tip="Ride elevation gain. Route-sensitive."
                  />
                </th>
                <th>
                  <MetricHeader
                    label="Moving"
                    tip="Moving time for the ride."
                  />
                </th>
              </tr>
            </thead>
            <tbody>
              {report.candidates.map((candidate) => (
                <tr key={candidate.activity_id}>
                  <td>
                    <input
                      type="checkbox"
                      className="checkbox checkbox-sm"
                      checked={selectedActivityIds.includes(
                        candidate.activity_id,
                      )}
                      onChange={() => {
                        toggleActivity(candidate.activity_id);
                      }}
                    />
                  </td>
                  <td>{candidate.title}</td>
                  <td>{formatShortDate(candidate.started_at)}</td>
                  <td>
                    {candidate.distance_meters == null
                      ? "n/a"
                      : `${formatNumber(candidate.distance_meters / 1609.344)} mi`}
                  </td>
                  <td>
                    {candidate.elevation_gain_meters == null
                      ? "n/a"
                      : `${formatNumber(candidate.elevation_gain_meters * 3.28084)} ft`}
                  </td>
                  <td>{formatDuration(candidate.moving_time_seconds ?? 0)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>
    </div>
  );
}

function ReassessmentReportView({
  report,
  isLoading,
}: {
  report: ReassessmentReport | null;
  isLoading: boolean;
}) {
  if (isLoading) return <LoadingReport label="Reassessing goal evidence..." />;
  if (!report) return <EmptyReport label="No reassessment data found." />;

  const signals = [
    report.endurance_progression,
    report.climbing_density,
    report.long_ride_pace,
    report.fitness_delta,
  ];

  return (
    <div className="mt-5 grid gap-5">
      <section
        className={
          "rounded-lg border p-5 " +
          reassessmentVerdictPanelClass(report.verdict)
        }
      >
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-xs font-medium uppercase text-base-content/55">
              {report.target.event_name} ·{" "}
              {report.target.target_finish_seconds == null
                ? "finish target missing"
                : formatDuration(report.target.target_finish_seconds)}
            </div>
            <h3 className="mt-2 text-xl font-semibold">
              {report.verdict_title}
            </h3>
            <p className="mt-2 max-w-3xl text-sm leading-6 text-base-content/70">
              {report.verdict_detail}
            </p>
          </div>
          <span
            className={"badge " + reassessmentVerdictBadgeClass(report.verdict)}
          >
            {reassessmentVerdictLabel(report.verdict)}
          </span>
        </div>
        <div className="mt-4 grid gap-3 sm:grid-cols-3">
          <ReportMetricCard
            label="Target pace"
            value={formatOptionalNumber(report.target.target_speed_mph)}
            unit={report.target.target_speed_mph == null ? undefined : "mph"}
            guide={
              report.target.target_speed_mph == null
                ? reassessmentTargetMissingGuide(report.target)
                : reassessmentTargetSourceLabel(report.target.target_source)
            }
          />
          <ReportMetricCard
            label="Target climbing density"
            value={formatOptionalNumber(
              report.target.target_climb_density_feet_per_hour,
            )}
            unit={
              report.target.target_climb_density_feet_per_hour == null
                ? undefined
                : "ft/h"
            }
            guide={
              report.target.target_climb_density_feet_per_hour == null
                ? reassessmentTargetMissingGuide(report.target)
                : report.target.target_date == null
                  ? (report.target.event_profile ?? undefined)
                  : `Target ${report.target.target_date}`
            }
          />
          <ReportMetricCard
            label="Target distance"
            value={formatOptionalNumber(
              report.target.target_distance_meters == null
                ? null
                : report.target.target_distance_meters / 1609.344,
            )}
            unit={
              report.target.target_distance_meters == null ? undefined : "mi"
            }
          />
        </div>
        <ReassessmentAbilityEstimateCard estimate={report.ability_estimate} />
      </section>

      <div className="grid gap-3 lg:grid-cols-4">
        {signals.map((signal) => (
          <ReassessmentSignalCard
            key={signal.title}
            signal={signal}
            currentLabel={report.recent_window.label}
            baselineLabel={report.spring_baseline_window.label}
          />
        ))}
      </div>

      <section className="rounded-lg border border-base-300 p-4">
        <h3 className="text-base font-semibold">Current Training vs Spring</h3>
        <div className="mt-4 grid gap-3 lg:grid-cols-2">
          <ReassessmentWindowCard window={report.recent_window} />
          <ReassessmentWindowCard window={report.spring_baseline_window} />
        </div>
        <div className="mt-4 grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
          <ReportMetricCard
            label="Fitness change"
            value={formatSignedNumber(report.improvement.fitness_change)}
            unit="CTL"
            guide={formatOptionalPercent(
              report.improvement.fitness_change_percent,
            )}
          />
          <ReportMetricCard
            label="Elapsed speed change"
            value={formatSignedNumber(
              report.improvement.long_ride_speed_change_mph,
            )}
            unit="mph"
            guide={formatOptionalPercent(
              report.improvement.long_ride_speed_change_percent,
            )}
          />
          <ReportMetricCard
            label="Long-ride distance change"
            value={formatSignedNumber(
              report.improvement.long_ride_distance_change_miles,
            )}
            unit="mi"
            guide={formatOptionalPercent(
              report.improvement.long_ride_distance_change_percent,
            )}
          />
          <ReportMetricCard
            label="Climb density change"
            value={formatSignedNumber(
              report.improvement.climbing_density_change_feet_per_hour,
            )}
            unit="ft/h"
            guide={formatOptionalPercent(
              report.improvement.climbing_density_change_percent,
            )}
          />
        </div>
      </section>

      <section className="rounded-lg border border-base-300 p-4">
        <h3 className="text-base font-semibold">Benchmark Long Rides</h3>
        {report.benchmark_rides.length === 0 ? (
          <div className="alert mt-4">
            <span>
              No long rides found for the training-block or spring windows.
            </span>
          </div>
        ) : (
          <div className="mt-4 overflow-x-auto">
            <table className="table table-zebra">
              <thead>
                <tr>
                  <th>Ride</th>
                  <th>Date</th>
                  <th>Elapsed</th>
                  <th>Moving</th>
                  <th>Distance</th>
                  <th>Elevation</th>
                  <th>Elapsed speed</th>
                  <th>Moving speed</th>
                  <th>Climb density</th>
                  <th>Drift</th>
                  <th>Late speed</th>
                  <th>Fatigue</th>
                </tr>
              </thead>
              <tbody>
                {report.benchmark_rides.map((ride) => (
                  <tr key={ride.activity_id}>
                    <td>{ride.title}</td>
                    <td>{formatShortDate(ride.started_at)}</td>
                    <td>{formatDuration(ride.elapsed_seconds)}</td>
                    <td>{formatOptionalDuration(ride.moving_seconds)}</td>
                    <td>{formatOptionalNumber(ride.distance_miles)} mi</td>
                    <td>{formatOptionalNumber(ride.elevation_gain_feet)} ft</td>
                    <td>{formatOptionalNumber(ride.elapsed_speed_mph)} mph</td>
                    <td>{formatOptionalNumber(ride.moving_speed_mph)} mph</td>
                    <td>
                      {formatOptionalNumber(
                        ride.climbing_density_feet_per_hour,
                      )}{" "}
                      ft/h
                    </td>
                    <td>{formatPercent(ride.aerobic_decoupling_percent)}</td>
                    <td>{formatPercent(ride.late_speed_change_percent)}</td>
                    <td>{formatFatigueIndex(ride.fatigue_index)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>

      {report.notes.length > 0 ? (
        <div className="rounded-lg border border-base-300 bg-base-200/40 p-4 text-sm leading-6 text-base-content/70">
          {report.notes.map((note) => (
            <p key={note} className="mb-1 last:mb-0">
              {note}
            </p>
          ))}
        </div>
      ) : null}
    </div>
  );
}

function ReassessmentAbilityEstimateCard({
  estimate,
}: {
  estimate: ReassessmentAbilityEstimate;
}) {
  return (
    <div className="mt-4 rounded-lg border border-base-300 bg-base-100 p-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h4 className="text-sm font-semibold">Estimated current ability</h4>
          <p className="mt-1 max-w-3xl text-xs leading-5 text-base-content/65">
            {estimate.detail}
          </p>
        </div>
        {estimate.limiter ? (
          <span className="badge badge-outline">
            Limited by {estimate.limiter}
          </span>
        ) : null}
      </div>
      <div className="mt-3 grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        <ReportMetricCard
          label="Estimated finish"
          value={formatOptionalDuration(estimate.estimated_finish_seconds)}
        />
        <ReportMetricCard
          label="Estimated course speed"
          value={formatOptionalNumber(estimate.estimated_speed_mph)}
          unit={estimate.estimated_speed_mph == null ? undefined : "mph"}
        />
        <ReportMetricCard
          label="Current best pace"
          value={formatOptionalNumber(estimate.current_long_ride_speed_mph)}
          unit={
            estimate.current_long_ride_speed_mph == null ? undefined : "mph"
          }
          guide={
            estimate.pace_limited_finish_seconds == null
              ? "pace limit n/a"
              : `pace limit ${formatDuration(estimate.pace_limited_finish_seconds)}`
          }
        />
        <ReportMetricCard
          label="Current climb density"
          value={formatOptionalNumber(
            estimate.current_climb_density_feet_per_hour,
          )}
          unit={
            estimate.current_climb_density_feet_per_hour == null
              ? undefined
              : "ft/h"
          }
          guide={
            estimate.climbing_limited_finish_seconds == null
              ? "climb limit n/a"
              : `climb limit ${formatDuration(
                  estimate.climbing_limited_finish_seconds,
                )}`
          }
        />
      </div>
    </div>
  );
}

function ReassessmentSignalCard({
  signal,
  currentLabel,
  baselineLabel,
}: {
  signal: ReassessmentSignal;
  currentLabel: string;
  baselineLabel: string;
}) {
  return (
    <div className="rounded-lg border border-base-300 bg-base-200/40 p-4">
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div className="min-w-32 flex-1 text-sm font-semibold leading-5">
          {signal.title}
        </div>
        <span
          className={
            "badge badge-sm shrink-0 whitespace-nowrap " +
            reassessmentVerdictBadgeClass(signal.status)
          }
        >
          {reassessmentVerdictLabel(signal.status)}
        </span>
      </div>
      <div className="mt-3 grid gap-2 text-sm">
        <ReassessmentSignalValue
          label={currentLabel}
          value={signal.current_value}
          unit={signal.unit}
        />
        {signal.projected_current_value != null ? (
          <ReassessmentSignalValue
            label="Projected current"
            value={signal.projected_current_value}
            unit={signal.unit}
          />
        ) : null}
        {signal.last_known_value != null ? (
          <ReassessmentSignalValue
            label="Last known"
            value={signal.last_known_value}
            unit={signal.unit}
          />
        ) : null}
        <ReassessmentSignalValue
          label={baselineLabel}
          value={signal.baseline_value}
          unit={signal.unit}
        />
        {signal.target_value != null ? (
          <ReassessmentSignalValue
            label="Target"
            value={signal.target_value}
            unit={signal.unit}
          />
        ) : null}
      </div>
      <p className="mt-3 text-xs leading-5 text-base-content/65">
        {signal.detail}
      </p>
      {signal.projection_detail ? (
        <p className="mt-2 text-xs leading-5 text-base-content/55">
          {signal.projection_detail}
        </p>
      ) : null}
      {signal.current_source_title ||
      signal.last_known_source_title ||
      signal.baseline_source_title ? (
        <div className="mt-3 grid gap-1 text-xs text-base-content/55">
          {signal.current_source_title ? (
            <div>
              {formatSignalSource(
                currentLabel,
                signal.current_source_title,
                signal.current_source_started_at,
              )}
            </div>
          ) : null}
          {signal.last_known_source_title ? (
            <div>
              {formatSignalSource(
                "Last known",
                signal.last_known_source_title,
                signal.last_known_source_started_at,
              )}
              {signal.last_known_days_old != null
                ? `, ${signal.last_known_days_old} days old`
                : ""}
            </div>
          ) : null}
          {signal.baseline_source_title ? (
            <div>
              {formatSignalSource(
                baselineLabel,
                signal.baseline_source_title,
                signal.baseline_source_started_at,
              )}
            </div>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}

function ReassessmentSignalValue({
  label,
  value,
  unit,
}: {
  label: string;
  value?: number | null;
  unit: string;
}) {
  return (
    <div className="flex items-baseline justify-between gap-2">
      <span className="text-base-content/55">{label}</span>
      <span className="font-medium">
        {formatOptionalNumber(value)} {value == null ? "" : unit}
      </span>
    </div>
  );
}

function ReassessmentWindowCard({ window }: { window: ReassessmentWindow }) {
  return (
    <div className="rounded-lg border border-base-300 bg-base-200/30 p-4">
      <div className="flex flex-wrap items-baseline justify-between gap-3">
        <h4 className="font-semibold">{window.label}</h4>
        <span className="text-sm text-base-content/60">
          {window.start_date} to {window.end_date}
        </span>
      </div>
      <div className="mt-4 grid gap-3 sm:grid-cols-2">
        <ReportMetricCard
          label="Rides"
          value={window.activity_count.toString()}
          guide={`${window.long_ride_count} long`}
        />
        <ReportMetricCard
          label="Total elevation"
          value={formatNumber(window.total_elevation_gain_feet)}
          unit="ft"
        />
        <ReportMetricCard
          label="Aggregate climb density"
          value={formatOptionalNumber(
            window.aggregate_long_ride_climbing_density_feet_per_hour,
          )}
          unit={
            window.aggregate_long_ride_climbing_density_feet_per_hour == null
              ? undefined
              : "ft/h"
          }
          guide="weighted across benchmark long rides"
        />
        <ReportMetricCard
          label="Best long ride"
          value={formatOptionalNumber(window.best_long_ride_distance_miles)}
          unit={window.best_long_ride_distance_miles == null ? undefined : "mi"}
          guide={
            window.best_long_ride_duration_seconds == null
              ? "n/a"
              : formatDuration(window.best_long_ride_duration_seconds)
          }
        />
        <ReportMetricCard
          label="Avg fitness"
          value={formatOptionalNumber(window.average_fitness)}
          unit={window.average_fitness == null ? undefined : "CTL"}
          guide={
            window.latest_fitness == null
              ? "latest n/a"
              : `latest ${formatNumber(window.latest_fitness)}`
          }
        />
      </div>
    </div>
  );
}

function HourlyTable({
  rows,
  unitSystem,
}: {
  rows: HourlyDurability[];
  unitSystem: UnitSystem;
}) {
  return (
    <div className="mt-4 min-w-0 max-w-full overflow-x-auto overscroll-x-contain">
      <table className="table table-sm table-zebra w-max min-w-full">
        <thead>
          <tr>
            <th>
              <MetricHeader label="Hour" />
            </th>
            <th>
              <MetricHeader
                label="Speed"
                unit={speedUnit(unitSystem)}
                tip="Average speed for this hour. Route-sensitive: terrain, stops, and trail difficulty matter."
              />
            </th>
            <th>
              <MetricHeader
                label="HR"
                unit="bpm"
                tip="Average heart rate for this hour."
              />
            </th>
            <th>
              <MetricHeader
                label="Ascent"
                unit={elevationUnit(unitSystem)}
                tip="Elevation gained during this hour."
              />
            </th>
            <th>
              <MetricHeader
                label="VAM"
                unit={elevationRateUnit(unitSystem)}
                tip="Vertical ascent rate for this hour. Higher is better on similar terrain."
              />
            </th>
            <th>
              <MetricHeader label="Moving" tip="Moving time in this hour." />
            </th>
            <th>
              <MetricHeader
                label="Stopped"
                tip="Stopped time in this hour. Lower is generally better."
              />
            </th>
            <th>
              <MetricHeader
                label="Stops"
                unit="/h"
                tip="Stop starts per hour. Lower is generally better."
              />
            </th>
            <th>
              <MetricHeader
                label="Eff"
                tip="Hourly efficiency: average speed divided by average heart rate. Unit is speed per bpm. Higher is better on similar terrain."
              />
            </th>
            <th>
              <MetricHeader
                label="Fatigue"
                unit="/100"
                tip="Deterministic fade score from hourly efficiency, speed, climb rate, and stops. Under 15 is steady; 25+ is a fade signal; higher is worse."
              />
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={row.hour}>
              <td className="whitespace-nowrap">{row.hour}</td>
              <td className="whitespace-nowrap">
                {formatActivitySpeed(row.average_speed_mps, unitSystem)}
              </td>
              <td className="whitespace-nowrap">
                {row.average_heart_rate_bpm == null
                  ? "n/a"
                  : `${formatNumber(row.average_heart_rate_bpm)} bpm`}
              </td>
              <td className="whitespace-nowrap">
                {formatElevation(row.ascent_meters, unitSystem)}
              </td>
              <td className="whitespace-nowrap">
                {row.climb_rate_meters_per_hour == null
                  ? "n/a"
                  : formatElevationRate(
                      row.climb_rate_meters_per_hour,
                      unitSystem,
                    )}
              </td>
              <td className="whitespace-nowrap">
                {formatDuration(row.moving_seconds)}
              </td>
              <td className="whitespace-nowrap">
                {formatDuration(row.stopped_seconds)}
              </td>
              <td className="whitespace-nowrap">
                {formatNumber(row.stop_frequency_per_hour)}
              </td>
              <td className="whitespace-nowrap">
                {formatOptionalNumber(row.efficiency_mps_per_bpm)}
              </td>
              <td className="whitespace-nowrap">
                {formatFatigueIndex(row.fatigue_index)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function PaginationControls({
  page,
  totalPages,
  totalItems,
  pageSize,
  onPageChange,
}: {
  page: number;
  totalPages: number;
  totalItems: number;
  pageSize: number;
  onPageChange: (page: number) => void;
}) {
  const startItem = totalItems === 0 ? 0 : (page - 1) * pageSize + 1;
  const endItem = Math.min(page * pageSize, totalItems);

  return (
    <div className="flex flex-wrap items-center justify-between gap-3 text-sm">
      <div className="text-base-content/60">
        Showing {startItem}-{endItem} of {totalItems}
      </div>
      <div className="join">
        <button
          type="button"
          className="btn join-item btn-sm"
          disabled={page <= 1}
          onClick={() => {
            onPageChange(page - 1);
          }}
        >
          Previous
        </button>
        <span className="join-item border border-base-300 px-3 py-1.5">
          Page {page} of {totalPages}
        </span>
        <button
          type="button"
          className="btn join-item btn-sm"
          disabled={page >= totalPages}
          onClick={() => {
            onPageChange(page + 1);
          }}
        >
          Next
        </button>
      </div>
    </div>
  );
}

function LoadingReport({ label }: { label: string }) {
  return (
    <div className="alert mt-4">
      <span>{label}</span>
    </div>
  );
}

function EmptyReport({ label }: { label: string }) {
  return (
    <div className="alert mt-4">
      <span>{label}</span>
    </div>
  );
}

function ReportMenu({
  selectedReportId,
  reports,
  onSelect,
}: {
  selectedReportId: ReportId;
  reports: ReportDefinition[];
  onSelect: (reportId: ReportId) => void;
}) {
  return (
    <nav className="p-3" aria-label="Report menu">
      <div className="px-2 pb-2 text-xs font-medium uppercase text-base-content/50">
        Reports
      </div>
      <div className="grid gap-1">
        {reports.map((report) => (
          <button
            key={report.id}
            type="button"
            className={
              "rounded-md px-3 py-3 text-left transition hover:bg-base-200 " +
              (selectedReportId === report.id
                ? "bg-primary text-primary-content hover:bg-primary/90"
                : "text-base-content/75")
            }
            onClick={() => {
              onSelect(report.id);
            }}
          >
            <span className="flex items-center justify-between gap-3">
              <span className="font-medium">{report.name}</span>
            </span>
            <span
              className={
                "mt-1 block text-xs leading-5 " +
                (selectedReportId === report.id
                  ? "text-primary-content/80"
                  : "text-base-content/60")
              }
            >
              {report.purpose}
            </span>
          </button>
        ))}
      </div>
    </nav>
  );
}

function FocusedBucketReportView({
  report,
  reportType,
  points,
  range,
  onRangeChange,
  isLoading,
  isFetching,
  isError,
  errorMessage,
}: {
  report: ReportDefinition;
  reportType: "distance" | "elevation" | "activity_type_time";
  points: NonNullable<ReturnType<typeof useTrainingReports>["data"]>["points"];
  range: TimeRange;
  onRangeChange: (range: TimeRange) => void;
  isLoading: boolean;
  isFetching: boolean;
  isError: boolean;
  errorMessage: string;
}) {
  const config = {
    distance: {
      title: "Distance",
      description: "Total distance in each interval bucket.",
      chartType: "distance" as const,
    },
    elevation: {
      title: "Elevation",
      description: "Total elevation gain in each interval bucket.",
      chartType: "elevation" as const,
    },
    activity_type_time: {
      title: "Time in Activity Type",
      description: "Moving time split by training and race activities.",
      chartType: "activity_type_time" as const,
    },
  }[reportType];
  const csvRows = focusedReportCsvRows(reportType, points, range);
  const showLoading = isLoading || (isFetching && points.length === 0);

  return (
    <section className="min-w-0 rounded-lg border border-base-300 bg-base-100 p-5">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <div className="flex items-center gap-2">
            <h2 className="text-lg font-semibold">{config.title}</h2>
            <InfoTooltip
              label={`${report.name} details`}
              tip={report.purpose}
            />
          </div>
          <p className="mt-1 text-xs text-base-content/55">
            {config.description}
          </p>
        </div>
        <div className="min-w-52">
          <form onSubmit={(event) => event.preventDefault()}>
            <div className="join w-full">
              <div>
                <div>
                  <label
                    className="input join-item flex items-center text-sm font-medium text-base-content/70"
                    htmlFor="focused-report-interval"
                  >
                    Interval
                  </label>
                </div>
              </div>
              <select
                id="focused-report-interval"
                className="select join-item min-w-0 flex-1"
                value={range}
                onChange={(event) =>
                  onRangeChange(event.target.value as TimeRange)
                }
              >
                {TIME_RANGE_OPTIONS.map((option) => (
                  <option key={option.key} value={option.key}>
                    {option.label}
                  </option>
                ))}
              </select>
              <button
                type="button"
                className="btn btn-outline join-item"
                disabled={showLoading || csvRows.length === 0}
                onClick={() => {
                  downloadCsv(
                    focusedReportCsvFilename(reportType, range),
                    csvRows,
                  );
                }}
              >
                <FontAwesomeIcon icon={faDownload} className="h-3.5 w-3.5" />
                Export CSV
              </button>
            </div>
          </form>
        </div>
      </div>

      {showLoading ? <ReportLoadingSpinner /> : null}

      {!showLoading && isError ? (
        <div className="alert alert-warning mt-4">
          <span>{errorMessage}</span>
        </div>
      ) : null}

      {!showLoading && !isError && points.length === 0 ? (
        <div className="alert mt-4">
          <span>No report data found for this interval.</span>
        </div>
      ) : null}

      {!showLoading && !isError && points.length > 0 ? (
        <div className="mt-4">
          <Charts type={config.chartType} points={points} range={range} />
        </div>
      ) : null}

      {!showLoading && isFetching ? (
        <p className="mt-3 text-sm opacity-70">Refreshing...</p>
      ) : null}
    </section>
  );
}

function ReportLoadingSpinner() {
  return (
    <div className="mt-4 flex h-72 items-center justify-center rounded border border-dashed border-base-300">
      <span className="loading loading-spinner loading-lg text-primary" />
      <span className="sr-only">Loading report data</span>
    </div>
  );
}

type CsvRow = Record<string, string | number>;

function focusedReportCsvRows(
  reportType: "distance" | "elevation" | "activity_type_time",
  points: TrainingReportPoint[],
  range: TimeRange,
): CsvRow[] {
  switch (reportType) {
    case "distance":
      return points.map((point) => ({
        bucket_label: chartBucketLabel(new Date(point.bucket_start), range),
        bucket_start: point.bucket_start,
        bucket_end: point.bucket_end,
        distance_miles: point.distance_miles,
        distance_meters: point.distance_meters,
      }));
    case "elevation":
      return points.map((point) => ({
        bucket_label: chartBucketLabel(new Date(point.bucket_start), range),
        bucket_start: point.bucket_start,
        bucket_end: point.bucket_end,
        elevation_gain_feet: point.elevation_gain_feet,
        elevation_gain_meters: point.elevation_gain_meters,
      }));
    case "activity_type_time":
      return points.map((point) => {
        const trainingSeconds = activityTypeSeconds(point, "training");
        const raceSeconds = activityTypeSeconds(point, "race");
        return {
          bucket_label: chartBucketLabel(new Date(point.bucket_start), range),
          bucket_start: point.bucket_start,
          bucket_end: point.bucket_end,
          training_hours: roundCsvNumber(trainingSeconds / 3600),
          training_seconds: trainingSeconds,
          race_hours: roundCsvNumber(raceSeconds / 3600),
          race_seconds: raceSeconds,
          total_hours: roundCsvNumber((trainingSeconds + raceSeconds) / 3600),
          total_seconds: trainingSeconds + raceSeconds,
        };
      });
  }
}

function activityTypeSeconds(point: TrainingReportPoint, activityType: string) {
  return (
    point.activity_type_times.find((row) => row.activity_type === activityType)
      ?.seconds ?? 0
  );
}

function roundCsvNumber(value: number) {
  return Number(value.toFixed(3));
}

function focusedReportCsvFilename(
  reportType: "distance" | "elevation" | "activity_type_time",
  range: TimeRange,
) {
  return `bike-${reportType.replaceAll("_", "-")}-${range}.csv`;
}

function downloadCsv(filename: string, rows: CsvRow[]) {
  if (rows.length === 0) {
    return;
  }

  const headers = Object.keys(rows[0]);
  const csv = [
    headers.join(","),
    ...rows.map((row) =>
      headers.map((header) => csvCell(row[header] ?? "")).join(","),
    ),
  ].join("\n");
  const blob = new Blob([csv], { type: "text/csv;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
}

function csvCell(value: string | number) {
  const text = String(value);
  return /[",\n\r]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text;
}

function AggregateTrendsReport({
  points,
  range,
  isLoading,
  isFetching,
}: {
  points: NonNullable<ReturnType<typeof useTrainingReports>["data"]>["points"];
  range: TimeRange;
  isLoading: boolean;
  isFetching: boolean;
}) {
  return (
    <div>
      {isLoading ? (
        <div className="alert mt-4">
          <span>Loading report data...</span>
        </div>
      ) : null}

      {!isLoading && points.length === 0 ? (
        <div className="alert mt-4">
          <span>No report data found for this boundary.</span>
        </div>
      ) : null}

      <div className="grid grid-cols-1 gap-6 md:grid-cols-2 mt-4">
        <div className="card p-4">
          <h2 className="text-lg font-medium mb-2">Z2 Speed (mph)</h2>
          <p className="mb-2 text-xs text-base-content/55">
            Higher is better only on comparable routes.
          </p>
          <Charts type="z2_speed" points={points} range={range} />
        </div>

        <div className="card p-4">
          <h2 className="text-lg font-medium mb-2">Decoupling (%)</h2>
          <p className="mb-2 text-xs text-base-content/55">
            Lower is better; &lt;5 good, &gt;10 fade.
          </p>
          <Charts type="decoupling" points={points} range={range} />
        </div>

        <div className="card p-4">
          <h2 className="text-lg font-medium mb-2">Median Climb Rate (ft/h)</h2>
          <p className="mb-2 text-xs text-base-content/55">
            Higher is better on similar climb profiles.
          </p>
          <Charts type="climbing_pace" points={points} range={range} />
        </div>

        <div className="card p-4">
          <h2 className="text-lg font-medium mb-2">Heart Rate Zones</h2>
          <p className="mb-2 text-xs text-base-content/55">
            Percent of heart-rate time in each bucket.
          </p>
          <Charts type="hr_zones" points={points} range={range} />
        </div>

        <div className="card p-4 md:col-span-2">
          <h2 className="text-lg font-medium mb-2">Elevation (ft)</h2>
          <p className="mb-2 text-xs text-base-content/55">
            Total gain in the bucket.
          </p>
          <Charts type="elevation" points={points} range={range} />
        </div>
      </div>

      {!isLoading && isFetching ? (
        <p className="mt-3 text-sm opacity-70">Refreshing...</p>
      ) : null}
    </div>
  );
}

function PlannedReport({ report }: { report: ReportDefinition }) {
  return (
    <div className="mt-5 grid gap-4">
      <div className="rounded-lg border border-dashed border-base-300 bg-base-200/50 p-5">
        <div className="text-sm font-medium">Report definition ready</div>
        <p className="mt-2 max-w-2xl text-sm leading-6 text-base-content/70">
          The standalone Rust analyzer will generate this report from activities
          in the selected date range.
        </p>
      </div>

      <div>
        <div className="mb-2 text-xs font-medium uppercase text-base-content/50">
          Metrics
        </div>
        <div className="flex flex-wrap gap-2">
          {report.metrics.map((metric) => (
            <span key={metric} className="badge badge-outline">
              {metric}
            </span>
          ))}
        </div>
      </div>
    </div>
  );
}

function parseActivityIds(value: string | null) {
  if (!value) {
    return [];
  }

  return value
    .split(",")
    .map((part) => Number(part.trim()))
    .filter((id) => Number.isFinite(id) && id > 0);
}

function canonicalReportQuery({
  range,
  selectedActivityIds,
  selectedReportId,
  isReassessment,
}: {
  range: TimeRange;
  selectedActivityIds: number[];
  selectedReportId: ReportId;
  isReassessment: boolean;
}) {
  const params = new URLSearchParams();
  if (selectedReportId !== DEFAULT_REPORT_ID) {
    params.set("report", selectedReportId);
  }
  if (!isReassessment) {
    params.set("range", range);
  }
  if (selectedActivityIds.length > 0) {
    params.set("activity_ids", selectedActivityIds.join(","));
  }
  return params.toString();
}

function reportGenerationErrorMessage(error: unknown) {
  const fallback =
    "A report is already generating. Go back to that report, or wait a few seconds before starting another one.";
  return apiErrorMessage(error, fallback);
}

function apiErrorMessage(error: unknown, fallback: string) {
  if (!error || typeof error !== "object") {
    return fallback;
  }

  const candidates = [
    (error as { message?: unknown }).message,
    (error as { data?: { message?: unknown } }).data?.message,
    (error as { body?: { message?: unknown } }).body?.message,
    (error as { response?: { data?: { message?: unknown } } }).response?.data
      ?.message,
    (error as { error?: { message?: unknown } }).error?.message,
  ];
  const message = candidates.find(
    (candidate): candidate is string =>
      typeof candidate === "string" && candidate.trim().length > 0,
  );
  return message ?? fallback;
}

function showReportRequestInProgressToast() {
  toast.error(
    "A report is already generating. Stay on this report until it finishes before starting another one.",
    {
      id: "report-generation-in-progress",
      duration: 7000,
    },
  );
}

function reportErrorStatus(error: unknown) {
  if (!error || typeof error !== "object") {
    return null;
  }

  const candidates = [
    (error as { status?: unknown }).status,
    (error as { data?: { status?: unknown } }).data?.status,
    (error as { body?: { status?: unknown } }).body?.status,
    (error as { response?: { status?: unknown } }).response?.status,
    (error as { error?: { status?: unknown } }).error?.status,
  ];
  const status = candidates.find(
    (candidate): candidate is number => typeof candidate === "number",
  );
  return status ?? null;
}

function isReportGenerationInProgressError(error: unknown, message: string) {
  return (
    reportErrorStatus(error) === 429 ||
    /already .*generating|generation .*in progress/i.test(message)
  );
}

function standaloneReportIdFor(
  reportId: ReportId,
):
  | "ride_summary"
  | "endurance"
  | "climbing"
  | "fatigue"
  | "compare_rides"
  | "reassessment"
  | null {
  switch (reportId) {
    case "ride_summary":
    case "endurance":
    case "climbing":
    case "fatigue":
    case "compare_rides":
    case "reassessment":
      return reportId;
    default:
      return null;
  }
}

function focusedBucketReportFor(
  reportId: ReportId,
): "distance" | "elevation" | "activity_type_time" | null {
  switch (reportId) {
    case "distance":
    case "elevation":
    case "activity_type_time":
      return reportId;
    default:
      return null;
  }
}

function reportBoundaryForTimeRange(range: TimeRange): TrainingReportBoundary {
  switch (range) {
    case "week":
      return "week";
    case "month":
      return "month";
    case "6month":
      return "6month";
    case "ytd":
    case "1year":
      return "1year";
    case "3year":
      return "3year";
    case "5year":
      return "5year";
    case "all":
      return "all";
  }
}

function dateRangeForTimeRange(range: TimeRange) {
  const end = new Date();
  const start = new Date(end);

  switch (range) {
    case "week":
      start.setDate(end.getDate() - 7);
      break;
    case "month":
      start.setDate(end.getDate() - 30);
      break;
    case "6month":
      start.setDate(end.getDate() - 180);
      break;
    case "ytd":
      start.setMonth(0, 1);
      break;
    case "1year":
      start.setDate(end.getDate() - 365);
      break;
    case "3year":
      start.setFullYear(end.getFullYear() - 2, 0, 1);
      break;
    case "5year":
      start.setFullYear(end.getFullYear() - 4, 0, 1);
      break;
    case "all":
      return {
        startDate: "",
        endDate: formatDateInput(end),
      };
  }

  return {
    startDate: formatDateInput(start),
    endDate: formatDateInput(end),
  };
}

function parseTimeRange(value: string | null): TimeRange {
  switch (value) {
    case "week":
    case "month":
    case "6month":
    case "ytd":
    case "1year":
    case "3year":
    case "5year":
    case "all":
      return value;
    case "day":
      return "week";
    case "3month":
      return "6month";
    case "2year":
      return "3year";
    default:
      return DEFAULT_REPORT_RANGE;
  }
}

function formatDateInput(date: Date) {
  const year = date.getFullYear();
  const month = `${date.getMonth() + 1}`.padStart(2, "0");
  const day = `${date.getDate()}`.padStart(2, "0");

  return `${year}-${month}-${day}`;
}

function formatNumber(value: number) {
  return new Intl.NumberFormat(undefined, {
    maximumFractionDigits: value >= 100 ? 0 : 1,
  }).format(value);
}

function formatOptionalNumber(value?: number | null) {
  return value == null ? "n/a" : formatNumber(value);
}

function formatPercent(value?: number | null) {
  return value == null ? "n/a" : `${formatNumber(value)}%`;
}

function formatOptionalPercent(value?: number | null) {
  return value == null ? "n/a" : `${formatSignedNumber(value)}%`;
}

function formatSignedNumber(value?: number | null) {
  if (value == null) {
    return "n/a";
  }

  return `${value > 0 ? "+" : ""}${formatNumber(value)}`;
}

function formatZonePercent(seconds: number, totalSeconds: number) {
  if (totalSeconds <= 0) {
    return "n/a";
  }

  return `${formatNumber((seconds / totalSeconds) * 100)}%`;
}

function formatFatigueIndex(value?: number | null) {
  return value == null ? "n/a" : `${formatNumber(value)}/100`;
}

function speedUnit(unitSystem: UnitSystem) {
  return unitSystem === "metric" ? "km/h" : "mph";
}

function elevationUnit(unitSystem: UnitSystem) {
  return unitSystem === "imperial" ? "ft" : "m";
}

function elevationRateUnit(unitSystem: UnitSystem) {
  return unitSystem === "imperial" ? "ft/h" : "m/h";
}

function compareMetricGuide(direction: string, key: string) {
  if (key === "aerobic_decoupling_percent") {
    return "<5 good; >10 fade";
  }
  if (key === "late_speed_change_percent") {
    return "near 0 good; negative fade";
  }
  if (key === "stopped_time_percent") {
    return "lower is better";
  }
  if (key.includes("climb_rate") || key.includes("recovery")) {
    return "higher is better";
  }
  if (direction === "lower") {
    return "lower is better";
  }
  if (direction === "higher") {
    return "higher is better";
  }
  return "route-sensitive";
}

function formatRecovery(
  recovery30Seconds?: number | null,
  recovery60Seconds?: number | null,
) {
  const recovery30 =
    recovery30Seconds == null
      ? "n/a"
      : `${formatNumber(recovery30Seconds)} bpm`;
  const recovery60 =
    recovery60Seconds == null
      ? "n/a"
      : `${formatNumber(recovery60Seconds)} bpm`;

  return `30s ${recovery30} / 60s ${recovery60}`;
}

function formatDropTimes(
  secondsToDrop10Bpm?: number | null,
  secondsToDrop15Bpm?: number | null,
) {
  const drop10 =
    secondsToDrop10Bpm == null ? "n/a" : formatDuration(secondsToDrop10Bpm);
  const drop15 =
    secondsToDrop15Bpm == null ? "n/a" : formatDuration(secondsToDrop15Bpm);

  return `10 bpm ${drop10} / 15 bpm ${drop15}`;
}

function trendClassName(interpretation: string) {
  switch (interpretation) {
    case "improving":
      return "text-success";
    case "declining":
      return "text-error";
    case "route_sensitive":
      return "text-base-content/60";
    default:
      return "text-base-content/70";
  }
}

function reassessmentVerdictLabel(verdict: ReassessmentVerdict) {
  switch (verdict) {
    case "on_track":
      return "On track";
    case "plausible_but_risky":
      return "Risky";
    case "needs_more_evidence":
      return "Needs work";
    case "missing_data":
      return "Missing data";
  }
}

function reassessmentVerdictBadgeClass(verdict: ReassessmentVerdict) {
  switch (verdict) {
    case "on_track":
      return "badge-success";
    case "plausible_but_risky":
      return "badge-warning";
    case "needs_more_evidence":
      return "badge-warning";
    case "missing_data":
      return "badge-ghost";
  }
}

function reassessmentVerdictPanelClass(verdict: ReassessmentVerdict) {
  switch (verdict) {
    case "on_track":
      return "border-success/30 bg-success/10";
    case "plausible_but_risky":
      return "border-warning/40 bg-warning/10";
    case "needs_more_evidence":
      return "border-warning/40 bg-warning/10";
    case "missing_data":
      return "border-base-300 bg-base-200/40";
  }
}

function reassessmentTargetSourceLabel(source: ReassessmentTargetSource) {
  switch (source) {
    case "saved_goal":
      return "Saved goal";
    case "missing_goal":
      return "Missing goal";
  }
}

function reassessmentTargetMissingGuide(target: ReassessmentTarget) {
  if (
    target.target_source === "missing_goal" ||
    target.target_distance_meters == null ||
    target.target_elevation_gain_meters == null
  ) {
    return "Goal incomplete";
  }
  if (target.target_finish_seconds == null) {
    return "Finish target missing";
  }
  return reassessmentTargetSourceLabel(target.target_source);
}

function formatSignalSource(
  label: string,
  title: string,
  startedAt?: string | null,
) {
  return `${label}: ${title}${startedAt ? ` (${formatShortDate(startedAt)})` : ""}`;
}

function formatShortDate(value: string) {
  return new Date(value).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

function formatDuration(seconds: number) {
  const safeSeconds = Math.max(0, seconds);
  const hours = Math.floor(safeSeconds / 3600);
  const minutes = Math.round((safeSeconds % 3600) / 60);

  if (hours <= 0) {
    return `${minutes}m`;
  }

  return `${hours}h ${minutes.toString().padStart(2, "0")}m`;
}

function formatOptionalDuration(seconds?: number | null) {
  return seconds == null ? "--" : formatDuration(seconds);
}

function topCandidates(
  candidates: CompareRideCandidate[],
  sortBy: "duration" | "distance" | "elevation",
  limit: number,
) {
  return [...candidates]
    .sort((a, b) => {
      if (sortBy === "duration") {
        return (
          (b.moving_time_seconds ?? b.total_time_seconds ?? 0) -
          (a.moving_time_seconds ?? a.total_time_seconds ?? 0)
        );
      }
      if (sortBy === "distance") {
        return (b.distance_meters ?? 0) - (a.distance_meters ?? 0);
      }
      return (b.elevation_gain_meters ?? 0) - (a.elevation_gain_meters ?? 0);
    })
    .slice(0, limit)
    .map((candidate) => candidate.activity_id);
}
