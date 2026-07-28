"use client";

import { useRouter, useSearchParams } from "next/navigation";
import { useEffect, useState } from "react";
import { extractApiMessage } from "../../lib/activityFormatting";
import {
  useRideSummaryReport,
  useTrainingReports,
  type ClimbingReport,
  type CompareRideCandidate,
  type CompareRidesReport,
  type EnduranceReport,
  type FatigueReport,
  type HourlyDurability,
  type RideSummaryReport,
} from "../../lib/queries";
import Charts from "./Charts";
import TimeRangeSelector, { TimeRange } from "./TimeRangeSelector";
import {
  DEFAULT_REPORT_ID,
  REPORT_DEFINITIONS,
  findReportDefinition,
  type ReportDefinition,
  type ReportId,
} from "./reportDefinitions";

export default function ReportsClient() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const initialRange = parseTimeRange(searchParams.get("range"));
  const initialDateRange = dateRangeForTimeRange(initialRange);
  const [range, setRange] = useState<TimeRange>(initialRange);
  const [startDate, setStartDate] = useState(
    searchParams.get("start_date") ?? initialDateRange.startDate,
  );
  const [endDate, setEndDate] = useState(
    searchParams.get("end_date") ?? initialDateRange.endDate,
  );
  const [selectedReportId, setSelectedReportId] = useState<ReportId>(
    findReportDefinition(searchParams.get("report")).id,
  );
  const [selectedActivityIds, setSelectedActivityIds] = useState<number[]>(
    parseActivityIds(searchParams.get("activity_ids")),
  );
  const selectedReport = findReportDefinition(selectedReportId);
  const isAggregateTrends = selectedReport.id === "aggregate_trends";
  const isRideSummary = selectedReport.id === "ride_summary";
  const standaloneReportId = standaloneReportIdFor(selectedReport.id);
  const isStandaloneReport = standaloneReportId != null;
  const { data, isLoading, isError, error, isFetching } = useTrainingReports(
    range,
    {
      enabled: isAggregateTrends,
      startDate,
      endDate,
    },
  );
  const reportQuery = useRideSummaryReport({
    report: standaloneReportId ?? "ride_summary",
    boundary: range,
    startDate,
    endDate,
    activityIds: selectedActivityIds,
    enabled: isStandaloneReport,
  });

  const points = data?.points ?? [];

  useEffect(() => {
    const params = new URLSearchParams();
    if (selectedReportId !== DEFAULT_REPORT_ID) {
      params.set("report", selectedReportId);
    }
    params.set("range", range);
    params.set("start_date", startDate);
    params.set("end_date", endDate);
    if (selectedActivityIds.length > 0) {
      params.set("activity_ids", selectedActivityIds.join(","));
    }

    const nextQuery = params.toString();
    if (nextQuery !== searchParams.toString()) {
      router.replace(`/training/reports?${nextQuery}`, { scroll: false });
    }
  }, [
    endDate,
    range,
    router,
    searchParams,
    selectedActivityIds,
    selectedReportId,
    startDate,
  ]);

  function handleRangeChange(nextRange: TimeRange) {
    const nextDateRange = dateRangeForTimeRange(nextRange);
    setRange(nextRange);
    setStartDate(nextDateRange.startDate);
    setEndDate(nextDateRange.endDate);
  }

  return (
    <div className="grid gap-6">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold">Reports</h1>
          <p className="mt-2 max-w-3xl text-sm text-base-content/70">
            Generate ad-hoc ride analysis for the selected range.
          </p>
        </div>
        <div className="min-w-0">
          <div className="mb-2 text-xs font-medium uppercase text-base-content/50">
            Preset
          </div>
          <TimeRangeSelector value={range} onChange={handleRangeChange} />
        </div>
      </div>

      <section className="rounded-lg border border-base-300 bg-base-100 p-4">
        <div className="grid gap-4 sm:grid-cols-[minmax(0,14rem)_minmax(0,14rem)_auto] sm:items-end">
          <label className="form-control">
            <div className="label">
              <span className="label-text font-medium">Start date</span>
            </div>
            <input
              type="date"
              className="input input-bordered"
              value={startDate}
              onChange={(event) => {
                setStartDate(event.target.value);
              }}
            />
          </label>
          <label className="form-control">
            <div className="label">
              <span className="label-text font-medium">End date</span>
            </div>
            <input
              type="date"
              className="input input-bordered"
              value={endDate}
              onChange={(event) => {
                setEndDate(event.target.value);
              }}
            />
          </label>
          <div className="text-sm text-base-content/60">
            Reports are generated from rides started inside this range.
          </div>
        </div>
      </section>

      <div className="grid gap-6 lg:grid-cols-[18rem_minmax(0,1fr)]">
        <ReportMenu
          selectedReportId={selectedReport.id}
          onSelect={setSelectedReportId}
        />

        <section className="min-w-0 rounded-lg border border-base-300 bg-base-100 p-5">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div>
              <h2 className="text-lg font-semibold">{selectedReport.name}</h2>
              <p className="mt-1 max-w-2xl text-sm text-base-content/70">
                {selectedReport.purpose}
              </p>
            </div>
          </div>

          {isAggregateTrends ? (
            <AggregateTrendsReport
              points={points}
              range={range}
              isLoading={isLoading}
              isError={isError}
              error={error}
              isFetching={isFetching}
            />
          ) : isRideSummary ? (
            <RideSummaryReportView
              summary={reportQuery.data?.ride_summary ?? null}
              startDate={startDate}
              endDate={endDate}
              isLoading={reportQuery.isLoading}
              isError={reportQuery.isError}
              error={reportQuery.error}
              isFetching={reportQuery.isFetching}
            />
          ) : selectedReport.id === "endurance" ? (
            <EnduranceReportView
              report={reportQuery.data?.endurance ?? null}
              isLoading={reportQuery.isLoading}
              isError={reportQuery.isError}
              error={reportQuery.error}
            />
          ) : selectedReport.id === "climbing" ? (
            <ClimbingReportView
              report={reportQuery.data?.climbing ?? null}
              isLoading={reportQuery.isLoading}
              isError={reportQuery.isError}
              error={reportQuery.error}
            />
          ) : selectedReport.id === "fatigue" ? (
            <FatigueReportView
              report={reportQuery.data?.fatigue ?? null}
              isLoading={reportQuery.isLoading}
              isError={reportQuery.isError}
              error={reportQuery.error}
            />
          ) : selectedReport.id === "compare_rides" ? (
            <CompareRidesReportView
              report={reportQuery.data?.compare_rides ?? null}
              selectedActivityIds={selectedActivityIds}
              onSelectedActivityIdsChange={setSelectedActivityIds}
              isLoading={reportQuery.isLoading}
              isError={reportQuery.isError}
              error={reportQuery.error}
            />
          ) : (
            <PlannedReport report={selectedReport} />
          )}
        </section>
      </div>
    </div>
  );
}

