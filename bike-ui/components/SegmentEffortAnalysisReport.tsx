"use client";

import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { useEffect, useMemo, useState } from "react";
import { formatDuration } from "../lib/activityFormatting";
import {
  type Segment,
  type SegmentAnalysisEffortSummary,
  useSegmentEffortAnalysis,
  useSegments,
} from "../lib/queries";
import SegmentEffortAnalysisSection from "./segment-detail/SegmentEffortAnalysisSection";
import { AppCard, CardHeader } from "./ui/Card";
import InfoTooltip from "./ui/InfoTooltip";
import { LoadingSpinner } from "./ui/QueryState";

function segmentOptionLabel(segment: Segment) {
  const effortLabel =
    segment.effort_count === 1 ? "1 effort" : `${segment.effort_count} efforts`;
  return `${segment.title} (${effortLabel})`;
}

function effortLabel(effort: SegmentAnalysisEffortSummary) {
  return `${formatDuration(effort.duration_seconds)} - ${effort.activity_title}`;
}

function effortById(
  efforts: SegmentAnalysisEffortSummary[],
  effortId: number | null,
) {
  return efforts.find((effort) => effort.effort_id === effortId) ?? null;
}

export default function SegmentEffortAnalysisReport() {
  const router = useRouter();
  const searchParams = useSearchParams();
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
  const [splitCount, setSplitCount] = useState<number>(10);
  const [showAllEfforts, setShowAllEfforts] = useState(false);
  const [selectedEffortId, setSelectedEffortId] = useState<number | null>(null);
  const analysisQuery = useSegmentEffortAnalysis(selectedSegment?.id, {
    splitCount,
    effortLimit: showAllEfforts ? 0 : undefined,
  });
  const analysis = analysisQuery.data;
  const efforts = analysis?.efforts ?? [];
  const referenceEffort = analysis?.reference_effort ?? null;
  const effectiveSelectedEffortId =
    selectedEffortId != null &&
    efforts.some((effort) => effort.effort_id === selectedEffortId)
      ? selectedEffortId
      : (referenceEffort?.effort_id ?? null);
  const selectedEffort = effortById(efforts, effectiveSelectedEffortId);

  function selectSegment(segmentId: string) {
    const params = new URLSearchParams(searchParams.toString());
    params.set("segment_id", segmentId);
    router.replace(`/segments/analysis?${params.toString()}`, {
      scroll: false,
    });
  }

  useEffect(() => {
    if (segmentsQuery.isLoading || !selectedSegment) {
      return;
    }

    if (requestedSegmentId !== selectedSegmentId) {
      const params = new URLSearchParams(searchParams.toString());
      params.set("segment_id", selectedSegmentId);
      router.replace(`/segments/analysis?${params.toString()}`, {
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
          title="Segment Analysis"
          titleExtras={
            <InfoTooltip
              label="Segment analysis details"
              tip="Choose one of your segments with recorded efforts before running distance-normalized effort analysis."
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
          <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-end">
            <fieldset className="fieldset max-w-3xl">
              <div className="grid gap-3 md:grid-cols-2">
                <div>
                  <label className="label">Choose a segment</label>
                  <select
                    className="select w-full"
                    value={selectedSegmentId}
                    onChange={(event) => {
                      selectSegment(event.target.value);
                    }}
                  >
                    {eligibleSegments.map((segment) => (
                      <option key={segment.id} value={segment.id}>
                        {segmentOptionLabel(segment)}
                      </option>
                    ))}
                  </select>
                  {analysis && analysis.total_effort_count > 50 ? (
                    <button
                      type="button"
                      className="btn btn-link btn-sm px-0"
                      onClick={() => setShowAllEfforts((current) => !current)}
                    >
                      {showAllEfforts
                        ? "Show fastest 50"
                        : `Show all ${analysis.total_effort_count} efforts`}
                    </button>
                  ) : null}
                </div>

                <div>
                  <label className="label">Selected ride</label>
                  <select
                    className="select w-full"
                    value={selectedEffort?.effort_id ?? ""}
                    disabled={analysisQuery.isLoading || efforts.length === 0}
                    onChange={(event) => {
                      setSelectedEffortId(Number(event.target.value));
                    }}
                  >
                    {efforts.map((effort) => (
                      <option key={effort.effort_id} value={effort.effort_id}>
                        {effort.effort_id === referenceEffort?.effort_id
                          ? `PR - ${effortLabel(effort)}`
                          : effortLabel(effort)}
                      </option>
                    ))}
                  </select>
                </div>
              </div>
            </fieldset>
          </div>
        )}

        {selectedSegment ? (
          <SegmentEffortAnalysisSection
            analysis={analysis}
            isAnalysisLoading={analysisQuery.isLoading}
            selectedEffortId={effectiveSelectedEffortId}
            splitCount={splitCount}
            setSplitCount={setSplitCount}
          />
        ) : null}
      </AppCard>
    </div>
  );
}
