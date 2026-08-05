"use client";

import { faPause, faPlay, faXmark } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import Link from "next/link";
import { useLayoutEffect, useMemo, useRef, useState } from "react";
import {
  Area,
  CartesianGrid,
  ComposedChart,
  Line,
  ReferenceDot,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import {
  formatDistance,
  formatDuration,
  formatElevation,
  formatHeartRate,
  formatSpeed,
  type UnitSystem,
} from "../../lib/activityFormatting";
import type { ActivityRoutePoint } from "../../lib/queries";
import {
  ATHLETE_PANEL_ROW_ANIMATION_MS,
  buildEffortOverlayChartRows,
  buildPlaybackEffortOverlayMarker,
  comparisonMarkerPoint,
  effortSeriesDataKey,
  formatSignedSecondsDelta,
  formatSignedSpeedDelta,
  type GapChartRow,
  interpolateRoutePointByProgress,
  type LiveComparisonRow,
  PLAYBACK_PACE_OPTIONS,
  type SelectedEffortRow,
  sortLiveComparisonRowsByLeader,
} from "../../lib/segmentDetail";
import MapLibreRouteMap from "../MapLibreRouteMap";
import { AppCard, CardHeader } from "../ui/Card";
import InfoTooltip from "../ui/InfoTooltip";
import { LoadingSpinner } from "../ui/QueryState";
import type {
  SegmentComparisonPlayback,
  SegmentComparisonWorkspace,
} from "./useSegmentDetailState";

const COMPARISON_WORKSPACE_HELP_TEXT =
  "Playback follows elapsed time so positions, leader gaps, speed, and heart rate update on every frame.";

function formatTooltipSeconds(
  wholeSeconds: number,
  fractionalMilliseconds: number,
  padToTwoDigits: boolean,
) {
  const secondsText = padToTwoDigits
    ? String(wholeSeconds).padStart(2, "0")
    : String(wholeSeconds);

  if (fractionalMilliseconds <= 0) {
    return `${secondsText}s`;
  }

  return `${secondsText}.${String(fractionalMilliseconds).padStart(3, "0")}s`;
}

function formatTooltipDuration(value?: number | null) {
  if (value == null || value <= 0) {
    return "--";
  }

  const truncatedMilliseconds = Math.trunc(value * 1000 + Number.EPSILON);
  const hours = Math.floor(truncatedMilliseconds / 3_600_000);
  const minutes = Math.floor((truncatedMilliseconds % 3_600_000) / 60_000);

  if (hours > 0) {
    return `${hours}h ${String(minutes).padStart(2, "0")}m`;
  }

  const secondMilliseconds = truncatedMilliseconds % 60_000;
  const wholeSeconds = Math.floor(secondMilliseconds / 1000);
  const fractionalMilliseconds = secondMilliseconds % 1000;

  if (minutes > 0) {
    return `${minutes}m ${formatTooltipSeconds(
      wholeSeconds,
      fractionalMilliseconds,
      true,
    )}`;
  }

  return formatTooltipSeconds(wholeSeconds, fractionalMilliseconds, false);
}

type ComparisonGapChartTooltipPayloadEntry = {
  color?: string;
  dataKey?: string;
  payload?: GapChartRow;
  value?: number | string | null;
};

function numericChartValue(value: unknown) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function comparisonTooltipContext({
  label,
  payload,
}: {
  label: number;
  payload?: ComparisonGapChartTooltipPayloadEntry[];
}) {
  const tooltipRow = payload?.find((entry) => entry.payload)?.payload;
  const elevationEntry = payload?.find(
    (entry) => entry.dataKey === "elevation",
  );

  return {
    progress: numericChartValue(tooltipRow?.progress),
    distanceMeters: numericChartValue(tooltipRow?.distanceMeters) ?? label,
    elevationMeters:
      numericChartValue(tooltipRow?.elevation) ??
      numericChartValue(elevationEntry?.value),
  };
}

export function ComparisonGapChartTooltip({
  active,
  label,
  payload,
  selectedRows,
  unitSystem,
}: {
  active?: boolean;
  label?: number;
  payload?: ComparisonGapChartTooltipPayloadEntry[];
  selectedRows: SelectedEffortRow[];
  unitSystem: UnitSystem;
}) {
  if (!active || typeof label !== "number") {
    return null;
  }

  const { progress, distanceMeters, elevationMeters } =
    comparisonTooltipContext({ label, payload });

  return (
    <div className="max-w-[26rem] border border-base-300 bg-base-100 px-3 py-3 shadow-lg">
      <div className="flex flex-wrap gap-2 text-[0.68rem] font-semibold uppercase tracking-[0.16em] text-base-content/60">
        <span className="border border-base-300 bg-base-200/70 px-2 py-1">
          Elev {formatElevation(elevationMeters, unitSystem)}
        </span>
        <span className="border border-base-300 bg-base-200/70 px-2 py-1">
          Dist {formatDistance(distanceMeters, unitSystem)}
        </span>
      </div>

      <div className="mt-2 space-y-1 text-[0.78rem] text-base-content/80 tabular-nums">
        {selectedRows.map((selectedRow) => {
          const comparisonPoint =
            progress != null
              ? interpolateRoutePointByProgress(
                  selectedRow.effort.route_points,
                  progress,
                )
              : null;

          return (
            <div
              key={selectedRow.effort.id}
              aria-label={`Ride ${selectedRow.markerLabel} tooltip row`}
              className="flex items-center gap-2 overflow-hidden whitespace-nowrap border border-base-300 bg-base-200/70 px-2 py-1.5"
            >
              <span
                className="inline-flex min-w-[2.25rem] items-center justify-center px-1.5 py-0.5 text-[0.65rem] font-semibold text-white"
                style={{ backgroundColor: selectedRow.color }}
              >
                #{selectedRow.markerLabel}
              </span>
              <span>
                {formatTooltipDuration(
                  comparisonPoint?.elapsed_seconds ?? null,
                )}
              </span>
              <span aria-hidden className="text-base-content/35">
                /
              </span>
              <span>
                {formatSpeed(comparisonPoint?.speed_mps ?? null, unitSystem)}
              </span>
              <span aria-hidden className="text-base-content/35">
                /
              </span>
              <span>
                {formatHeartRate(comparisonPoint?.heart_rate_bpm ?? null)}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function RouteComparisonMap({
  routePoints,
  selectedRows,
  playbackSeconds,
}: {
  routePoints: ActivityRoutePoint[] | null | undefined;
  selectedRows: SelectedEffortRow[];
  playbackSeconds: number;
}) {
  const hasRouteMap = (routePoints?.length ?? 0) >= 2;

  const markers = selectedRows
    .map((selectedRow) => {
      const point = comparisonMarkerPoint(
        routePoints,
        selectedRow.effort,
        playbackSeconds,
      );

      if (!point) {
        return null;
      }

      return {
        id: selectedRow.effort.id,
        color: selectedRow.color,
        point,
        label: selectedRow.markerLabel,
      };
    })
    .filter(
      (
        marker,
      ): marker is {
        id: number;
        color: string;
        point: ActivityRoutePoint;
        label: string;
      } => marker !== null,
    );

  return hasRouteMap ? (
    <MapLibreRouteMap
      routePoints={routePoints}
      movingMarkers={markers.map((marker) => ({
        id: String(marker.id),
        point: marker.point,
        color: marker.color,
        opacity: 1,
        label: marker.label,
      }))}
      ariaLabel="Segment comparison map"
      emptyMessage="Segment route geometry is not available yet."
      fitBoundsPadding={40}
      fitBoundsMaxZoom={18}
      className="h-full min-h-[24rem] w-full rounded-none border-0"
    />
  ) : (
    <div className="flex h-full min-h-[24rem] items-center justify-center p-4">
      <div className="alert">Segment route geometry is not available yet.</div>
    </div>
  );
}

function ComparisonChart({
  routePoints,
  routeDistanceMeters,
  selectedRows,
  playbackSeconds,
  unitSystem,
}: {
  routePoints: ActivityRoutePoint[] | null | undefined;
  routeDistanceMeters: number | null | undefined;
  selectedRows: SelectedEffortRow[];
  playbackSeconds: number;
  unitSystem: UnitSystem;
}) {
  const [hoveredRow, setHoveredRow] = useState<GapChartRow | null>(null);
  const chartRows = useMemo(
    () =>
      buildEffortOverlayChartRows(
        routePoints,
        selectedRows,
        routeDistanceMeters,
      ),
    [routeDistanceMeters, routePoints, selectedRows],
  );
  const maxDistance =
    chartRows.at(-1)?.distanceMeters ?? routeDistanceMeters ?? 1;

  return chartRows.length >= 2 ? (
    <div
      role="img"
      aria-label="Segment comparison chart"
      className="h-[18rem] p-3"
    >
      <ResponsiveContainer
        width="100%"
        height="100%"
        minWidth={320}
        minHeight={240}
      >
        <ComposedChart
          data={chartRows}
          margin={{ top: 12, right: 12, bottom: 4, left: 8 }}
          onMouseLeave={() => {
            setHoveredRow(null);
          }}
          onMouseMove={(state) => {
            const nextIndex = Number(state?.activeTooltipIndex);

            if (
              state?.isTooltipActive &&
              Number.isInteger(nextIndex) &&
              nextIndex >= 0 &&
              nextIndex < chartRows.length
            ) {
              setHoveredRow(chartRows[nextIndex]);
            } else {
              setHoveredRow(null);
            }
          }}
        >
          <CartesianGrid
            vertical={false}
            stroke="var(--color-base-content)"
            strokeOpacity={0.1}
          />
          <XAxis
            axisLine={false}
            dataKey="distanceMeters"
            domain={[0, maxDistance]}
            tick={{ fill: "var(--color-base-content)", fontSize: 10 }}
            tickFormatter={(value: number) => formatDistance(value, unitSystem)}
            tickLine={false}
            type="number"
          />
          <YAxis
            axisLine={false}
            tick={{ fill: "var(--color-base-content)", fontSize: 10 }}
            tickFormatter={(value: number) =>
              formatElevation(value, unitSystem)
            }
            tickLine={false}
            tickMargin={10}
            width={68}
            yAxisId="elevation"
          />
          <YAxis
            axisLine={false}
            orientation="right"
            tick={{ fill: "var(--color-base-content)", fontSize: 10 }}
            tickFormatter={(value: number) => formatDuration(Math.round(value))}
            tickLine={false}
            tickMargin={10}
            width={72}
            yAxisId="elapsed"
          />
          <Tooltip
            content={
              <ComparisonGapChartTooltip
                selectedRows={selectedRows}
                unitSystem={unitSystem}
              />
            }
            cursor={{
              stroke: "#71717a",
              strokeDasharray: "4 4",
              strokeOpacity: 0.95,
            }}
          />

          <Area
            type="linear"
            dataKey="elevation"
            yAxisId="elevation"
            stroke="#9ca3af"
            fill="#d1d5db"
            fillOpacity={0.45}
            strokeOpacity={0.7}
            strokeWidth={1.5}
            dot={false}
            connectNulls
          />
          {selectedRows.map((selectedRow) => {
            return (
              <Line
                key={selectedRow.effort.id}
                type="linear"
                dataKey={effortSeriesDataKey(selectedRow.effort.id)}
                yAxisId="elapsed"
                stroke={selectedRow.color}
                strokeWidth={2.4}
                strokeOpacity={1}
                dot={false}
                activeDot={false}
                connectNulls
              />
            );
          })}

          {!hoveredRow
            ? selectedRows.map((selectedRow) => {
                const playbackMarker = buildPlaybackEffortOverlayMarker(
                  selectedRow,
                  routePoints,
                  routeDistanceMeters,
                  playbackSeconds,
                );

                if (!playbackMarker) {
                  return null;
                }

                return (
                  <ReferenceDot
                    key={`${selectedRow.effort.id}-marker`}
                    x={playbackMarker.distanceMeters}
                    y={playbackMarker.value}
                    fill={selectedRow.color}
                    fillOpacity={1}
                    r={playbackMarker.isFinished ? 4.75 : 5.5}
                    stroke="var(--color-base-100)"
                    strokeOpacity={1}
                    strokeWidth={1.2}
                    yAxisId="elapsed"
                  />
                );
              })
            : null}
        </ComposedChart>
      </ResponsiveContainer>
    </div>
  ) : (
    <div className="flex h-[18rem] items-center justify-center p-4">
      <div className="alert">
        The selected efforts do not have enough point-level data for a shared
        chart.
      </div>
    </div>
  );
}

function SelectedEffortsPanel({
  comparisonRows,
  focusedEffortId,
  unitSystem,
  onHoverEffort,
  onRemoveEffort,
}: {
  comparisonRows: LiveComparisonRow[];
  focusedEffortId: number | null;
  unitSystem: UnitSystem;
  onHoverEffort: (effortId: number | null) => void;
  onRemoveEffort: (effortId: number) => void;
}) {
  const rowRefs = useRef(new Map<number, HTMLLIElement>());
  const previousRowTopByEffortIdRef = useRef(new Map<number, number>());
  const animationFrameRef = useRef<number | null>(null);
  const athleteGridTemplateClassName =
    "[grid-template-columns:auto_minmax(0,1fr)_4.5rem_5rem_3.75rem_1rem] xl:[grid-template-columns:auto_minmax(0,1fr)_5.25rem_6.5rem_4.5rem_1.25rem]";
  const sortedComparisonRows = useMemo(() => {
    return sortLiveComparisonRowsByLeader(comparisonRows);
  }, [comparisonRows]);
  const sortedComparisonRowOrder = useMemo(
    () =>
      sortedComparisonRows
        .map((comparisonRow) => comparisonRow.effort.id)
        .join(","),
    [sortedComparisonRows],
  );

  function formatRideDateLabel(value: string) {
    const date = new Date(value);

    return Number.isNaN(date.getTime())
      ? value
      : date.toLocaleDateString(undefined, { dateStyle: "medium" });
  }

  useLayoutEffect(() => {
    if (animationFrameRef.current != null) {
      window.cancelAnimationFrame(animationFrameRef.current);
      animationFrameRef.current = null;
    }

    for (const comparisonRow of sortedComparisonRows) {
      const row = rowRefs.current.get(comparisonRow.effort.id);

      if (!row) {
        continue;
      }

      row.style.transition = "none";
      row.style.transform = "translateY(0)";
    }

    const nextRowTopByEffortId = new Map<number, number>();

    for (const comparisonRow of sortedComparisonRows) {
      const row = rowRefs.current.get(comparisonRow.effort.id);

      if (!row) {
        continue;
      }

      const nextTop = row.getBoundingClientRect().top;
      nextRowTopByEffortId.set(comparisonRow.effort.id, nextTop);

      const previousTop = previousRowTopByEffortIdRef.current.get(
        comparisonRow.effort.id,
      );

      if (previousTop == null) {
        continue;
      }

      const deltaY = previousTop - nextTop;

      if (Math.abs(deltaY) < 1) {
        continue;
      }

      row.style.transition = "none";
      row.style.transform = `translateY(${deltaY}px)`;
    }

    animationFrameRef.current = window.requestAnimationFrame(() => {
      for (const comparisonRow of sortedComparisonRows) {
        const row = rowRefs.current.get(comparisonRow.effort.id);

        if (!row || !row.style.transform) {
          continue;
        }

        row.style.transition = `transform ${ATHLETE_PANEL_ROW_ANIMATION_MS}ms cubic-bezier(0.22, 1, 0.36, 1)`;
        row.style.transform = "translateY(0)";
      }

      animationFrameRef.current = null;
    });

    previousRowTopByEffortIdRef.current = nextRowTopByEffortId;

    return () => {
      if (animationFrameRef.current != null) {
        window.cancelAnimationFrame(animationFrameRef.current);
        animationFrameRef.current = null;
      }
    };
  }, [sortedComparisonRowOrder]);

  return sortedComparisonRows.length > 0 ? (
    <div className="flex bg-base-100 xl:h-full xl:min-h-[24rem] xl:flex-col">
      <div className="w-full xl:flex-1 xl:overflow-hidden">
        <div
          className="overflow-x-hidden overflow-y-visible xl:h-full xl:overflow-y-auto"
          style={{ scrollbarGutter: "stable" }}
        >
          <div className="border-b border-base-300 bg-base-100 px-3 py-3 sm:px-4 xl:sticky xl:top-0 xl:z-10">
            <div
              className={`grid items-center gap-3 text-xs font-semibold uppercase tracking-[0.14em] text-base-content/55 xl:gap-4 ${athleteGridTemplateClassName}`}
            >
              <span className="col-span-2">Athletes</span>
              <span className="justify-self-end text-right">Gap</span>
              <span className="justify-self-end text-right">Speed</span>
              <span className="justify-self-end text-right">HR</span>
              <span aria-hidden="true" className="block" />
            </div>
          </div>

          <ul className="list bg-base-100 p-0">
            {sortedComparisonRows.map((comparisonRow) => {
              const isFocused = focusedEffortId === comparisonRow.effort.id;
              const speedValue = comparisonRow.isLeader
                ? formatSpeed(
                    comparisonRow.currentPoint?.speed_mps ?? null,
                    unitSystem,
                  )
                : formatSignedSpeedDelta(
                    comparisonRow.speedDeltaMps,
                    unitSystem,
                  );
              const heartRateValue = comparisonRow.isFinished
                ? "--"
                : formatHeartRate(
                    comparisonRow.currentPoint?.heart_rate_bpm ?? null,
                  );
              const timeValue = comparisonRow.isLeader
                ? "Lead"
                : formatSignedSecondsDelta(comparisonRow.gapSeconds);
              const isPositiveGap = (comparisonRow.gapSeconds ?? 0) > 0;
              const isNegativeGap = (comparisonRow.gapSeconds ?? 0) < 0;
              const isPositiveSpeed = (comparisonRow.speedDeltaMps ?? 0) > 0;
              const isNegativeSpeed = (comparisonRow.speedDeltaMps ?? 0) < 0;

              return (
                <li
                  key={comparisonRow.effort.id}
                  ref={(element) => {
                    if (element) {
                      rowRefs.current.set(comparisonRow.effort.id, element);
                    } else {
                      rowRefs.current.delete(comparisonRow.effort.id);
                    }
                  }}
                  className={`list-row grid min-w-0 items-center gap-3 rounded-none border-b border-base-300 px-3 py-3 transition-colors last:border-b-0 sm:px-4 xl:gap-4 ${athleteGridTemplateClassName} ${
                    isFocused ? "bg-base-200/80" : "bg-transparent"
                  }`}
                  style={{ willChange: "transform" }}
                  onMouseEnter={() => {
                    onHoverEffort(comparisonRow.effort.id);
                  }}
                  onMouseLeave={() => {
                    onHoverEffort(null);
                  }}
                >
                  <span
                    aria-hidden
                    className="inline-flex h-8 w-8 shrink-0 items-center justify-center text-xs font-semibold text-white xl:h-9 xl:w-9 xl:text-sm"
                    style={{ backgroundColor: comparisonRow.color }}
                  >
                    {comparisonRow.markerLabel}
                  </span>

                  <div className="min-w-0 text-left">
                    <div className="min-w-0">
                      <div className="truncate font-semibold leading-tight text-base-content">
                        {comparisonRow.effort.rider_name}
                      </div>
                      <div className="mt-1 truncate text-xs font-medium text-base-content/55">
                        {formatRideDateLabel(
                          comparisonRow.effort.activity_started_at,
                        )}
                      </div>
                    </div>
                  </div>

                  <div
                    className={`justify-self-end text-right font-semibold ${
                      comparisonRow.isLeader
                        ? "text-success"
                        : isPositiveGap
                          ? "text-error"
                          : isNegativeGap
                            ? "text-success"
                            : "text-base-content"
                    }`}
                  >
                    {timeValue}
                  </div>

                  <div
                    className={`justify-self-end text-right font-semibold ${
                      comparisonRow.isLeader
                        ? "text-base-content"
                        : isPositiveSpeed
                          ? "text-success"
                          : isNegativeSpeed
                            ? "text-error"
                            : "text-base-content"
                    }`}
                  >
                    {speedValue}
                  </div>

                  <div className="justify-self-end text-right text-sm font-medium text-base-content">
                    {heartRateValue}
                  </div>

                  <button
                    type="button"
                    className="inline-flex h-4 w-4 justify-self-end items-center justify-center text-base-content/50 transition hover:text-base-content"
                    aria-label={`Remove ${comparisonRow.effort.activity_title} from comparison`}
                    onClick={() => {
                      onRemoveEffort(comparisonRow.effort.id);
                    }}
                  >
                    <FontAwesomeIcon icon={faXmark} className="h-3 w-3" />
                  </button>
                </li>
              );
            })}
          </ul>
        </div>
      </div>
    </div>
  ) : (
    <div className="flex h-full min-h-[24rem] items-center justify-center p-4">
      <div className="alert bg-base-100 text-sm text-base-content/70">
        <span>Add rides from the effort list to start the comparison.</span>
      </div>
    </div>
  );
}

type SegmentDetailComparisonSectionProps = {
  isLoading: boolean;
  unitSystem: UnitSystem;
  workspace: SegmentComparisonWorkspace;
  playback: SegmentComparisonPlayback;
};

export default function SegmentDetailComparisonSection({
  isLoading,
  unitSystem,
  workspace,
  playback,
}: SegmentDetailComparisonSectionProps) {
  const {
    routePoints,
    routeDistanceMeters,
    selectedRows,
    liveComparisonRows,
    focusedEffortId,
    raceViewerHref,
    actions,
  } = workspace;

  return (
    <AppCard bodyClassName="gap-3">
      <CardHeader
        title="Comparison workspace"
        titleExtras={
          <InfoTooltip
            label="Comparison workspace details"
            tip={COMPARISON_WORKSPACE_HELP_TEXT}
          />
        }
        actions={
          raceViewerHref ? (
            <Link
              href={raceViewerHref}
              className="btn btn-sm btn-outline normal-case tracking-normal"
            >
              Open race viewer
            </Link>
          ) : null
        }
      />

      {isLoading ? (
        <div className="flex min-h-[28rem] items-center justify-center border border-base-300 bg-base-200">
          <LoadingSpinner size="lg" aria-label="Loading segment comparison" />
        </div>
      ) : (
        <div className="overflow-hidden border border-base-300 bg-base-200">
          <div className="grid gap-0 xl:grid-cols-[minmax(0,1fr)_minmax(24rem,0.78fr)]">
            <div className="border-b border-base-300 xl:order-2 xl:border-b-0 xl:border-l">
              <SelectedEffortsPanel
                comparisonRows={liveComparisonRows}
                focusedEffortId={focusedEffortId}
                unitSystem={unitSystem}
                onHoverEffort={actions.hoverEffort}
                onRemoveEffort={actions.removeEffort}
              />
            </div>

            <div className="min-h-[24rem] xl:order-1">
              <RouteComparisonMap
                routePoints={routePoints}
                selectedRows={selectedRows}
                playbackSeconds={playback.seconds}
              />
            </div>
          </div>

          <div className="flex flex-wrap items-center gap-3 border-t border-base-300 bg-base-100 px-3 py-3 sm:px-4">
            <div className="flex min-w-0 flex-1 items-center gap-3 max-[420px]:gap-2">
              <button
                type="button"
                className="btn btn-sm btn-circle shrink-0 border-0 bg-orange-500 text-white hover:bg-orange-600"
                disabled={
                  selectedRows.length === 0 || playback.limitSeconds <= 0
                }
                aria-label={
                  playback.isPlaying
                    ? "Pause comparison playback"
                    : playback.seconds >= playback.limitSeconds
                      ? "Replay comparison playback"
                      : "Play comparison playback"
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

              <input
                type="range"
                min={0}
                max={Math.max(playback.limitSeconds, 1)}
                step={0.1}
                value={Math.min(
                  playback.seconds,
                  Math.max(playback.limitSeconds, 1),
                )}
                className="range range-primary min-w-0 flex-1"
                disabled={
                  selectedRows.length === 0 || playback.limitSeconds <= 0
                }
                aria-label="Playback timeline"
                onChange={(event) => {
                  playback.setSeconds(Number(event.target.value));
                  playback.setIsPlaying(false);
                }}
              />

              <span className="shrink-0 rounded-full border border-base-300 px-2.5 py-1 text-xs font-semibold tabular-nums text-base-content/80 max-[420px]:hidden">
                {formatDuration(Math.round(playback.seconds))} /{" "}
                {formatDuration(playback.limitSeconds)}
              </span>
            </div>

            <div className="join shrink-0 max-[420px]:hidden">
              {PLAYBACK_PACE_OPTIONS.map((option) => (
                <button
                  key={option.key}
                  type="button"
                  className={`join-item btn btn-sm ${
                    playback.pace === option.key ? "btn-neutral" : "btn-ghost"
                  }`}
                  aria-pressed={playback.pace === option.key}
                  onClick={() => {
                    playback.setPace(option.key);
                  }}
                >
                  {option.label}
                </button>
              ))}
            </div>
          </div>

          <div className="border-t border-base-300 bg-base-100/95">
            <ComparisonChart
              routePoints={routePoints}
              routeDistanceMeters={routeDistanceMeters}
              selectedRows={selectedRows}
              playbackSeconds={playback.seconds}
              unitSystem={unitSystem}
            />
          </div>
        </div>
      )}
    </AppCard>
  );
}
