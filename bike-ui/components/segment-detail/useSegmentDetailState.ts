"use client";

import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type SetStateAction,
} from "react";
import { toast } from "../../lib/toast";
import { type UnitSystem } from "../../lib/activityFormatting";
import {
  useSegment,
  useSegmentComparison,
  type Segment,
  type SegmentEffort,
} from "../../lib/queries";
import {
  EFFORT_COLORS,
  EMPTY_EFFORTS,
  PLAYBACK_END_EPSILON,
  fastestEffort,
  filterEffortsByTimeWindow,
  overallEffortRanks,
  resolveRouteDistanceMeters,
  resolveRouteNetElevationMeters,
  type EffortTimeFilter,
  type LiveComparisonRow,
  type PlaybackPace,
  type SelectedEffortRow,
} from "../../lib/segmentDetail";
import { useUnitPreferences } from "../../lib/unitPreferences";
import { useKeyedState } from "../../lib/useKeyedState";
import { getAuthenticatedUserId, useAuthenticatedUser } from "../RequireAuth";

export type SegmentRouteMetrics = {
  routeDistanceMeters: number | null | undefined;
  routeNetElevationMeters: number | null | undefined;
  routeGradePercent: number | null;
  unitSystem: UnitSystem;
};

export type SegmentPerformanceSummary = {
  currentUserPr: SegmentEffort | null;
  currentUserPrDurationSeconds: number | null;
  currentUserPrLabel: string;
  currentUserPrDisplayName: string;
  overallKom: SegmentEffort | null;
  overallRankByEffortId: Map<number, number>;
};

type SegmentCurrentRider = {
  id: number | null;
  name: string | null;
};

export type SegmentPlaybackState = {
  limitSeconds: number;
  seconds: number;
  isPlaying: boolean;
  setSeconds: (seconds: number) => void;
  setIsPlaying: (isPlaying: boolean) => void;
};

export type SegmentTitleEditor = {
  isEditingTitle: boolean;
  draftTitle: string;
  startEditingTitle: () => void;
  cancelEditingTitle: () => void;
  setDraftTitle: (value: string) => void;
  saveTitle: () => void;
};

export type SegmentComparisonWorkspace = {
  routePoints: Segment["route_points"];
  routeDistanceMeters: number | null | undefined;
  selectedEffortIds: number[];
  selectedRows: SelectedEffortRow[];
  liveComparisonRows: LiveComparisonRow[];
  focusedEffortId: number | null;
  raceViewerHref: string | null;
  actions: {
    hoverEffort: (effortId: number | null) => void;
    removeEffort: (effortId: number) => void;
  };
};

export type SegmentComparisonPlayback = SegmentPlaybackState & {
  pace: PlaybackPace;
  setPace: (pace: PlaybackPace) => void;
};

export type SegmentEffortListState = {
  visibleEfforts: SegmentEffort[];
  selectedEffortIds: number[];
  selectedRows: SelectedEffortRow[];
  effortTimeFilter: EffortTimeFilter;
  setEffortTimeFilter: (filter: EffortTimeFilter) => void;
  addEffort: (effortId: number) => void;
  removeEffort: (effortId: number) => void;
};

export type SegmentEffortsContainer = {
  effortList: SegmentEffortListState;
  selectedEffortIds: number[];
  selectedEfforts: SegmentEffort[];
  selectedRows: SelectedEffortRow[];
  focusedEffortId: number | null;
  raceViewerHref: (playbackPace?: PlaybackPace) => string | null;
  actions: {
    hoverEffort: (effortId: number | null) => void;
    addEffort: (effortId: number) => void;
    removeEffort: (effortId: number) => void;
  };
};

function useCurrentSegmentRider() {
  const user = useAuthenticatedUser();
  const currentUserId = getAuthenticatedUserId(user);
  const currentUserName = user.name?.trim() || null;

  return useMemo(
    () => ({
      id: currentUserId,
      name: currentUserName,
    }),
    [currentUserId, currentUserName],
  );
}

