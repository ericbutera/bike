"use client";

import {
  faArrowLeft,
  faChevronDown,
  faGaugeHigh,
  faPause,
  faPlay,
  faXmark,
} from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import Link from "next/link";
import { useMemo, useState } from "react";
import { formatDuration } from "../../lib/activityFormatting";
import { config } from "../../lib/config";
import { type ActivityRoutePoint } from "../../lib/queries";
import { LoadingSpinner } from "../ui/QueryState";
import {
  RACE_PLAYBACK_SPEED_OPTIONS,
  buildLeaderGroupFollowViewport,
  buildLeaderPairFollowViewport,
  buildLiveLeaderComparisonRows,
  comparisonMarkerPoint,
  formatSignedSecondsDelta,
  sortLiveComparisonRowsByLeader,
  type LiveComparisonRow,
  type RacePlaybackSpeed,
} from "../../lib/segmentDetail";
import MapLibreRouteMap from "../MapLibreRouteMap";
import { type RouteMapBasemap } from "../RouteMapTypes";
import {
  buildSegmentDetailHref,
  useSegmentEffortSelection,
  useSegmentPlayback,
  useSegmentWithComparison,
} from "./useSegmentDetailState";

type RaceMapMode = "overview" | "leader-follow";

const FOLLOW_LEADER_MAP_ZOOM = 19;
const LONG_SEGMENT_AUTO_ZOOM_SECONDS = 180;

function resolveRaceViewerBasemap(styleUrl: string): RouteMapBasemap {
  switch (styleUrl.trim().toLowerCase()) {
    case "":
    case "topo":
    case "opentopomap":
    case "outdoors":
      return "topo";
    case "street":
    case "streets":
    case "carto-voyager":
      return "street";
    case "satellite":
    case "imagery":
      return "satellite";
    default:
      return "topo";
  }
}

function formatRideDateLabel(value: string) {
  const date = new Date(value);

  return Number.isNaN(date.getTime())
    ? value
    : date.toLocaleDateString(undefined, { dateStyle: "medium" });
}

function formatAttemptNumber(value: number) {
  return `Attempt ${value}`;
}

function racePlaybackSpeedLabel(speed: RacePlaybackSpeed) {
  return (
    RACE_PLAYBACK_SPEED_OPTIONS.find((option) => option.value === speed)
      ?.label ?? `${speed}x`
  );
}

