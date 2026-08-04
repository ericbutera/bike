"use client";

import { useRouter } from "next/navigation";
import { useEffect, useMemo, useRef, useState } from "react";
import toast from "react-hot-toast";
import { formatDuration } from "../lib/activityFormatting";
import {
  useDeleteSegment,
  useSegment,
  useSegmentComparison,
  useUpdateSegment,
  type SegmentEffort,
  type SegmentMode,
} from "../lib/queries";
import {
  EFFORT_COLORS,
  EMPTY_EFFORTS,
  EMPTY_EFFORT_IDS,
  PLAYBACK_END_EPSILON,
  areEffortIdListsEqual,
  buildLiveComparisonRows,
  fastestEffort,
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
import { useAuthenticatedUser } from "./RequireAuth";
import SegmentDetailComparisonSection from "./segment-detail/SegmentDetailComparisonSection";
import SegmentDetailEffortsSection from "./segment-detail/SegmentDetailEffortsSection";
import SegmentDetailHeader from "./segment-detail/SegmentDetailHeader";

export default function SegmentDetailPanel({
  segmentId,
  initialSelectedEffortIds = EMPTY_EFFORT_IDS,
  initialReferenceEffortId = null,
}: {
  segmentId: number | string;
  initialSelectedEffortIds?: number[];
  initialReferenceEffortId?: number | null;
}) {
  const router = useRouter();
  const user = useAuthenticatedUser();
  const { unitSystem } = useUnitPreferences();
  const segmentQuery = useSegment(segmentId);
  const comparisonQuery = useSegmentComparison(
    segmentQuery.data ? segmentId : null,
  );
  const updateSegmentMutation = useUpdateSegment();
  const deleteSegmentMutation = useDeleteSegment();
  const isComparisonLoading =
    Boolean(segmentQuery.data) && comparisonQuery.isLoading;
  const requestedSelectionBySegmentIdRef = useRef(
    new Map<number, number[]>([[Number(segmentId), initialSelectedEffortIds]]),
  );
  const [selectedEffortIds, setSelectedEffortIds] = useState<number[]>(
    initialSelectedEffortIds,
  );
  const initializedSelectionSegmentIdRef = useRef<number | null>(null);
  const playbackAnimationFrameRef = useRef<number | null>(null);
  const playbackLastTimestampRef = useRef<number | null>(null);
  const [playbackSeconds, setPlaybackSeconds] = useState(0);
  const [isPlaying, setIsPlaying] = useState(false);
  const [playbackPace, setPlaybackPace] = useState<PlaybackPace>("auto");
  const [hoveredEffortId, setHoveredEffortId] = useState<number | null>(null);
  const [pinnedEffortId, setPinnedEffortId] = useState<number | null>(
    initialReferenceEffortId,
  );
  const [effortTimeFilter, setEffortTimeFilter] =
    useState<EffortTimeFilter>("all");
  const [isEditingTitle, setIsEditingTitle] = useState(false);
  const [draftTitle, setDraftTitle] = useState("");
  const segment = useMemo(
    () =>
      segmentQuery.data
        ? {
            ...segmentQuery.data,
            route_points:
              comparisonQuery.data?.route_points ??
              segmentQuery.data.route_points ??
              [],
            efforts:
              comparisonQuery.data?.efforts ?? segmentQuery.data.efforts ?? [],
          }
        : null,
    [comparisonQuery.data, segmentQuery.data],
  );
  const allEfforts = segment?.efforts ?? EMPTY_EFFORTS;
  const visibleEfforts = filterEffortsByTimeWindow(
    segment?.efforts,
    effortTimeFilter,
  );
  const currentUserId = typeof user.id === "number" ? user.id : null;
  const currentUserName = user.name?.trim() || null;
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
  const raceViewerHref = useMemo(() => {
    if (!segment?.id) {
      return null;
    }

    const searchParams = new URLSearchParams();

    if (selectedEffortIds.length > 0) {
      searchParams.set("efforts", selectedEffortIds.join(","));
    }

    if (referenceEffortId != null) {
      searchParams.set("ref", String(referenceEffortId));
    }

    if (playbackPace !== "auto") {
      searchParams.set("pace", playbackPace);
    }

    const queryString = searchParams.toString();

    return `/segments/${segment.id}/race${queryString ? `?${queryString}` : ""}`;
  }, [playbackPace, referenceEffortId, segment?.id, selectedEffortIds]);
  const referenceSummaryLabel = referenceEffort
    ? `${referenceEffort.rider_name} · ${formatDuration(referenceEffort.duration_seconds)}`
    : "No reference ride";

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
      const availableIds = new Set(allEfforts.map((effort) => effort.id));
      const requested = (
        requestedSelectionBySegmentIdRef.current.get(segment.id) ?? []
      ).filter((id) => availableIds.has(id));

      if (requested.length > 0 && shouldSeedSelection) {
        return areEffortIdListsEqual(current, requested) ? current : requested;
      }

      const valid = current.filter((id) => availableIds.has(id));

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

  if (!segment) {
    return null;
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
        isSavingSegment={updateSegmentMutation.isPending}
        isDeletingSegment={deleteSegmentMutation.isPending}
        builderEditHref={builderEditHref}
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
        onSegmentModeChange={(mode) => {
          void handleUpdateSegmentMode(mode);
        }}
        onDeleteSegment={() => {
          void handleDeleteSegment();
        }}
      />

      <SegmentDetailEffortsSection
        segment={segment}
        visibleEfforts={visibleEfforts}
        selectedEffortIds={selectedEffortIds}
        selectedRows={selectedRows}
        overallRankByEffortId={overallRankByEffortId}
        currentUserPr={currentUserPr}
        isLoading={isComparisonLoading}
        effortTimeFilter={effortTimeFilter}
        onEffortTimeFilterChange={setEffortTimeFilter}
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
        raceViewerHref={raceViewerHref}
        isLoading={isComparisonLoading}
        playbackLimitSeconds={playbackLimitSeconds}
        playbackSeconds={playbackSeconds}
        isPlaying={isPlaying}
        playbackPace={playbackPace}
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
