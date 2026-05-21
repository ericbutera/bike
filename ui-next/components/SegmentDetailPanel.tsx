"use client";

import { auth } from "@ericbutera/kaleido";
import { useRouter } from "next/navigation";
import { useEffect, useMemo, useRef, useState } from "react";
import toast from "react-hot-toast";
import { extractApiMessage, formatDuration } from "../lib/activityFormatting";
import {
  useDeleteSegment,
  useSegment,
  useUpdateSegment,
  type SegmentEffort,
} from "../lib/queries";
import {
  EFFORT_COLORS,
  EMPTY_EFFORTS,
  EMPTY_EFFORT_IDS,
  PLAYBACK_END_EPSILON,
  areEffortIdListsEqual,
  buildLiveComparisonRows,
  fastestEffort,
  filterEffortsBySearchQuery,
  filterEffortsByTimeWindow,
  overallEffortRanks,
  playbackTargetSeconds,
  resolveRouteDistanceMeters,
  resolveRouteNetElevationMeters,
  type EffortTimeFilter,
  type PlaybackPace,
  type SelectedEffortRow,
} from "../lib/segmentDetail";
import { useUnitPreferences } from "../lib/unitPreferences";
import AuthRequiredCard from "./AuthRequiredCard";
import SegmentDetailComparisonSection from "./segment-detail/SegmentDetailComparisonSection";
import SegmentDetailEffortsSection from "./segment-detail/SegmentDetailEffortsSection";
import SegmentDetailHeader from "./segment-detail/SegmentDetailHeader";