function RacePlaybackSpeedControl({
  value,
  onChange,
}: {
  value: RacePlaybackSpeed;
  onChange: (value: RacePlaybackSpeed) => void;
}) {
  const selectedIndex = Math.max(
    RACE_PLAYBACK_SPEED_OPTIONS.findIndex((option) => option.value === value),
    0,
  );

  return (
    <div className="dropdown dropdown-top dropdown-end shrink-0 sm:order-4">
      <button
        type="button"
        tabIndex={0}
        className="btn btn-sm btn-outline min-w-[6.75rem]"
        aria-label="Race playback speed"
      >
        <FontAwesomeIcon icon={faGaugeHigh} className="h-3.5 w-3.5" />
        {racePlaybackSpeedLabel(value)}
        <FontAwesomeIcon icon={faChevronDown} className="h-3 w-3" />
      </button>
      <div
        tabIndex={0}
        className="dropdown-content z-30 mb-2 w-72 rounded-box border border-base-300 bg-base-100 p-4 shadow-xl"
      >
        <div className="mb-3 flex items-center justify-between gap-3">
          <span className="text-xs font-semibold uppercase tracking-[0.16em] text-base-content/55">
            Speed
          </span>
          <span className="rounded-full border border-base-300 bg-base-200 px-2.5 py-1 text-xs font-semibold tabular-nums">
            {racePlaybackSpeedLabel(value)}
          </span>
        </div>
        <input
          type="range"
          min={0}
          max={RACE_PLAYBACK_SPEED_OPTIONS.length - 1}
          step={1}
          value={selectedIndex}
          className="range range-primary range-xs"
          aria-label="Race playback speed slider"
          onChange={(event) => {
            const nextOption =
              RACE_PLAYBACK_SPEED_OPTIONS[Number(event.target.value)];

            if (nextOption) {
              onChange(nextOption.value);
            }
          }}
        />
        <div className="mt-3 grid grid-cols-5 gap-1 text-center text-[0.65rem] font-medium tabular-nums text-base-content/65">
          {RACE_PLAYBACK_SPEED_OPTIONS.map((option) => (
            <button
              key={option.value}
              type="button"
              className={`rounded px-1 py-1 ${option.value === value ? "bg-primary text-primary-content" : "hover:bg-base-200"}`}
              onClick={() => {
                onChange(option.value);
              }}
            >
              {option.label}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

function RaceViewerMap({
  routePoints,
  comparisonRows,
  playbackSeconds,
  mapMode,
  selectedBasemap,
  onSelectedBasemapChange,
  playbackLimitSeconds,
}: {
  routePoints: ActivityRoutePoint[] | null | undefined;
  comparisonRows: LiveComparisonRow[];
  playbackSeconds: number;
  mapMode: RaceMapMode;
  selectedBasemap: RouteMapBasemap;
  onSelectedBasemapChange: (basemap: RouteMapBasemap) => void;
  playbackLimitSeconds: number;
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
  const sortedComparisonRows = sortLiveComparisonRowsByLeader(comparisonRows);
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
  const pairFollowViewport =
    mapMode === "leader-follow"
      ? buildLeaderPairFollowViewport(
          rankedMarkers[0]?.point ?? null,
          rankedMarkers[1]?.point ?? null,
          FOLLOW_LEADER_MAP_ZOOM,
        )
      : null;
  const topThreeFollowViewport =
    mapMode === "leader-follow"
      ? buildLeaderGroupFollowViewport(
          rankedMarkers.slice(0, 3).map((marker) => marker.point),
          FOLLOW_LEADER_MAP_ZOOM,
        )
      : null;
  const followViewport =
    playbackLimitSeconds >= LONG_SEGMENT_AUTO_ZOOM_SECONDS
      ? (topThreeFollowViewport ?? pairFollowViewport)
      : pairFollowViewport;

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
      followViewportBehavior="ease"
      followViewportPreserveUserZoom
      ariaLabel="Segment race viewer map"
      emptyMessage="Segment route geometry is not available yet."
      className="absolute inset-0 border-0"
      basemapOptions={["topo", "street", "satellite"]}
      defaultBasemap="topo"
      selectedBasemap={selectedBasemap}
      onSelectedBasemapChange={onSelectedBasemapChange}
      fitBoundsPadding={40}
      fitBoundsMaxZoom={18}
      showZoomControls
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
  initialPlaybackSpeed = 1,
}: {
  segmentId: number | string;
  initialSelectedEffortIds: number[];
  initialPlaybackSpeed?: RacePlaybackSpeed;
}) {
  const { segmentQuery, comparisonQuery, segment } =
    useSegmentWithComparison(segmentId);
  const [playbackSpeed, setPlaybackSpeed] =
    useState<RacePlaybackSpeed>(initialPlaybackSpeed);
  const [selectedBasemap, setSelectedBasemap] = useState<RouteMapBasemap>(() =>
    resolveRaceViewerBasemap(config.MAP_STYLE_URL),
  );
  const mapMode: RaceMapMode = "leader-follow";
  const effortSelection = useSegmentEffortSelection({
    segmentId,
    segment,
    initialSelectedEffortIds,
    reseedWhenSelectionEmpty: true,
  });
  const { selectedEffortIds, selectedRows, removeEffort } = effortSelection;
  const playbackLimitSeconds = Math.max(
    0,
    ...selectedRows.map((row) => row.effort.duration_seconds),
  );
  const playback = useSegmentPlayback({
    playbackLimitSeconds,
    playbackRate: playbackSpeed > 0 ? playbackSpeed : 0,
  });
  const comparisonRows = useMemo(
    () => buildLiveLeaderComparisonRows(selectedRows, playback.seconds),
    [playback.seconds, selectedRows],
  );
  const sortedComparisonRows = useMemo(
    () => sortLiveComparisonRowsByLeader(comparisonRows),
    [comparisonRows],
  );
  const leaderGapByEffortId = useMemo(
    () =>
      new Map(
        comparisonRows.map((comparisonRow) => [
          comparisonRow.effort.id,
          comparisonRow.gapSeconds,
        ]),
      ),
    [comparisonRows],
  );
  const livePositionByEffortId = useMemo(
    () =>
      new Map(
        sortedComparisonRows.map((comparisonRow, index) => [
          comparisonRow.effort.id,
          index + 1,
        ]),
      ),
    [sortedComparisonRows],
  );
  const backHref = useMemo(() => {
    return buildSegmentDetailHref({
      segmentId,
      selectedEffortIds,
    });
  }, [segmentId, selectedEffortIds]);

  if (
    segmentQuery.isLoading ||
    (segmentQuery.data && comparisonQuery.isLoading)
  ) {
    return (
      <section className="flex min-h-screen items-center justify-center bg-base-200 px-6 py-10">
        <LoadingSpinner size="lg" />
      </section>
    );
  }

  if (!segment) {
    return null;
  }

  return (
    <section className="flex h-screen flex-col overflow-hidden bg-base-950 text-base-100">
      <div className="relative min-h-0 flex-1 overflow-hidden">
        <RaceViewerMap
          routePoints={segment.route_points}
          comparisonRows={comparisonRows}
          playbackSeconds={playback.seconds}
          mapMode={mapMode}
          selectedBasemap={selectedBasemap}
          onSelectedBasemapChange={setSelectedBasemap}
          playbackLimitSeconds={playback.limitSeconds}
        />

        <div className="pointer-events-none absolute inset-0">
          <div className="pointer-events-auto flex flex-wrap items-start justify-between gap-3 p-4 sm:p-6">
            <div className="flex min-w-0 items-center gap-3 overflow-x-auto pb-1">
              <Link
                href={backHref}
                className="btn btn-sm bg-base-100 text-base-content border border-base-300 shadow-sm hover:bg-base-200"
              >
                <FontAwesomeIcon icon={faArrowLeft} className="h-3.5 w-3.5" />
                Back
              </Link>

              <div className="rounded-box border border-base-300/80 bg-base-100/90 p-1 shadow-sm backdrop-blur">
                <div className="join join-horizontal shrink-0">
                  {(
                    [
                      ["topo", "Topo"],
                      ["street", "Street"],
                      ["satellite", "Satellite"],
                    ] as const
                  ).map(([basemap, label]) => (
                    <button
                      key={basemap}
                      type="button"
                      className={`join-item btn btn-sm ${selectedBasemap === basemap ? "btn-primary" : "bg-base-100 text-base-content border border-base-300 shadow-sm hover:bg-base-200"}`}
                      aria-pressed={selectedBasemap === basemap}
                      onClick={() => {
                        setSelectedBasemap(basemap);
                      }}
                    >
                      {label}
                    </button>
                  ))}
                </div>
              </div>
            </div>

            <div className="card mr-14 w-full max-w-xs bg-base-100 text-base-content shadow-xl sm:mr-16 sm:w-auto sm:max-w-sm">
              <div className="card-body gap-1 px-4 py-3 text-right sm:px-5">
                <div className="text-[0.68rem] font-semibold uppercase tracking-[0.18em] text-base-content/55">
                  Race viewer
                </div>
                <div className="text-lg font-semibold leading-tight">
                  {segment.title}
                </div>
              </div>
            </div>
          </div>

          <div className="pointer-events-auto absolute inset-x-0 bottom-0 p-4 sm:p-6">
            <div className="mx-auto flex w-full max-w-7xl flex-col gap-3">
              {comparisonRows.length > 0 ? (
                <div className="overflow-x-auto pt-28">
                  <div className="flex min-w-max items-end gap-3 pb-1">
                    {comparisonRows.map((comparisonRow) => {
                      const leaderGapSeconds =
                        leaderGapByEffortId.get(comparisonRow.effort.id) ??
                        null;
                      const livePosition =
                        livePositionByEffortId.get(comparisonRow.effort.id) ??
                        null;
                      const gapValue =
                        livePosition === 1
                          ? null
                          : formatSignedSecondsDelta(leaderGapSeconds);
                      const isTrailing = (leaderGapSeconds ?? 0) > 0;
                      const rideDate = formatRideDateLabel(
                        comparisonRow.effort.activity_started_at,
                      );
                      const attemptNumber = formatAttemptNumber(
                        comparisonRow.effort.effort_index,
                      );
                      const effortDetailsLabel = `${comparisonRow.effort.rider_name} - ${rideDate} - ${attemptNumber}`;

                      return (
                        <div
                          key={comparisonRow.effort.id}
                          className="group relative w-44 rounded-box bg-base-100 p-3 text-base-content shadow-lg"
                          tabIndex={0}
                          title={effortDetailsLabel}
                          aria-label={effortDetailsLabel}
                        >
                          <div className="flex items-center gap-2">
                            <span
                              className="inline-flex h-6 w-6 shrink-0 items-center justify-center rounded-full text-xs font-semibold text-white"
                              style={{ backgroundColor: comparisonRow.color }}
                            >
                              {comparisonRow.markerLabel}
                            </span>
                            <span className="truncate font-semibold">
                              {comparisonRow.effort.rider_name}
                            </span>
                            <button
                              type="button"
                              className="btn btn-ghost btn-xs btn-circle ml-auto min-h-6 h-6 w-6 shrink-0 text-base-content/55 hover:text-error"
                              aria-label={`Remove ${effortDetailsLabel} from race viewer`}
                              onClick={() => {
                                removeEffort(comparisonRow.effort.id);
                              }}
                            >
                              <FontAwesomeIcon
                                icon={faXmark}
                                className="h-3 w-3"
                              />
                            </button>
                          </div>
                          <div className="mt-1 flex items-center justify-between gap-2 text-xs">
                            <span className="text-base-content/70">
                              {formatDuration(
                                comparisonRow.effort.duration_seconds,
                              )}
                            </span>
                            {gapValue ? (
                              <span
                                className={`font-semibold ${isTrailing ? "text-error" : "text-base-content/70"}`}
                              >
                                {gapValue}
                              </span>
                            ) : livePosition === 1 ? (
                              <span className="font-semibold text-success">
                                Lead
                              </span>
                            ) : null}
                          </div>
                          <div className="pointer-events-none absolute bottom-full left-0 z-20 mb-2 hidden w-44 rounded-box border border-base-300 bg-base-100 p-3 text-left text-xs shadow-xl group-hover:block group-focus:block">
                            <div className="truncate font-semibold text-base-content">
                              {comparisonRow.effort.activity_title}
                            </div>
                            <div className="mt-2 grid gap-1 text-base-content/70">
                              <div className="flex justify-between gap-3">
                                <span>Ride date</span>
                                <span className="text-right font-medium text-base-content">
                                  {rideDate}
                                </span>
                              </div>
                              <div className="flex justify-between gap-3">
                                <span>Attempt</span>
                                <span className="text-right font-medium text-base-content">
                                  {attemptNumber}
                                </span>
                              </div>
                            </div>
                          </div>
                        </div>
                      );
                    })}
                  </div>
                </div>
              ) : null}

              <div className="rounded-box border border-base-300 bg-base-100 px-3 py-3 text-base-content shadow-2xl sm:px-4">
                <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
                  <div className="flex items-center gap-3 sm:contents">
                    <button
                      type="button"
                      className="btn btn-sm btn-circle btn-primary shrink-0"
                      disabled={
                        selectedRows.length === 0 ||
                        playback.limitSeconds <= 0
                      }
                      aria-label={
                        playback.isPlaying
                          ? "Pause race playback"
                          : playback.seconds >= playback.limitSeconds
                            ? "Replay race playback"
                            : "Play race playback"
                      }
                      onClick={() => {
                        if (playback.seconds >= playback.limitSeconds) {
                          playback.setSeconds(0);
                        }
                        playback.setIsPlaying(!playback.isPlaying);
                      }}
                    >
                      <FontAwesomeIcon
                        icon={playback.isPlaying ? faPause : faPlay}
                        className="h-4 w-4"
                      />
                    </button>

                    <span className="ml-auto shrink-0 rounded-full border border-base-300 bg-base-200 px-2.5 py-1 text-xs font-semibold tabular-nums text-base-content/80 sm:order-3 sm:ml-0">
                      {formatDuration(Math.round(playback.seconds))} /{" "}
                      {formatDuration(playback.limitSeconds)}
                    </span>

                    <RacePlaybackSpeedControl
                      value={playbackSpeed}
                      onChange={setPlaybackSpeed}
                    />
                  </div>

                  <input
                    type="range"
                    min={0}
                    max={Math.max(playback.limitSeconds, 1)}
                    step={0.1}
                    value={Math.min(
                      playback.seconds,
                      Math.max(playback.limitSeconds, 1),
                    )}
                    className="range range-primary w-full sm:order-2 sm:min-w-0 sm:flex-1"
                    disabled={
                      selectedRows.length === 0 || playback.limitSeconds <= 0
                    }
                    aria-label="Race playback timeline"
                    onChange={(event) => {
                      playback.setSeconds(Number(event.target.value));
                      playback.setIsPlaying(false);
                    }}
                  />
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
