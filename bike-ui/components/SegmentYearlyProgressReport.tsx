"use client";

import ResponsiveChart from "./ui/ResponsiveChart";

import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { useEffect, useMemo } from "react";
import {
  CartesianGrid,
  Line,
  LineChart,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import {
  formatActivityTimestamp,
  formatDistance,
  formatDuration,
} from "../lib/activityFormatting";
import {
  type Segment,
  type SegmentYearlyBest,
  useSegments,
  useSegmentYearlyBests,
} from "../lib/queries";
import { useUnitPreferences } from "../lib/unitPreferences";
import { AppCard, CardHeader } from "./ui/Card";
import InfoTooltip from "./ui/InfoTooltip";
import { LoadingSpinner } from "./ui/QueryState";

type ChartPoint = {
  year: number;
  duration: number;
  label: string;
};

function formatChartDuration(value: unknown) {
  return typeof value === "number" ? `${Math.round(value)} sec` : `${value}`;
}

function formatCompactDuration(value?: number | null) {
  if (value == null || value <= 0) {
    return "--";
  }

  const minutes = Math.floor(value / 60);
  const seconds = value % 60;

  return minutes > 0
    ? `${minutes}:${String(seconds).padStart(2, "0")}`
    : `${seconds}s`;
}

function formatSignedSeconds(value?: number | null) {
  if (value == null) {
    return "--";
  }

  if (value === 0) {
    return "0 sec";
  }

  return value > 0 ? `${value} sec faster` : `${Math.abs(value)} sec slower`;
}

function segmentOptionLabel(segment: Segment) {
  const effortLabel =
    segment.effort_count === 1 ? "1 effort" : `${segment.effort_count} efforts`;
  return `${segment.title} (${effortLabel})`;
}

function buildSummary(yearlyBests: SegmentYearlyBest[]) {
  if (yearlyBests.length === 0) {
    return "No efforts have been recorded for this segment yet.";
  }

  if (yearlyBests.length === 1) {
    const only = yearlyBests[0];
    return `Only ${only.year} has a recorded result so far: ${formatCompactDuration(
      only.duration_seconds,
    )}.`;
  }

  const first = yearlyBests[0];
  const latest = yearlyBests[yearlyBests.length - 1];
  const changeSeconds = first.duration_seconds - latest.duration_seconds;
  const changePercent =
    first.duration_seconds > 0
      ? Math.round((changeSeconds / first.duration_seconds) * 100)
      : 0;
  const direction = changeSeconds >= 0 ? "reduction" : "increase";

  return `Overall change: ${formatCompactDuration(
    first.duration_seconds,
  )} to ${formatCompactDuration(latest.duration_seconds)}, a ${Math.abs(
    changeSeconds,
  )}-second ${direction} (${Math.abs(changePercent)}%) from ${first.year} to ${
    latest.year
  }.`;
}

export default function SegmentYearlyProgressReport() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const { unitSystem } = useUnitPreferences();
  const segmentsQuery = useSegments();
  const eligibleSegments = useMemo(
    () =>
      (segmentsQuery.data ?? [])
        .filter((segment) => segment.current_user_pr_duration_seconds != null)
        .sort(
          (left, right) =>
            left.title.localeCompare(right.title, undefined, {
              sensitivity: "base",
            }) || left.id - right.id,
        ),
    [segmentsQuery.data],
  );
  const requestedSegmentId = searchParams.get("segment_id") ?? "";
  const selectedSegment =
    eligibleSegments.find(
      (segment) => segment.id.toString() === requestedSegmentId,
    ) ?? eligibleSegments[0];
  const selectedSegmentId =
    selectedSegment?.id.toString() ?? requestedSegmentId;
  const yearlyBestsQuery = useSegmentYearlyBests(selectedSegment?.id);
  const yearlyBests = yearlyBestsQuery.data?.years ?? [];
  const chartData = yearlyBests.map<ChartPoint>((best) => ({
    year: best.year,
    duration: best.duration_seconds,
    label: formatCompactDuration(best.duration_seconds),
  }));

  useEffect(() => {
    if (segmentsQuery.isLoading || !selectedSegment) {
      return;
    }

    if (requestedSegmentId !== selectedSegmentId) {
      const params = new URLSearchParams(searchParams.toString());
      params.set("segment_id", selectedSegmentId);
      router.replace(`/segments/progress?${params.toString()}`, {
        scroll: false,
      });
    }
  }, [
    router,
    searchParams,
    segmentsQuery.isLoading,
    requestedSegmentId,
    selectedSegment,
    selectedSegmentId,
  ]);

  return (
    <div className="grid gap-6">
      <AppCard as="section" bodyClassName="gap-5">
        <CardHeader
          title="Segment progress"
          titleExtras={
            <InfoTooltip
              label="Segment progress details"
              tip="Choose one of your segments with recorded efforts to see the fastest time from each year."
            />
          }
        />
        {segmentsQuery.isLoading ? (
          <div className="flex items-center gap-2 text-sm text-base-content/60">
            <LoadingSpinner size="sm" />
            <span>Loading segments...</span>
          </div>
        ) : eligibleSegments.length === 0 ? (
          <div className="alert">
            <span>No segment efforts found yet.</span>
          </div>
        ) : (
          <label className="form-control max-w-xl">
            <span className="label">
              <span className="label-text">Choose a segment</span>
            </span>
            <select
              className="select w-full"
              value={selectedSegmentId}
              onChange={(event) => {
                const params = new URLSearchParams(searchParams.toString());
                params.set("segment_id", event.target.value);
                router.replace(`/segments/progress?${params.toString()}`, {
                  scroll: false,
                });
              }}
            >
              {eligibleSegments.map((segment) => (
                <option key={segment.id} value={segment.id}>
                  {segmentOptionLabel(segment)}
                </option>
              ))}
            </select>
          </label>
        )}

        {selectedSegment ? (
          yearlyBestsQuery.isLoading ? (
            <div className="flex justify-center py-12">
              <LoadingSpinner size="md" />
            </div>
          ) : yearlyBests.length === 0 ? (
            <div className="alert">
              <span>No yearly efforts found for this segment.</span>
            </div>
          ) : (
            <>
              <div className="h-80 w-full">
                <ResponsiveChart width="100%" height="100%">
                  <LineChart
                    data={chartData}
                    margin={{ top: 12, right: 24, bottom: 8, left: 8 }}
                  >
                    <CartesianGrid strokeDasharray="4 4" />
                    <XAxis dataKey="year" />
                    <YAxis
                      width={72}
                      tickFormatter={formatChartDuration}
                      domain={["dataMin - 5", "dataMax + 5"]}
                    />
                    <Tooltip
                      formatter={(value) => [
                        formatChartDuration(value),
                        "Time",
                      ]}
                      labelFormatter={(label) => `${label}`}
                    />
                    <Line
                      type="monotone"
                      dataKey="duration"
                      name="Fastest"
                      stroke="#2563eb"
                      strokeWidth={3}
                      dot={{ r: 4, strokeWidth: 2 }}
                      activeDot={{ r: 6 }}
                    />
                  </LineChart>
                </ResponsiveChart>
              </div>

              <p className="max-w-3xl text-sm leading-6 text-base-content/70">
                {buildSummary(yearlyBests)}
              </p>

              <div className="overflow-x-auto">
                <table className="table table-zebra">
                  <thead>
                    <tr>
                      <th>Year</th>
                      <th>Fastest</th>
                      <th>YoY</th>
                      <th>Since first</th>
                      <th>Ride</th>
                      <th>Date</th>
                    </tr>
                  </thead>
                  <tbody>
                    {yearlyBests.map((best) => (
                      <tr key={best.year}>
                        <td className="font-semibold">{best.year}</td>
                        <td>{formatDuration(best.duration_seconds)}</td>
                        <td>
                          {formatSignedSeconds(
                            best.improvement_from_previous_year_seconds,
                          )}
                        </td>
                        <td>
                          {formatSignedSeconds(
                            best.improvement_from_first_year_seconds,
                          )}
                        </td>
                        <td>
                          <Link
                            href={`/activities/${best.activity_id}`}
                            className="link-hover link font-medium"
                          >
                            {best.activity_title}
                          </Link>
                          <span className="ml-2 text-xs text-base-content/55">
                            #{best.effort_index}
                          </span>
                        </td>
                        <td>
                          {formatActivityTimestamp(best.activity_started_at)}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </>
          )
        ) : null}
      </AppCard>
    </div>
  );
}
