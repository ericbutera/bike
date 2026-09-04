"use client";

import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { useEffect, useMemo, useState } from "react";
import { type Segment, useSegments } from "../lib/queries";
import { AppCard, CardHeader } from "./ui/Card";
import InfoTooltip from "./ui/InfoTooltip";
import { LoadingSpinner } from "./ui/QueryState";
import SegmentEffortAnalysisSection from "./segment-detail/SegmentEffortAnalysisSection";

function segmentOptionLabel(segment: Segment) {
  const effortLabel =
    segment.effort_count === 1 ? "1 effort" : `${segment.effort_count} efforts`;
  return `${segment.title} (${effortLabel})`;
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
  const [selectedSegmentId, setSelectedSegmentId] = useState(
    searchParams.get("segment_id") ?? "",
  );
  const selectedSegment = eligibleSegments.find(
    (segment) => segment.id.toString() === selectedSegmentId,
  );

  useEffect(() => {
    if (segmentsQuery.isLoading || eligibleSegments.length === 0) {
      return;
    }

    if (
      !selectedSegmentId ||
      !eligibleSegments.some(
        (segment) => segment.id.toString() === selectedSegmentId,
      )
    ) {
      setSelectedSegmentId(eligibleSegments[0].id.toString());
    }
  }, [eligibleSegments, segmentsQuery.isLoading, selectedSegmentId]);

  useEffect(() => {
    const params = new URLSearchParams(searchParams);
    if (selectedSegmentId) {
      params.set("segment_id", selectedSegmentId);
    } else {
      params.delete("segment_id");
    }

    const nextQuery = params.toString();
    if (nextQuery !== searchParams.toString()) {
      router.replace(`/segments/analysis${nextQuery ? `?${nextQuery}` : ""}`, {
        scroll: false,
      });
    }
  }, [router, searchParams, selectedSegmentId]);

  return (
    <div className="grid gap-6">
      <h1 className="text-2xl font-semibold">Segment Analysis</h1>

      <AppCard as="section" bodyClassName="gap-5">
        <CardHeader
          title="Segment"
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
            <label className="form-control max-w-xl">
              <div className="label">
                <span className="label-text font-medium">Choose a segment</span>
              </div>
              <select
                className="select select-bordered w-full"
                value={selectedSegmentId}
                onChange={(event) => {
                  setSelectedSegmentId(event.target.value);
                }}
              >
                {eligibleSegments.map((segment) => (
                  <option key={segment.id} value={segment.id}>
                    {segmentOptionLabel(segment)}
                  </option>
                ))}
              </select>
            </label>
            {selectedSegment ? (
              <Link
                href={`/segments/${selectedSegment.id}`}
                className="btn btn-ghost btn-sm"
              >
                Detail
              </Link>
            ) : null}
          </div>
        )}
      </AppCard>

      {selectedSegment ? (
        <SegmentEffortAnalysisSection segment={selectedSegment} />
      ) : null}
    </div>
  );
}
