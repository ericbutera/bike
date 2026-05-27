"use client";

import { auth } from "@ericbutera/kaleido";
import {
  faArrowLeft,
  faPause,
  faPlay,
} from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import Link from "next/link";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  extractApiMessage,
  formatDuration,
} from "../../lib/activityFormatting";
import {
  useSegment,
  type ActivityRoutePoint,
  type SegmentEffort,
} from "../../lib/queries";
import {
  EFFORT_COLORS,
  EMPTY_EFFORTS,
  EMPTY_EFFORT_IDS,
  PLAYBACK_END_EPSILON,
  PLAYBACK_PACE_OPTIONS,
  areEffortIdListsEqual,
  buildLeaderPairFollowViewport,
  buildLiveComparisonRows,
  comparisonMarkerPoint,
  fastestEffort,
  playbackTargetSeconds,
  type LiveComparisonRow,
  type PlaybackPace,
  type SelectedEffortRow,
} from "../../lib/segmentDetail";
import AuthRequiredCard from "../AuthRequiredCard";
import MapLibreRouteMap from "../MapLibreRouteMap";

type RaceMapMode = "overview" | "leader-follow";

const FOLLOW_LEADER_MAP_ZOOM = 19;

function sortComparisonRowsByLeader(comparisonRows: LiveComparisonRow[]) {
  const fallbackIndexByEffortId = new Map(
    comparisonRows.map((comparisonRow, index) => [
      comparisonRow.effort.id,
      index,
    ]),
  );

  return [...comparisonRows].sort((left, right) => {
    const progressDelta = (right.progress ?? -1) - (left.progress ?? -1);

    if (Math.abs(progressDelta) > Number.EPSILON) {
      return progressDelta;
    }

    const gapDelta = (right.gapSeconds ?? 0) - (left.gapSeconds ?? 0);

    if (Math.abs(gapDelta) > Number.EPSILON) {
      return gapDelta;
    }

    return (
      (fallbackIndexByEffortId.get(left.effort.id) ?? 0) -
      (fallbackIndexByEffortId.get(right.effort.id) ?? 0)
    );
  });
}

function buildInitialReferenceEffortId(
  selectedEfforts: SegmentEffort[],
  requestedReferenceEffortId: number | null,
  currentUserId: number | null,
  currentUserName: string | null,
) {
  if (
    requestedReferenceEffortId != null &&
    selectedEfforts.some((effort) => effort.id === requestedReferenceEffortId)
  ) {
    return requestedReferenceEffortId;
  }

  const currentUserReference = selectedEfforts.find((effort) => {
    if (currentUserId != null) {
      return effort.rider_user_id === currentUserId;
    }

    return currentUserName ? effort.rider_name === currentUserName : false;
  });

  return currentUserReference?.id ?? selectedEfforts[0]?.id ?? null;
}