function currentRiderFastestEffort(
  efforts: SegmentEffort[],
  currentRider: SegmentCurrentRider,
) {
  if (currentRider.id != null) {
    return fastestEffort(
      efforts.filter((effort) => effort.rider_user_id === currentRider.id),
    );
  }

  if (currentRider.name) {
    return fastestEffort(
      efforts.filter((effort) => effort.rider_name === currentRider.name),
    );
  }

  return efforts.length > 0 &&
    new Set(efforts.map((effort) => effort.rider_name)).size === 1
    ? fastestEffort(efforts)
    : null;
}

function selectedEffortsForIds(
  efforts: SegmentEffort[],
  selectedEffortIds: number[],
) {
  const effortById = new Map(efforts.map((effort) => [effort.id, effort]));

  return selectedEffortIds
    .map((id) => effortById.get(id))
    .filter((effort): effort is SegmentEffort => Boolean(effort));
}

function selectedRowsForEfforts(selectedEfforts: SegmentEffort[]) {
  return selectedEfforts.map((effort, index) => ({
    effort,
    color: EFFORT_COLORS[index % EFFORT_COLORS.length],
    markerLabel: String(index + 1),
  }));
}

function segmentEffortSearchParams({
  selectedEffortIds,
  playbackPace,
}: {
  selectedEffortIds: number[];
  playbackPace?: PlaybackPace;
}) {
  const searchParams = new URLSearchParams();

  if (selectedEffortIds.length > 0) {
    searchParams.set("efforts", selectedEffortIds.join(","));
  }

  if (playbackPace && playbackPace !== "auto") {
    searchParams.set("pace", playbackPace);
  }

  return searchParams;
}

export function buildSegmentDetailHref({
  segmentId,
  selectedEffortIds,
}: {
  segmentId: number | string;
  selectedEffortIds: number[];
}) {
  const queryString = segmentEffortSearchParams({
    selectedEffortIds,
  }).toString();

  return `/segments/${segmentId}${queryString ? `?${queryString}` : ""}`;
}

export function buildSegmentRaceViewerHref({
  segmentId,
  selectedEffortIds,
  playbackPace,
}: {
  segmentId: number | string;
  selectedEffortIds: number[];
  playbackPace?: PlaybackPace;
}) {
  const queryString = segmentEffortSearchParams({
    selectedEffortIds,
    playbackPace,
  }).toString();

  return `/segments/${segmentId}/race${queryString ? `?${queryString}` : ""}`;
}

export function useSegmentWithComparison(segmentId: number | string) {
  const segmentQuery = useSegment(segmentId);
  const comparisonQuery = useSegmentComparison(
    segmentQuery.data ? segmentId : null,
  );
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

  return { segmentQuery, comparisonQuery, segment };
}