function RideSummaryReportView({
  summary,
  startDate,
  endDate,
  isLoading,
  isError,
  error,
  isFetching,
}: {
  summary: RideSummaryReport | null;
  startDate: string;
  endDate: string;
  isLoading: boolean;
  isError: boolean;
  error: unknown;
  isFetching: boolean;
}) {
  if (isLoading) {
    return (
      <div className="alert mt-4">
        <span>Generating ride summary...</span>
      </div>
    );
  }

  if (isError) {
    return (
      <div className="alert alert-error mt-4">
        <span>{extractApiMessage(error) || "Failed to generate ride summary."}</span>
      </div>
    );
  }

  if (!summary || summary.activity_count === 0) {
    return (
      <div className="alert mt-4">
        <span>No rides found from {startDate} through {endDate}.</span>
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
          unit={summary.climbing_density_feet_per_hour == null ? undefined : "ft/h"}
        />
      </div>

      <section className="rounded-lg border border-base-300 p-4">
        <h3 className="text-base font-semibold">Heart Rate Zones</h3>
        <div className="mt-4 grid gap-3 sm:grid-cols-5">
          {[
            ["Z1", summary.z1_seconds],
            ["Z2", summary.z2_seconds],
            ["Z3", summary.z3_seconds],
            ["Z4", summary.z4_seconds],
            ["Z5", summary.z5_seconds],
          ].map(([label, seconds]) => (
            <div key={label} className="rounded-md bg-base-200 p-3">
              <div className="text-xs font-medium uppercase text-base-content/50">
                {label}
              </div>
              <div className="mt-1 text-lg font-semibold">
                {formatDuration(seconds as number)}
              </div>
              <div className="mt-1 text-xs text-base-content/60">
                {zoneTotal > 0
                  ? `${Math.round(((seconds as number) / zoneTotal) * 100)}%`
                  : "n/a"}
              </div>
            </div>
          ))}
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
}: {
  label: string;
  value: string;
  unit?: string;
}) {
  return (
    <div className="rounded-lg border border-base-300 bg-base-200/40 p-4">
      <div className="text-xs font-medium uppercase text-base-content/50">
        {label}
      </div>
      <div className="mt-2 flex items-baseline gap-2">
        <span className="text-2xl font-semibold">{value}</span>
        {unit ? <span className="text-sm text-base-content/60">{unit}</span> : null}
      </div>
    </div>
  );
}

function EnduranceReportView({
  report,
  isLoading,
  isError,
  error,
}: {
  report: EnduranceReport | null;
  isLoading: boolean;
  isError: boolean;
  error: unknown;
}) {
  if (isLoading) return <LoadingReport label="Generating endurance report..." />;
  if (isError) return <ErrorReport error={error} label="Failed to generate endurance report." />;
  if (!report || report.activity_count === 0) return <EmptyReport label="No rides with usable point data found." />;

  return (
    <div className="mt-5 grid gap-5">
      <div className="grid gap-3 sm:grid-cols-3">
        <ReportMetricCard label="Rides" value={report.activity_count.toString()} />
        <ReportMetricCard
          label="Median decoupling"
          value={formatOptionalNumber(report.median_aerobic_decoupling_percent)}
          unit={report.median_aerobic_decoupling_percent == null ? undefined : "%"}
        />
        <ReportMetricCard
          label="Median late fade"
          value={formatOptionalNumber(report.median_late_speed_change_percent)}
          unit={report.median_late_speed_change_percent == null ? undefined : "%"}
        />
      </div>
      <div className="overflow-x-auto">
        <table className="table table-zebra">
          <thead>
            <tr>
              <th>Ride</th>
              <th>Duration</th>
              <th>Decoupling</th>
              <th>Late Speed</th>
              <th>Late HR</th>
              <th>Hours</th>
            </tr>
          </thead>
          <tbody>
            {report.rides.map((ride) => (
              <tr key={ride.activity_id}>
                <td>{ride.title}</td>
                <td>{formatDuration(ride.elapsed_seconds)}</td>
                <td>{formatPercent(ride.aerobic_decoupling_percent)}</td>
                <td>{formatPercent(ride.late_speed_change_percent)}</td>
                <td>{formatPercent(ride.late_heart_rate_change_percent)}</td>
                <td>{ride.hourly.length}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function ClimbingReportView({
  report,
  isLoading,
  isError,
  error,
}: {
  report: ClimbingReport | null;
  isLoading: boolean;
  isError: boolean;
  error: unknown;
}) {
  if (isLoading) return <LoadingReport label="Generating climbing report..." />;
  if (isError) return <ErrorReport error={error} label="Failed to generate climbing report." />;
  if (!report || report.climb_count === 0) return <EmptyReport label="No sustained climbs found." />;

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

  return (
    <div className="mt-5 grid gap-5">
      <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        {summaryRows.map(([label, climb]) => (
          <ReportMetricCard
            key={label}
            label={label}
            value={climb ? formatNumber(climb.vertical_rate_meters_per_hour) : "n/a"}
            unit={climb ? "m/h" : undefined}
          />
        ))}
      </div>
      <div className="overflow-x-auto">
        <table className="table table-zebra">
          <thead>
            <tr>
              <th>Ride</th>
              <th>#</th>
              <th>Gain</th>
              <th>Duration</th>
              <th>Vertical Rate</th>
              <th>Grade</th>
              <th>Avg HR</th>
              <th>Half</th>
            </tr>
          </thead>
          <tbody>
            {report.climbs.map((climb) => (
              <tr key={`${climb.activity_id}-${climb.climb_number}`}>
                <td>{climb.activity_title}</td>
                <td>{climb.climb_number}</td>
                <td>{formatNumber(climb.gain_meters)} m</td>
                <td>{formatDuration(climb.duration_seconds)}</td>
                <td>{formatNumber(climb.vertical_rate_meters_per_hour)} m/h</td>
                <td>{formatPercent(climb.average_grade_percent)}</td>
                <td>{formatOptionalNumber(climb.average_heart_rate_bpm)}</td>
                <td>{climb.first_or_second_half}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function FatigueReportView({
  report,
  isLoading,
  isError,
  error,
}: {
  report: FatigueReport | null;
  isLoading: boolean;
  isError: boolean;
  error: unknown;
}) {
  if (isLoading) return <LoadingReport label="Generating fatigue report..." />;
  if (isError) return <ErrorReport error={error} label="Failed to generate fatigue report." />;
  if (!report || report.activity_count === 0) return <EmptyReport label="No rides with usable point data found." />;

  return (
    <div className="mt-5 grid gap-5">
      {report.rides.map((ride) => (
        <section key={ride.activity_id} className="rounded-lg border border-base-300 p-4">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <h3 className="text-base font-semibold">{ride.title}</h3>
            <span className="badge badge-outline">
              {ride.fatigue_start_hour
                ? `Fade begins hour ${ride.fatigue_start_hour}`
                : "No clear fade"}
            </span>
          </div>
          <HourlyTable rows={ride.hourly} />
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
  isError,
  error,
}: {
  report: CompareRidesReport | null;
  selectedActivityIds: number[];
  onSelectedActivityIdsChange: (activityIds: number[]) => void;
  isLoading: boolean;
  isError: boolean;
  error: unknown;
}) {
  if (isLoading) return <LoadingReport label="Loading compare ride candidates..." />;
  if (isError) return <ErrorReport error={error} label="Failed to load compare rides." />;
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
                    <div className="font-medium">{metric.label}</div>
                    <div className="text-xs text-base-content/50">
                      {metric.direction === "lower"
                        ? "Lower is better"
                        : metric.direction === "higher"
                          ? "Higher is better"
                          : "Route-sensitive"}
                    </div>
                  </td>
                  {report.selected_rides.map((ride) => {
                    const value = metric.values.find(
                      (candidate) => candidate.activity_id === ride.activity_id,
                    );
                    return <td key={ride.activity_id}>{value?.display ?? "n/a"}</td>;
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="alert">
          <span>Select at least two rides to generate the comparison table.</span>
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
                <th>Select</th>
                <th>Ride</th>
                <th>Date</th>
                <th>Distance</th>
                <th>Elevation</th>
                <th>Moving</th>
              </tr>
            </thead>
            <tbody>
              {report.candidates.map((candidate) => (
                <tr key={candidate.activity_id}>
                  <td>
                    <input
                      type="checkbox"
                      className="checkbox checkbox-sm"
                      checked={selectedActivityIds.includes(candidate.activity_id)}
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

function HourlyTable({ rows }: { rows: HourlyDurability[] }) {
  return (
    <div className="mt-4 overflow-x-auto">
      <table className="table table-zebra">
        <thead>
          <tr>
            <th>Hour</th>
            <th>Speed</th>
            <th>HR</th>
            <th>Ascent</th>
            <th>Moving</th>
            <th>Stopped</th>
            <th>Stops</th>
            <th>Efficiency</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={row.hour}>
              <td>{row.hour}</td>
              <td>{formatOptionalNumber(row.average_speed_mps)} m/s</td>
              <td>{formatOptionalNumber(row.average_heart_rate_bpm)}</td>
              <td>{formatNumber(row.ascent_meters)} m</td>
              <td>{formatDuration(row.moving_seconds)}</td>
              <td>{formatDuration(row.stopped_seconds)}</td>
              <td>{row.stop_count}</td>
              <td>{formatOptionalNumber(row.efficiency_mps_per_bpm)}</td>
            </tr>
          ))}
        </tbody>
      </table>
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

function ErrorReport({ error, label }: { error: unknown; label: string }) {
  return (
    <div className="alert alert-error mt-4">
      <span>{extractApiMessage(error) || label}</span>
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
  onSelect,
}: {
  selectedReportId: ReportId;
  onSelect: (reportId: ReportId) => void;
}) {
  return (
    <nav className="rounded-lg border border-base-300 bg-base-100 p-3">
      <div className="px-2 pb-2 text-xs font-medium uppercase text-base-content/50">
        Report Menu
      </div>
      <div className="grid gap-1">
        {REPORT_DEFINITIONS.map((report) => (
          <button
            key={report.id}
            type="button"
            className={
              "rounded-md px-3 py-3 text-left transition hover:bg-base-200 " +
              (selectedReportId === report.id
                ? "bg-base-200 text-base-content"
                : "text-base-content/75")
            }
            onClick={() => {
              onSelect(report.id);
            }}
          >
            <span className="flex items-center justify-between gap-3">
              <span className="font-medium">{report.name}</span>
            </span>
            <span className="mt-1 block text-xs leading-5 text-base-content/60">
              {report.purpose}
            </span>
          </button>
        ))}
      </div>
    </nav>
  );
}

function AggregateTrendsReport({
  points,
  range,
  isLoading,
  isError,
  error,
  isFetching,
}: {
  points: NonNullable<ReturnType<typeof useTrainingReports>["data"]>["points"];
  range: TimeRange;
  isLoading: boolean;
  isError: boolean;
  error: unknown;
  isFetching: boolean;
}) {
  return (
    <div>
      {isLoading ? (
        <div className="alert mt-4">
          <span>Loading report data...</span>
        </div>
      ) : null}

      {isError ? (
        <div className="alert alert-error mt-4">
          <span>{extractApiMessage(error) || "Failed to load reports."}</span>
        </div>
      ) : null}

      {!isLoading && !isError && points.length === 0 ? (
        <div className="alert mt-4">
          <span>No report data found for this boundary.</span>
        </div>
      ) : null}

      <div className="grid grid-cols-1 gap-6 md:grid-cols-2 mt-4">
        <div className="card p-4">
          <h2 className="text-lg font-medium mb-2">Z2 Speed</h2>
          <Charts type="z2_speed" points={points} range={range} />
        </div>

        <div className="card p-4">
          <h2 className="text-lg font-medium mb-2">Decoupling</h2>
          <Charts type="decoupling" points={points} range={range} />
        </div>

        <div className="card p-4">
          <h2 className="text-lg font-medium mb-2">Climbing Pace (feet/week)</h2>
          <Charts type="climbing_pace" points={points} range={range} />
        </div>

        <div className="card p-4">
          <h2 className="text-lg font-medium mb-2">Heart Rate Zones</h2>
          <Charts type="hr_zones" points={points} range={range} />
        </div>

        <div className="card p-4 md:col-span-2">
          <h2 className="text-lg font-medium mb-2">Elevation</h2>
          <Charts type="elevation" points={points} range={range} />
        </div>
      </div>

      {!isLoading && !isError && isFetching ? (
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

function parseTimeRange(value: string | null): TimeRange {
  switch (value) {
    case "day":
    case "week":
    case "month":
    case "3month":
    case "6month":
    case "1year":
    case "2year":
      return value;
    default:
      return "month";
  }
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

function standaloneReportIdFor(
  reportId: ReportId,
): "ride_summary" | "endurance" | "climbing" | "fatigue" | "compare_rides" | null {
  switch (reportId) {
    case "ride_summary":
    case "endurance":
    case "climbing":
    case "fatigue":
    case "compare_rides":
      return reportId;
    default:
      return null;
  }
}

function dateRangeForTimeRange(range: TimeRange) {
  const end = new Date();
  const start = new Date(end);

  switch (range) {
    case "day":
      start.setDate(end.getDate() - 1);
      break;
    case "week":
      start.setDate(end.getDate() - 7);
      break;
    case "month":
      start.setDate(end.getDate() - 30);
      break;
    case "3month":
      start.setDate(end.getDate() - 90);
      break;
    case "6month":
      start.setDate(end.getDate() - 180);
      break;
    case "1year":
      start.setDate(end.getDate() - 365);
      break;
    case "2year":
      start.setDate(end.getDate() - 730);
      break;
  }

  return {
    startDate: formatDateInput(start),
    endDate: formatDateInput(end),
  };
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