function RaceViewerMap({
  routePoints,
  comparisonRows,
  playbackSeconds,
  mapMode,
}: {
  routePoints: ActivityRoutePoint[] | null | undefined;
  comparisonRows: LiveComparisonRow[];
  playbackSeconds: number;
  mapMode: RaceMapMode;
}) {
  const hasRouteMap = (routePoints?.length ?? 0) >= 2;

  const markers = comparisonRows
    .map((comparisonRow) => {
      const point = comparisonMarkerPoint(
        routePoints,
        comparisonRow.effort,
        playbackSeconds,
      );

      if (!point) {
        return null;
      }

      return {
        id: comparisonRow.effort.id,
        color: comparisonRow.color,
        point,
        progress: comparisonRow.progress,
        label: comparisonRow.markerLabel,
      };
    })
    .filter(
      (
        marker,
      ): marker is {
        id: number;
        color: string;
        point: ActivityRoutePoint;
        progress: number | null;
        label: string;
      } => marker !== null,
    );
  const sortedComparisonRows = sortComparisonRowsByLeader(comparisonRows);
  const markerById = new Map(markers.map((marker) => [marker.id, marker]));
  const rankedMarkers =
    mapMode === "leader-follow"
      ? sortedComparisonRows
          .map(
            (comparisonRow) => markerById.get(comparisonRow.effort.id) ?? null,
          )
          .filter(
            (
              marker,
            ): marker is {
              id: number;
              color: string;
              point: ActivityRoutePoint;
              progress: number | null;
              label: string;
            } => marker !== null,
          )
      : [];
  const followViewport =
    mapMode === "leader-follow"
      ? buildLeaderPairFollowViewport(
          rankedMarkers[0]?.point ?? null,
          rankedMarkers[1]?.point ?? null,
          FOLLOW_LEADER_MAP_ZOOM,
        )
      : null;

  return hasRouteMap ? (
    <MapLibreRouteMap
      key={mapMode}
      routePoints={routePoints}
      movingMarkers={markers.map((marker) => ({
        id: String(marker.id),
        point: marker.point,
        progress: marker.progress,
        color: marker.color,
        opacity: 1,
        label: marker.label,
      }))}
      followViewport={followViewport}
      followViewportBehavior="jump"
      ariaLabel="Segment race viewer map"
      emptyMessage="Segment route geometry is not available yet."
      className="absolute inset-0 border-0"
      layerPickerClassName="absolute left-[5.5rem] top-4 z-20 flex flex-wrap gap-2 rounded-box border border-base-100/15 bg-base-950/78 p-1 shadow-lg backdrop-blur sm:left-[6rem] sm:top-6"
      fitBoundsPadding={40}
      fitBoundsMaxZoom={18}
      showZoomControls
      showLayerPicker
    />
  ) : (
    <div className="flex h-full items-center justify-center p-6">
      <div className="alert bg-base-100 text-sm text-base-content/70">
        <span>Segment route geometry is not available yet.</span>
      </div>
    </div>
  );
}