export function useSegmentEffortSelection({
  segmentId,
  segment,
  initialSelectedEffortIds,
  reseedWhenSelectionEmpty,
}: {
  segmentId: number | string;
  segment: Segment | null;
  initialSelectedEffortIds: number[];
  reseedWhenSelectionEmpty: boolean;
}) {
  const allEfforts = segment?.efforts ?? EMPTY_EFFORTS;
  const [selectionState, setSelectionState] = useKeyedState(
    String(segment?.id ?? segmentId),
    { ids: initialSelectedEffortIds, edited: false },
  );
  const availableEffortIds = new Set(allEfforts.map((effort) => effort.id));
  function resolvedSelection(state: { ids: number[]; edited: boolean }) {
    const validIds = state.ids.filter((id) => availableEffortIds.has(id));
    if (validIds.length > 0 || (state.edited && !reseedWhenSelectionEmpty)) {
      return validIds;
    }

    return allEfforts
      .slice(0, Math.min(3, allEfforts.length))
      .map((effort) => effort.id);
  }

  const selectedEffortIds = resolvedSelection(selectionState);
  const selectedEfforts = useMemo(
    () => selectedEffortsForIds(allEfforts, selectedEffortIds),
    [allEfforts, selectedEffortIds],
  );
  const selectedRows = useMemo(
    (): SelectedEffortRow[] => selectedRowsForEfforts(selectedEfforts),
    [selectedEfforts],
  );

  function addEffort(effortId: number) {
    setSelectionState((current) => {
      const currentIds = resolvedSelection(current);
      const ids = currentIds.includes(effortId)
        ? currentIds
        : [...currentIds, effortId];
      return { ids, edited: true };
    });
  }

  function removeEffort(effortId: number) {
    setSelectionState((current) => ({
      ids: resolvedSelection(current).filter((id) => id !== effortId),
      edited: true,
    }));
  }

  function setSelectedEffortIds(value: SetStateAction<number[]>) {
    setSelectionState((current) => ({
      ids:
        typeof value === "function" ? value(resolvedSelection(current)) : value,
      edited: true,
    }));
  }

  return {
    allEfforts,
    selectedEffortIds,
    selectedEfforts,
    selectedRows,
    setSelectedEffortIds,
    addEffort,
    removeEffort,
  };
}

export function useSegmentPlayback({
  playbackLimitSeconds,
  playbackRate,
}: {
  playbackLimitSeconds: number;
  playbackRate: number;
}): SegmentPlaybackState {
  const playbackAnimationFrameRef = useRef<number | null>(null);
  const playbackLastTimestampRef = useRef<number | null>(null);
  const initialPlayback = useMemo(() => ({ seconds: 0, isPlaying: false }), []);
  const [playback, setPlayback] = useKeyedState(
    String(playbackLimitSeconds),
    initialPlayback,
  );
  const { seconds: playbackSeconds, isPlaying } = playback;
  const setPlaybackSeconds = useCallback(
    (value: SetStateAction<number>) =>
      setPlayback((current) => ({
        ...current,
        seconds: typeof value === "function" ? value(current.seconds) : value,
      })),
    [setPlayback],
  );
  const setIsPlaying = useCallback(
    (value: SetStateAction<boolean>) =>
      setPlayback((current) => ({
        ...current,
        isPlaying:
          typeof value === "function" ? value(current.isPlaying) : value,
      })),
    [setPlayback],
  );

  useEffect(() => {
    if (!isPlaying || playbackLimitSeconds <= 0) {
      playbackLastTimestampRef.current = null;
      return undefined;
    }

    const tick = (timestamp: number) => {
      const previousTimestamp = playbackLastTimestampRef.current ?? timestamp;
      const deltaSeconds =
        playbackRate > 0
          ? ((timestamp - previousTimestamp) / 1000) * playbackRate
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
  }, [
    isPlaying,
    playbackLimitSeconds,
    playbackRate,
    setIsPlaying,
    setPlaybackSeconds,
  ]);

  return {
    limitSeconds: playbackLimitSeconds,
    seconds: playbackSeconds,
    isPlaying,
    setSeconds: setPlaybackSeconds,
    setIsPlaying,
  };
}

export function useSegmentRouteMetrics(
  segment: Segment | null,
): SegmentRouteMetrics {
  const { unitSystem } = useUnitPreferences();
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

  return {
    routeDistanceMeters,
    routeNetElevationMeters,
    routeGradePercent,
    unitSystem,
  };
}

export function useSegmentPerformanceSummary(
  segment: Segment | null,
): SegmentPerformanceSummary {
  const currentRider = useCurrentSegmentRider();
  const allEfforts = segment?.efforts ?? EMPTY_EFFORTS;
  const overallRankByEffortId = overallEffortRanks(allEfforts);
  const overallKom = fastestEffort(allEfforts);
  const currentUserPr = currentRiderFastestEffort(allEfforts, currentRider);
  const currentUserPrDurationSeconds =
    currentUserPr?.duration_seconds ??
    segment?.current_user_pr_duration_seconds ??
    null;
  const currentUserPrLabel = currentUserPr
    ? currentUserPr.activity_title
    : segment?.current_user_pr_duration_seconds != null
      ? "Personal best across matched efforts"
      : "No PR yet";

  return {
    currentUserPr,
    currentUserPrDurationSeconds,
    currentUserPrLabel,
    currentUserPrDisplayName:
      currentUserPr?.rider_name ?? currentRider.name ?? "You",
    overallKom,
    overallRankByEffortId,
  };
}

export function useSegmentTitleEditor({
  segment,
  updateTitle,
}: {
  segment: Segment | null;
  updateTitle: (input: { id: number; title: string }) => Promise<Segment>;
}): SegmentTitleEditor {
  const editorKey = `${segment?.id ?? "none"}:${segment?.title ?? ""}`;
  const [editor, setEditor] = useKeyedState(editorKey, {
    isEditingTitle: false,
    draftTitle: segment?.title ?? "",
  });
  const { isEditingTitle, draftTitle } = editor;
  const setDraftTitle = (value: string) =>
    setEditor((current) => ({ ...current, draftTitle: value }));
  const setIsEditingTitle = (value: boolean) =>
    setEditor((current) => ({ ...current, isEditingTitle: value }));

  return {
    isEditingTitle,
    draftTitle,
    startEditingTitle: () => {
      setDraftTitle(segment?.title ?? "");
      setIsEditingTitle(true);
    },
    cancelEditingTitle: () => {
      setDraftTitle(segment?.title ?? "");
      setIsEditingTitle(false);
    },
    setDraftTitle,
    saveTitle: () => {
      void (async () => {
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
          const updatedSegment = await updateTitle({
            id: segment.id,
            title,
          });

          setDraftTitle(updatedSegment.title);
          setIsEditingTitle(false);
          toast.success(`Saved ${updatedSegment.title}.`);
        } catch {
          // The mutation exposes the API error state used by the panel.
        }
      })();
    },
  };
}

