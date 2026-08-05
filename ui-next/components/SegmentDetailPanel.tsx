"use client";

import { useRouter } from "next/navigation";
import { useMemo, useState } from "react";
import toast from "react-hot-toast";
import {
  useDeleteSegment,
  useUpdateSegment,
  type SegmentMode,
} from "../lib/queries";
import {
  EMPTY_EFFORT_IDS,
  buildLiveLeaderComparisonRows,
  playbackTargetSeconds,
  type PlaybackPace,
} from "../lib/segmentDetail";
import SegmentDetailComparisonSection from "./segment-detail/SegmentDetailComparisonSection";
import SegmentDetailEffortsSection from "./segment-detail/SegmentDetailEffortsSection";
import SegmentDetailHeader from "./segment-detail/SegmentDetailHeader";
import {
  useSegmentEffortsContainer,
  useSegmentPlayback,
  useSegmentPerformanceSummary,
  useSegmentRouteMetrics,
  useSegmentTitleEditor,
  useSegmentWithComparison,
} from "./segment-detail/useSegmentDetailState";

export default function SegmentDetailPanel({
  segmentId,
  initialSelectedEffortIds = EMPTY_EFFORT_IDS,
}: {
  segmentId: number | string;
  initialSelectedEffortIds?: number[];
}) {
  const router = useRouter();
  const { segmentQuery, comparisonQuery, segment } =
    useSegmentWithComparison(segmentId);
  const updateSegmentMutation = useUpdateSegment();
  const deleteSegmentMutation = useDeleteSegment();
  const isComparisonLoading =
    Boolean(segmentQuery.data) && comparisonQuery.isLoading;
  const routeMetrics = useSegmentRouteMetrics(segment);
  const performance = useSegmentPerformanceSummary(segment);
  const titleEditor = useSegmentTitleEditor({
    segment,
    updateTitle: updateSegmentMutation.updateAsync,
  });
  const efforts = useSegmentEffortsContainer({
    segmentId,
    segment,
    initialSelectedEffortIds,
  });
  const [playbackPace, setPlaybackPace] = useState<PlaybackPace>("auto");
  const playbackLimitSeconds = Math.max(
    0,
    ...efforts.selectedRows.map((row) => row.effort.duration_seconds),
  );
  const targetPlaybackDurationSeconds = playbackTargetSeconds(
    playbackLimitSeconds,
    playbackPace,
  );
  const playbackRate =
    targetPlaybackDurationSeconds > 0
      ? playbackLimitSeconds / targetPlaybackDurationSeconds
      : 0;
  const playback = useSegmentPlayback({
    playbackLimitSeconds,
    playbackRate,
  });
  const liveComparisonRows = useMemo(
    () => buildLiveLeaderComparisonRows(efforts.selectedRows, playback.seconds),
    [efforts.selectedRows, playback.seconds],
  );
  const raceViewerHref = efforts.raceViewerHref(playbackPace);
  const builderEditHref = segment?.builder_source
    ? `/segments/builder?segmentId=${segment.id}`
    : null;

  async function handleUpdateSegmentMode(mode: SegmentMode) {
    if (!segment || segment.mode === mode) {
      return;
    }

    try {
      const updatedSegment = await updateSegmentMutation.updateAsync({
        id: segment.id,
        mode,
      });

      toast.success(
        `Segment mode set to ${updatedSegment.mode.toUpperCase()}.`,
      );
    } catch {
      // The mutation exposes the API error state used below.
    }
  }

  async function handleDeleteSegment() {
    if (!segment) {
      return;
    }

    const confirmed =
      typeof globalThis.confirm === "function"
        ? globalThis.confirm(
            "Delete this segment? This removes the segment and clears matched efforts tied to it.",
          )
        : true;

    if (!confirmed) {
      return;
    }

    try {
      await deleteSegmentMutation.deleteAsync(segment.id);
      toast.success("Segment deleted.");
      router.push("/segments");
    } catch {
      // The mutation exposes the API error state used below.
    }
  }

  if (!segment) {
    return null;
  }

  return (
    <section className="space-y-6">
      <SegmentDetailHeader
        segment={segment}
        metrics={routeMetrics}
        performance={performance}
        editor={titleEditor}
        status={{
          isSavingSegment: updateSegmentMutation.isPending,
          isDeletingSegment: deleteSegmentMutation.isPending,
        }}
        links={{ builderEditHref }}
        actions={{
          changeSegmentMode: (mode) => {
            void handleUpdateSegmentMode(mode);
          },
          deleteSegment: () => {
            void handleDeleteSegment();
          },
        }}
      />

      <SegmentDetailEffortsSection
        segment={segment}
        effortList={efforts.effortList}
        performance={performance}
        isLoading={isComparisonLoading}
      />

      <SegmentDetailComparisonSection
        isLoading={isComparisonLoading}
        unitSystem={routeMetrics.unitSystem}
        workspace={{
          routePoints: segment.route_points,
          routeDistanceMeters: routeMetrics.routeDistanceMeters,
          selectedEffortIds: efforts.selectedEffortIds,
          selectedRows: efforts.selectedRows,
          liveComparisonRows,
          focusedEffortId: efforts.focusedEffortId,
          raceViewerHref,
          actions: {
            hoverEffort: efforts.actions.hoverEffort,
            removeEffort: efforts.actions.removeEffort,
          },
        }}
        playback={{
          ...playback,
          pace: playbackPace,
          setPace: setPlaybackPace,
        }}
      />
    </section>
  );
}