export default function SegmentRaceViewer({
  segmentId,
  initialSelectedEffortIds,
  initialReferenceEffortId,
  initialPlaybackPace = "auto",
}: {
  segmentId: number | string;
  initialSelectedEffortIds: number[];
  initialReferenceEffortId: number | null;
  initialPlaybackPace?: PlaybackPace;
}) {
  const authApi = auth.useAuthApi();
  const { user, isLoading: isLoadingUser } = authApi.useCurrentUser();
  const segmentQuery = useSegment(user ? segmentId : null);
  const playbackAnimationFrameRef = useRef<number | null>(null);
  const playbackLastTimestampRef = useRef<number | null>(null);
  const requestedSelectionBySegmentIdRef = useRef(
    new Map<number, number[]>([[Number(segmentId), initialSelectedEffortIds]]),
  );
  const requestedReferenceBySegmentIdRef = useRef(
    new Map<number, number | null>([
      [Number(segmentId), initialReferenceEffortId],
    ]),
  );
  const initializedSelectionSegmentIdRef = useRef<number | null>(null);
  const [selectedEffortIds, setSelectedEffortIds] = useState<number[]>(
    initialSelectedEffortIds,
  );
  const [referenceEffortId, setReferenceEffortId] = useState<number | null>(
    initialReferenceEffortId,
  );
  const [playbackSeconds, setPlaybackSeconds] = useState(0);
  const [isPlaying, setIsPlaying] = useState(false);
  const [playbackPace, setPlaybackPace] =
    useState<PlaybackPace>(initialPlaybackPace);
  const mapMode: RaceMapMode = "leader-follow";
  const segment = segmentQuery.data;
  const allEfforts = segment?.efforts ?? EMPTY_EFFORTS;
  const currentUserId =
    typeof user?.id === "number"
      ? user.id
      : user?.id != null && Number.isFinite(Number(user.id))
        ? Number(user.id)
        : null;
  const currentUserName = user?.name?.trim() || null;
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
  const referenceEffort =
    selectedEfforts.find((effort) => effort.id === referenceEffortId) ?? null;
  const playbackLimitSeconds = referenceEffort?.duration_seconds ?? 0;
  const targetPlaybackDurationSeconds = playbackTargetSeconds(
    playbackLimitSeconds,
    playbackPace,
  );
  const comparisonRows = useMemo(
    () =>
      buildLiveComparisonRows(selectedRows, referenceEffort, playbackSeconds),
    [playbackSeconds, referenceEffort, selectedRows],
  );
  const sortedComparisonRows = useMemo(
    () => sortComparisonRowsByLeader(comparisonRows),
    [comparisonRows],
  );
  const backHref = `/segments/${segmentId}`;

  useEffect(() => {
    if (!segment || allEfforts.length === 0) {
      initializedSelectionSegmentIdRef.current = null;
      setSelectedEffortIds((current) =>
        current.length === 0 ? current : EMPTY_EFFORT_IDS,
      );
      setReferenceEffortId((current) => (current == null ? current : null));
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

      const seeded = allEfforts
        .slice(0, Math.min(3, allEfforts.length))
        .map((effort) => effort.id);

      return areEffortIdListsEqual(current, seeded) ? current : seeded;
    });
  }, [allEfforts, segment?.id, segment]);

  useEffect(() => {
    if (selectedEfforts.length === 0) {
      setReferenceEffortId(null);
      return;
    }

    setReferenceEffortId((current) => {
      if (
        current != null &&
        selectedEfforts.some((effort) => effort.id === current)
      ) {
        return current;
      }

      return buildInitialReferenceEffortId(
        selectedEfforts,
        segment?.id != null
          ? (requestedReferenceBySegmentIdRef.current.get(segment.id) ?? null)
          : initialReferenceEffortId,
        currentUserId,
        currentUserName,
      );
    });
  }, [
    currentUserId,
    currentUserName,
    initialReferenceEffortId,
    segment?.id,
    selectedEfforts,
  ]);

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
      <section className="flex min-h-screen items-center justify-center bg-base-200 px-6 py-10">
        <span className="loading loading-spinner loading-lg" />
      </section>
    );
  }

  if (!user) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-base-200 px-6 py-10">
        <div className="w-full max-w-2xl">
          <AuthRequiredCard
            eyebrow="Race viewer"
            title="Sign in to open the race viewer"
            description="Open the fullscreen race viewer to watch selected efforts on the map without the chart and leaderboard."
          />
        </div>
      </div>
    );
  }

  if (segmentQuery.isError || !segment) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-base-200 px-6 py-10">
        <div className="alert max-w-2xl bg-base-100 text-sm text-base-content/80 shadow-lg">
          <span>
            {extractApiMessage(segmentQuery.error) ||
              "Segment playback could not be loaded."}
          </span>
          <Link href={backHref} className="btn btn-sm btn-outline">
            Back to segment
          </Link>
        </div>
      </div>
    );
  }

  return (
    <section className="flex h-screen flex-col overflow-hidden bg-base-950 text-base-100">
      <div className="relative min-h-0 flex-1 overflow-hidden">
        <RaceViewerMap
          routePoints={segment.route_points}
          comparisonRows={comparisonRows}
          playbackSeconds={playbackSeconds}
          mapMode={mapMode}
        />

        <div className="pointer-events-none absolute inset-0">
          <div className="pointer-events-auto flex flex-wrap items-start justify-between gap-3 p-4 sm:p-6">
            <div className="flex min-w-0 flex-wrap items-center gap-3">
              <Link
                href={backHref}
                className="btn btn-sm border-0 bg-base-100/90 text-base-content shadow-sm backdrop-blur hover:bg-base-100"
              >
                <FontAwesomeIcon icon={faArrowLeft} className="h-3.5 w-3.5" />
                Back
              </Link>
            </div>

            <div className="mr-14 max-w-[12rem] rounded-box border border-base-100/15 bg-base-950/70 px-3 py-2 text-right shadow-lg backdrop-blur sm:mr-16 sm:max-w-[18rem]">
              <div className="text-[0.68rem] font-semibold uppercase tracking-[0.18em] text-base-100/55">
                Race viewer
              </div>
              <div className="truncate text-lg font-semibold text-base-100">
                {segment.title}
              </div>
            </div>
          </div>

          <div className="pointer-events-auto absolute inset-x-0 bottom-0 p-4 sm:p-6">
            <div className="mx-auto flex w-full max-w-7xl flex-col gap-3">
              {sortedComparisonRows.length > 0 ? (
                <div className="overflow-x-auto">
                  <div className="flex min-w-max gap-2 pb-1">
                    {sortedComparisonRows.map((comparisonRow) => (
                      <button
                        key={comparisonRow.effort.id}
                        type="button"
                        className={`rounded-box border px-3 py-2 text-left shadow-lg backdrop-blur transition ${comparisonRow.effort.id === referenceEffortId ? "border-base-100/20 bg-base-100 text-base-content" : "border-base-100/15 bg-base-950/75 text-base-100 hover:bg-base-950/85"}`}
                        onClick={() => {
                          setReferenceEffortId(comparisonRow.effort.id);
                          setPlaybackSeconds(0);
                          setIsPlaying(false);
                        }}
                      >
                        <div className="flex items-center gap-2">
                          <span
                            className="inline-flex h-6 w-6 items-center justify-center text-xs font-semibold text-white"
                            style={{ backgroundColor: comparisonRow.color }}
                          >
                            {comparisonRow.markerLabel}
                          </span>
                          <span className="font-semibold">
                            {comparisonRow.effort.rider_name}
                          </span>
                          {comparisonRow.effort.id === referenceEffortId ? (
                            <span className="badge badge-sm border-0 bg-base-content text-base-100">
                              Ref
                            </span>
                          ) : null}
                        </div>
                        <div className="mt-1 text-xs text-current/70">
                          {formatDuration(
                            comparisonRow.effort.duration_seconds,
                          )}
                        </div>
                      </button>
                    ))}
                  </div>
                </div>
              ) : null}

              <div className="rounded-box border border-base-100/15 bg-base-950/78 px-3 py-3 shadow-2xl backdrop-blur sm:px-4">
                <div className="flex flex-wrap items-center gap-3">
                  <div className="flex min-w-0 flex-1 items-center gap-3 max-[420px]:gap-2">
                    <button
                      type="button"
                      className="btn btn-sm btn-circle shrink-0 border-0 bg-orange-500 text-white hover:bg-orange-600"
                      disabled={
                        selectedRows.length === 0 || playbackLimitSeconds <= 0
                      }
                      aria-label={
                        isPlaying
                          ? "Pause race playback"
                          : playbackSeconds >= playbackLimitSeconds
                            ? "Replay race playback"
                            : "Play race playback"
                      }
                      onClick={() => {
                        if (playbackSeconds >= playbackLimitSeconds) {
                          setPlaybackSeconds(0);
                        }
                        setIsPlaying(!isPlaying);
                      }}
                    >
                      <FontAwesomeIcon
                        icon={isPlaying ? faPause : faPlay}
                        className="h-4 w-4"
                      />
                    </button>

                    <input
                      type="range"
                      min={0}
                      max={Math.max(playbackLimitSeconds, 1)}
                      step={0.1}
                      value={Math.min(
                        playbackSeconds,
                        Math.max(playbackLimitSeconds, 1),
                      )}
                      className="range range-primary min-w-0 flex-1"
                      disabled={
                        selectedRows.length === 0 || playbackLimitSeconds <= 0
                      }
                      aria-label="Race playback timeline"
                      onChange={(event) => {
                        setPlaybackSeconds(Number(event.target.value));
                        setIsPlaying(false);
                      }}
                    />

                    <span className="shrink-0 rounded-full border border-base-100/15 px-2.5 py-1 text-xs font-semibold tabular-nums text-base-100/85 max-[420px]:hidden">
                      {formatDuration(Math.round(playbackSeconds))} /{" "}
                      {formatDuration(playbackLimitSeconds)}
                    </span>
                  </div>

                  <div className="join shrink-0 max-[420px]:hidden">
                    {PLAYBACK_PACE_OPTIONS.map((option) => (
                      <button
                        key={option.key}
                        type="button"
                        className={`join-item btn btn-sm border-0 ${playbackPace === option.key ? "bg-base-100 text-base-content hover:bg-base-100" : "bg-transparent text-base-100/80 hover:bg-base-100/10"}`}
                        aria-pressed={playbackPace === option.key}
                        onClick={() => {
                          setPlaybackPace(option.key);
                        }}
                      >
                        {option.label}
                      </button>
                    ))}
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