export function useSegmentEffortsContainer({
  segmentId,
  segment,
  initialSelectedEffortIds,
}: {
  segmentId: number | string;
  segment: Segment | null;
  initialSelectedEffortIds: number[];
}): SegmentEffortsContainer {
  const effortSelection = useSegmentEffortSelection({
    segmentId,
    segment,
    initialSelectedEffortIds,
    reseedWhenSelectionEmpty: false,
  });
  const [effortTimeFilter, setEffortTimeFilter] =
    useState<EffortTimeFilter>("all");
  const {
    selectedEffortIds,
    selectedEfforts,
    selectedRows,
    addEffort,
    removeEffort,
  } = effortSelection;
  const [requestedHoveredEffortId, setHoveredEffortId] = useState<
    number | null
  >(null);
  const hoveredEffortId = selectedEfforts.some(
    (effort) => effort.id === requestedHoveredEffortId,
  )
    ? requestedHoveredEffortId
    : null;
  const visibleEfforts = filterEffortsByTimeWindow(
    segment?.efforts,
    effortTimeFilter,
  );
  const raceViewerHref = (playbackPace?: PlaybackPace) => {
    if (!segment?.id) {
      return null;
    }

    return buildSegmentRaceViewerHref({
      segmentId: segment.id,
      selectedEffortIds,
      playbackPace,
    });
  };

  return {
    effortList: {
      visibleEfforts,
      selectedEffortIds,
      selectedRows,
      effortTimeFilter,
      setEffortTimeFilter,
      addEffort,
      removeEffort,
    },
    selectedEffortIds,
    selectedEfforts,
    selectedRows,
    focusedEffortId: hoveredEffortId,
    raceViewerHref,
    actions: {
      hoverEffort: setHoveredEffortId,
      addEffort,
      removeEffort,
    },
  };
}