export default function SegmentDetailPanel({
  segmentId,
}: {
  segmentId: number | string;
}) {
  const authApi = auth.useAuthApi();
  const router = useRouter();
  const { user, isLoading: isLoadingUser } = authApi.useCurrentUser();
  const { unitSystem } = useUnitPreferences();
  const segmentQuery = useSegment(user ? segmentId : null);
  const updateSegmentMutation = useUpdateSegment();
  const deleteSegmentMutation = useDeleteSegment();
  const [selectedEffortIds, setSelectedEffortIds] = useState<number[]>([]);
  const initializedSelectionSegmentIdRef = useRef<number | null>(null);
  const playbackAnimationFrameRef = useRef<number | null>(null);
  const playbackLastTimestampRef = useRef<number | null>(null);
  const [playbackSeconds, setPlaybackSeconds] = useState(0);
  const [isPlaying, setIsPlaying] = useState(false);
  const [playbackPace, setPlaybackPace] = useState<PlaybackPace>("auto");
  const [hoveredEffortId, setHoveredEffortId] = useState<number | null>(null);
  const [pinnedEffortId, setPinnedEffortId] = useState<number | null>(null);
  const [effortTimeFilter, setEffortTimeFilter] =
    useState<EffortTimeFilter>("all");
  const [effortSearchQuery, setEffortSearchQuery] = useState("");
  const [isEditingTitle, setIsEditingTitle] = useState(false);
  const [draftTitle, setDraftTitle] = useState("");
  const segment = segmentQuery.data;
  const allEfforts = segment?.efforts ?? EMPTY_EFFORTS;
  const visibleEfforts = filterEffortsByTimeWindow(
    segment?.efforts,
    effortTimeFilter,
  );
  const filteredVisibleEfforts = useMemo(
    () => filterEffortsBySearchQuery(visibleEfforts, effortSearchQuery),
    [effortSearchQuery, visibleEfforts],
  );
  const currentUserId = user?.id ?? null;
  const currentUserName = user?.name?.trim() || null;
  const overallRankByEffortId = overallEffortRanks(allEfforts);
  const overallKom = fastestEffort(allEfforts);
  const currentUserPr =
    currentUserId != null
      ? fastestEffort(
          allEfforts.filter((effort) => effort.rider_user_id === currentUserId),
        )
      : currentUserName
        ? fastestEffort(
            allEfforts.filter(
              (effort) => effort.rider_name === currentUserName,
            ),
          )
        : allEfforts.length > 0 &&
            new Set(allEfforts.map((effort) => effort.rider_name)).size === 1
          ? fastestEffort(allEfforts)
          : null;
  const currentUserPrDurationSeconds =
    currentUserPr?.duration_seconds ??
    segment?.current_user_pr_duration_seconds ??
    null;
  const currentUserPrLabel = currentUserPr
    ? currentUserPr.activity_title
    : segment?.current_user_pr_duration_seconds != null
      ? "Personal best across matched efforts"
      : "No PR yet";
  const selectedEfforts = useMemo(() => {
    const effortById = new Map(allEfforts.map((effort) => [effort.id, effort]));

    return selectedEffortIds
      .map((id) => effortById.get(id))
      .filter((effort): effort is SegmentEffort => Boolean(effort));
  }, [allEfforts, selectedEffortIds]);
  const selectedRows = useMemo(
    (): SelectedEffortRow[] =>
      selectedEfforts.map((effort, index) => ({
        effort,
        color: EFFORT_COLORS[index % EFFORT_COLORS.length],
        markerLabel: String(index + 1),
      })),
    [selectedEfforts],
  );
  const focusedEffortId = hoveredEffortId ?? pinnedEffortId;
  const referenceEffortId =
    selectedEfforts.find((effort) => effort.id === pinnedEffortId)?.id ??
    selectedEfforts.find((effort) => {
      if (currentUserId != null) {
        return effort.rider_user_id === currentUserId;
      }

      return currentUserName ? effort.rider_name === currentUserName : false;
    })?.id ??
    selectedEfforts[0]?.id ??
    null;
  const referenceEffort =
    selectedEfforts.find((effort) => effort.id === referenceEffortId) ?? null;
  const playbackLimitSeconds = referenceEffort?.duration_seconds ?? 0;
  const targetPlaybackDurationSeconds = playbackTargetSeconds(
    playbackLimitSeconds,
    playbackPace,
  );
  const liveComparisonRows = useMemo(
    () =>
      buildLiveComparisonRows(selectedRows, referenceEffort, playbackSeconds),
    [playbackSeconds, referenceEffort, selectedRows],
  );
  const routeDistanceMeters = resolveRouteDistanceMeters(
    segment?.route_points,
    segment?.distance_meters,
  );
  const routeNetElevationMeters = resolveRouteNetElevationMeters(
    segment?.route_points,
  );
  const routeGradePercent =
    routeDistanceMeters && routeNetElevationMeters != null
      ? (routeNetElevationMeters / routeDistanceMeters) * 100
      : null;
  const builderEditHref = segment?.builder_source
    ? `/segments/builder?segmentId=${segment.id}`
    : null;
  const comparisonSelectionLabel =
    selectedRows.length === 1
      ? "1 ride selected"
      : `${selectedRows.length} rides selected`;
  const referenceSummaryLabel = referenceEffort
    ? `${referenceEffort.rider_name} · ${formatDuration(referenceEffort.duration_seconds)}`
    : "No reference ride";

  useEffect(() => {
    setEffortSearchQuery("");
  }, [segment?.id]);

  useEffect(() => {
    setIsEditingTitle(false);
    setDraftTitle(segment?.title ?? "");
  }, [segment?.id, segment?.title]);

  useEffect(() => {
    if (!segment || allEfforts.length === 0) {
      initializedSelectionSegmentIdRef.current = null;
      setSelectedEffortIds((current) =>
        current.length === 0 ? current : EMPTY_EFFORT_IDS,
      );
      setPlaybackSeconds((current) => (current === 0 ? current : 0));
      setIsPlaying((current) => (current ? false : current));
      return;
    }

    const shouldSeedSelection =
      initializedSelectionSegmentIdRef.current !== segment.id;
    initializedSelectionSegmentIdRef.current = segment.id;

    setSelectedEffortIds((current) => {
      const valid = current.filter((id) =>
        allEfforts.some((effort) => effort.id === id),
      );

      if (valid.length > 0) {
        return areEffortIdListsEqual(current, valid) ? current : valid;
      }

      if (!shouldSeedSelection) {
        return current.length === 0 ? current : EMPTY_EFFORT_IDS;
      }

      const seeded = allEfforts
        .slice(0, Math.min(3, allEfforts.length))
        .map((effort) => effort.id);

      return areEffortIdListsEqual(current, seeded) ? current : seeded;
    });
  }, [allEfforts, segment?.id, segment]);

  useEffect(() => {
    if (
      pinnedEffortId != null &&
      !selectedEfforts.some((effort) => effort.id === pinnedEffortId)
    ) {
      setPinnedEffortId(null);
    }

    if (
      hoveredEffortId != null &&
      !selectedEfforts.some((effort) => effort.id === hoveredEffortId)
    ) {
      setHoveredEffortId(null);
    }
  }, [hoveredEffortId, pinnedEffortId, selectedEfforts]);

  function togglePinnedEffort(effortId: number) {
    setPinnedEffortId((current) => (current === effortId ? null : effortId));
  }

  function addEffortToComparison(effortId: number) {
    setSelectedEffortIds((current) => {
      if (current.includes(effortId)) {
        return current;
      }

      return [...current, effortId];
    });
  }

  function removeEffortFromComparison(effortId: number) {
    setSelectedEffortIds((current) => current.filter((id) => id !== effortId));
  }

  async function handleUpdateSegmentTitle() {
    if (!segment) {
      return;
    }

    const title = draftTitle.trim();

    if (!title) {
      toast.error("Segment name is required.");
      return;
    }

    if (title === segment.title) {
      setIsEditingTitle(false);
      setDraftTitle(segment.title);
      return;
    }

    try {
      const updatedSegment = await updateSegmentMutation.updateAsync({
        id: segment.id,
        title,
      });

      setDraftTitle(updatedSegment.title);
      setIsEditingTitle(false);
      toast.success(`Saved ${updatedSegment.title}.`);
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

  useEffect(() => {
    if (playbackLimitSeconds <= 0) {
      setPlaybackSeconds(0);
      setIsPlaying(false);
      return;
    }

    setPlaybackSeconds((current) => Math.min(current, playbackLimitSeconds));
  }, [playbackLimitSeconds]);

  useEffect(() => {
    if (!isPlaying || playbackLimitSeconds <= 0) {
      playbackLastTimestampRef.current = null;
      return undefined;
    }

    const tick = (timestamp: number) => {
      const previousTimestamp = playbackLastTimestampRef.current ?? timestamp;
      const deltaSeconds =
        targetPlaybackDurationSeconds > 0
          ? ((timestamp - previousTimestamp) / 1000) *
            (playbackLimitSeconds / targetPlaybackDurationSeconds)
          : 0;

      playbackLastTimestampRef.current = timestamp;

      let reachedEnd = false;

      setPlaybackSeconds((current) => {
        const next = Math.min(current + deltaSeconds, playbackLimitSeconds);

        if (next >= playbackLimitSeconds - PLAYBACK_END_EPSILON) {
          reachedEnd = true;
          return playbackLimitSeconds;
        }

        return next;
      });

      if (reachedEnd) {
        playbackLastTimestampRef.current = null;
        playbackAnimationFrameRef.current = null;
        setIsPlaying(false);
        return;
      }

      playbackAnimationFrameRef.current = window.requestAnimationFrame(tick);
    };

    playbackAnimationFrameRef.current = window.requestAnimationFrame(tick);

    return () => {
      if (playbackAnimationFrameRef.current != null) {
        window.cancelAnimationFrame(playbackAnimationFrameRef.current);
        playbackAnimationFrameRef.current = null;
      }
      playbackLastTimestampRef.current = null;
    };
  }, [isPlaying, playbackLimitSeconds, targetPlaybackDurationSeconds]);

  if (isLoadingUser || segmentQuery.isLoading) {
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
        eyebrow="Segment comparison"
        title="Sign in to compare segment efforts"
        description="Select attempts, then use time to open the full activity detail."
      />
    );
  }

  if (segmentQuery.isError || !segment) {
    return (
      <section className="card bg-base-100 shadow-xl">
        <div className="card-body">
          <div className="alert alert-error">
            {extractApiMessage(segmentQuery.error) || "Segment not found"}
          </div>
        </div>
      </section>
    );
  }

  return (
    <section className="space-y-6">
      <SegmentDetailHeader
        segment={segment}
        routeDistanceMeters={routeDistanceMeters}
        routeGradePercent={routeGradePercent}
        routeNetElevationMeters={routeNetElevationMeters}
        unitSystem={unitSystem}
        currentUserPr={currentUserPr}
        currentUserName={currentUserName}
        currentUserPrDurationSeconds={currentUserPrDurationSeconds}
        currentUserPrLabel={currentUserPrLabel}
        overallKom={overallKom}
        isEditingTitle={isEditingTitle}
        draftTitle={draftTitle}
        isSavingTitle={updateSegmentMutation.isPending}
        isDeletingSegment={deleteSegmentMutation.isPending}
        builderEditHref={builderEditHref}
        actionErrorMessage={
          updateSegmentMutation.isError
            ? extractApiMessage(updateSegmentMutation.error)
            : deleteSegmentMutation.isError
              ? extractApiMessage(deleteSegmentMutation.error)
              : null
        }
        onStartEditingTitle={() => {
          setDraftTitle(segment.title);
          setIsEditingTitle(true);
        }}
        onCancelEditingTitle={() => {
          setDraftTitle(segment.title);
          setIsEditingTitle(false);
        }}
        onDraftTitleChange={setDraftTitle}
        onSaveTitle={() => {
          void handleUpdateSegmentTitle();
        }}
        onDeleteSegment={() => {
          void handleDeleteSegment();
        }}
      />

      <SegmentDetailEffortsSection
        segment={segment}
        filteredVisibleEfforts={filteredVisibleEfforts}
        visibleEfforts={visibleEfforts}
        selectedEffortIds={selectedEffortIds}
        selectedRows={selectedRows}
        overallRankByEffortId={overallRankByEffortId}
        currentUserPr={currentUserPr}
        effortTimeFilter={effortTimeFilter}
        effortSearchQuery={effortSearchQuery}
        comparisonSelectionLabel={comparisonSelectionLabel}
        onEffortTimeFilterChange={setEffortTimeFilter}
        onEffortSearchQueryChange={setEffortSearchQuery}
        onAddEffort={addEffortToComparison}
        onRemoveEffort={removeEffortFromComparison}
      />

      <SegmentDetailComparisonSection
        routePoints={segment.route_points}
        routeDistanceMeters={routeDistanceMeters}
        selectedRows={selectedRows}
        liveComparisonRows={liveComparisonRows}
        focusedEffortId={focusedEffortId}
        referenceEffortId={referenceEffortId}
        referenceSummaryLabel={referenceSummaryLabel}
        playbackLimitSeconds={playbackLimitSeconds}
        playbackSeconds={playbackSeconds}
        isPlaying={isPlaying}
        playbackPace={playbackPace}
        targetPlaybackDurationSeconds={targetPlaybackDurationSeconds}
        unitSystem={unitSystem}
        onHoverEffort={setHoveredEffortId}
        onTogglePinnedEffort={togglePinnedEffort}
        onRemoveEffort={removeEffortFromComparison}
        onPlaybackSecondsChange={setPlaybackSeconds}
        onPlayingChange={setIsPlaying}
        onPlaybackPaceChange={setPlaybackPace}
      />
    </section>
  );
}
