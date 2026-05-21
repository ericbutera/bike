"use client";

import { faPause, faPlay, faXmark } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { useLayoutEffect, useMemo, useRef, useState } from "react";
import {
  Area,
  CartesianGrid,
  ComposedChart,
  Line,
  ReferenceDot,
  ReferenceLine,
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
  PLAYBACK_PACE_OPTIONS,
  buildGapChartRowAtProgress,
  buildGapChartRows,
  buildPlaybackGapMarker,
  comparisonMarkerPoint,
  effortProgressAtElapsed,
  effortSeriesDataKey,
  formatSignedSecondsDelta,
  formatSignedSpeedDelta,
  type GapChartRow,
  type LiveComparisonRow,
  type PlaybackPace,
  type SelectedEffortRow,
} from "../../lib/segmentDetail";
import MapLibreRouteMap from "../MapLibreRouteMap";

function ComparisonGapChartTooltip({
  active,
  label,
  payload,
  selectedRows,
  unitSystem,
}: {
  active?: boolean;
  label?: number;
  payload?: Array<{
    color?: string;
    dataKey?: string;
    value?: number | string | null;
  }>;
  selectedRows: SelectedEffortRow[];
  unitSystem: UnitSystem;
}) {
  if (!active || typeof label !== "number") {
    return null;
  }

  const elevationValue = payload?.find(
    (entry) => entry.dataKey === "elevation",
  )?.value;

  return (
    <div className="border border-base-300 bg-base-100 px-3 py-3 shadow-lg">
      <p className="text-sm font-semibold text-base-content">
        {formatDistance(label, unitSystem)}
      </p>
      <p className="mt-1 text-sm text-base-content/70">
        Elevation {formatElevation(Number(elevationValue ?? null), unitSystem)}
      </p>
      <div className="mt-2 space-y-1.5 text-sm text-base-content/75">
        {selectedRows.map((selectedRow) => {
          const value = payload?.find(
            (entry) =>
              entry.dataKey === effortSeriesDataKey(selectedRow.effort.id),
          )?.value;

          return (
            <div
              key={selectedRow.effort.id}
              className="border border-base-300 bg-base-200/70 px-2 py-2"
              style={{ borderLeftColor: selectedRow.color, borderLeftWidth: 4 }}
            >
              <div className="flex items-center justify-between gap-3">
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <span
                      aria-hidden
                      className="inline-flex h-5 w-5 items-center justify-center text-[0.65rem] font-semibold text-white"
                      style={{ backgroundColor: selectedRow.color }}
                    >
                      {selectedRow.markerLabel}
                    </span>
                    <span className="truncate font-medium text-base-content">
                      {selectedRow.effort.rider_name}
                    </span>
                  </div>
                  <div className="truncate pl-7 text-xs text-base-content/65">
                    {selectedRow.effort.activity_title}
                  </div>
                </div>
                <span className="whitespace-nowrap font-medium text-base-content">
                  {formatSignedSecondsDelta(
                    typeof value === "number" ? value : null,
                  )}
                </span>
              </div>
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
  referenceEffortId,
  playbackSeconds,
  unitSystem,
}: {
  routePoints: ActivityRoutePoint[] | null | undefined;
  routeDistanceMeters: number | null | undefined;
  selectedRows: SelectedEffortRow[];
  referenceEffortId: number | null;
  playbackSeconds: number;
  unitSystem: UnitSystem;
}) {
  const [hoveredRow, setHoveredRow] = useState<GapChartRow | null>(null);
  const referenceEffort =
    selectedRows.find(
      (selectedRow) => selectedRow.effort.id === referenceEffortId,
    )?.effort ?? null;
  const chartRows = useMemo(
    () =>
      buildGapChartRows(
        routePoints,
        selectedRows,
        referenceEffort,
        routeDistanceMeters,
      ),
    [referenceEffort, routeDistanceMeters, routePoints, selectedRows],
  );
  const maxDistance =
    chartRows.at(-1)?.distanceMeters ?? routeDistanceMeters ?? 1;
  const playbackProgress = referenceEffort
    ? effortProgressAtElapsed(referenceEffort, playbackSeconds)
    : null;
  const playbackRow =
    referenceEffort && playbackProgress != null
      ? buildGapChartRowAtProgress(
          playbackProgress,
          routePoints,
          selectedRows,
          referenceEffort,
          routeDistanceMeters,
        )
      : null;
  const displayRow = hoveredRow ?? playbackRow;
  const displayDistance = displayRow?.distanceMeters ?? 0;

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
            tickFormatter={(value: number) => formatSignedSecondsDelta(value)}
            tickLine={false}
            tickMargin={10}
            width={72}
            yAxisId="gap"
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
          <ReferenceLine
            y={0}
            yAxisId="gap"
            stroke="#52525b"
            strokeOpacity={0.8}
          />

          {selectedRows.map((selectedRow) => {
            const isReference = selectedRow.effort.id === referenceEffortId;

            return (
              <Line
                key={selectedRow.effort.id}
                type="linear"
                dataKey={effortSeriesDataKey(selectedRow.effort.id)}
                yAxisId="gap"
                stroke={selectedRow.color}
                strokeWidth={isReference ? 3.2 : 2.4}
                strokeOpacity={1}
                dot={false}
                activeDot={false}
                connectNulls
              />
            );
          })}

          {!hoveredRow && displayRow && displayDistance > 0 ? (
            <ReferenceLine
              x={displayDistance}
              stroke="#52525b"
              strokeDasharray="4 4"
            />
          ) : null}

          {!hoveredRow && displayRow
            ? selectedRows.map((selectedRow) => {
                const playbackMarker = buildPlaybackGapMarker(
                  selectedRow,
                  routePoints,
                  selectedRows,
                  referenceEffort,
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
                    yAxisId="gap"
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
  referenceEffortId,
  playbackSeconds,
  unitSystem,
  onHoverEffort,
  onTogglePinnedEffort,
  onRemoveEffort,
}: {
  comparisonRows: LiveComparisonRow[];
  focusedEffortId: number | null;
  referenceEffortId: number | null;
  playbackSeconds: number;
  unitSystem: UnitSystem;
  onHoverEffort: (effortId: number | null) => void;
  onTogglePinnedEffort: (effortId: number) => void;
  onRemoveEffort: (effortId: number) => void;
}) {
  const rowRefs = useRef(new Map<number, HTMLLIElement>());
  const previousRowTopByEffortIdRef = useRef(new Map<number, number>());
  const animationFrameRef = useRef<number | null>(null);
  const gridTemplateColumns =
    "auto minmax(0,1fr) 5.25rem 6.5rem 4.5rem 1.25rem";
  const sortedComparisonRows = useMemo(() => {
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
    <div className="flex h-full min-h-[24rem] flex-col bg-base-100">
      <div className="flex-1 overflow-hidden">
        <div
          className="h-full overflow-y-auto overflow-x-hidden"
          style={{ scrollbarGutter: "stable" }}
        >
          <div className="sticky top-0 z-10 border-b border-base-300 bg-base-100 px-4 py-3">
            <div
              className="grid items-center gap-4 text-xs font-semibold uppercase tracking-[0.14em] text-base-content/55"
              style={{ gridTemplateColumns }}
            >
              <span className="col-span-2">Athletes</span>
              <span className="justify-self-end text-right">Time</span>
              <span className="justify-self-end text-right">Speed</span>
              <span className="justify-self-end text-right">HR</span>
              <span aria-hidden="true" className="block" />
            </div>
          </div>

          <ul className="list bg-base-100 p-0">
            {sortedComparisonRows.map((comparisonRow) => {
              const isFocused = focusedEffortId === comparisonRow.effort.id;
              const speedValue = comparisonRow.isReference
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
              const timeValue = comparisonRow.isReference
                ? formatDuration(
                    Math.round(
                      Math.min(
                        playbackSeconds,
                        comparisonRow.effort.duration_seconds,
                      ),
                    ),
                  )
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
                  className={`list-row grid min-w-0 items-center gap-4 rounded-none border-b border-base-300 px-4 py-3 transition-colors last:border-b-0 ${isFocused ? "bg-base-200/80" : "bg-transparent"}`}
                  style={{ gridTemplateColumns, willChange: "transform" }}
                  onMouseEnter={() => {
                    onHoverEffort(comparisonRow.effort.id);
                  }}
                  onMouseLeave={() => {
                    onHoverEffort(null);
                  }}
                >
                  <span
                    aria-hidden
                    className="inline-flex h-9 w-9 shrink-0 items-center justify-center text-sm font-semibold text-white"
                    style={{ backgroundColor: comparisonRow.color }}
                  >
                    {comparisonRow.markerLabel}
                  </span>

                  <button
                    type="button"
                    className="min-w-0 text-left"
                    aria-pressed={comparisonRow.effort.id === referenceEffortId}
                    aria-label={`Make ${comparisonRow.effort.activity_title} the reference ride`}
                    onFocus={() => {
                      onHoverEffort(comparisonRow.effort.id);
                    }}
                    onBlur={() => {
                      onHoverEffort(null);
                    }}
                    onClick={() => {
                      onTogglePinnedEffort(comparisonRow.effort.id);
                    }}
                  >
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
                  </button>

                  <div
                    className={`justify-self-end text-right font-semibold ${comparisonRow.isReference ? "text-base-content" : isPositiveGap ? "text-success" : isNegativeGap ? "text-error" : "text-base-content"}`}
                  >
                    {timeValue}
                  </div>

                  <div
                    className={`justify-self-end text-right font-semibold ${comparisonRow.isReference ? "text-base-content" : isPositiveSpeed ? "text-success" : isNegativeSpeed ? "text-error" : "text-base-content"}`}
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
  routePoints: ActivityRoutePoint[] | null | undefined;
  routeDistanceMeters: number | null | undefined;
  selectedRows: SelectedEffortRow[];
  liveComparisonRows: LiveComparisonRow[];
  focusedEffortId: number | null;
  referenceEffortId: number | null;
  referenceSummaryLabel: string;
  playbackLimitSeconds: number;
  playbackSeconds: number;
  isPlaying: boolean;
  playbackPace: PlaybackPace;
  targetPlaybackDurationSeconds: number;
  unitSystem: UnitSystem;
  onHoverEffort: (effortId: number | null) => void;
  onTogglePinnedEffort: (effortId: number) => void;
  onRemoveEffort: (effortId: number) => void;
  onPlaybackSecondsChange: (seconds: number) => void;
  onPlayingChange: (isPlaying: boolean) => void;
  onPlaybackPaceChange: (pace: PlaybackPace) => void;
};

export default function SegmentDetailComparisonSection({
  routePoints,
  routeDistanceMeters,
  selectedRows,
  liveComparisonRows,
  focusedEffortId,
  referenceEffortId,
  referenceSummaryLabel,
  playbackLimitSeconds,
  playbackSeconds,
  isPlaying,
  playbackPace,
  targetPlaybackDurationSeconds,
  unitSystem,
  onHoverEffort,
  onTogglePinnedEffort,
  onRemoveEffort,
  onPlaybackSecondsChange,
  onPlayingChange,
  onPlaybackPaceChange,
}: SegmentDetailComparisonSectionProps) {
  return (
    <div className="card bg-base-100 shadow-xl">
      <div className="card-body gap-4">
        <div className="flex flex-wrap items-end justify-between gap-4">
          <div>
            <h2 className="text-xl font-semibold text-base-content">
              Comparison workspace
            </h2>
            <p className="text-sm text-base-content/70">
              Playback follows the reference ride so time gaps, speed, and heart
              rate update on every frame.
            </p>
          </div>
          <span className="badge badge-outline whitespace-nowrap">
            Ref: {referenceSummaryLabel}
          </span>
        </div>

        <div className="overflow-hidden border border-base-300 bg-base-200">
          <div className="grid gap-0 xl:grid-cols-[minmax(0,1fr)_minmax(24rem,0.78fr)]">
            <div className="min-h-[24rem] border-b border-base-300 xl:border-b-0 xl:border-r">
              <RouteComparisonMap
                routePoints={routePoints}
                selectedRows={selectedRows}
                playbackSeconds={playbackSeconds}
              />
            </div>

            <SelectedEffortsPanel
              comparisonRows={liveComparisonRows}
              focusedEffortId={focusedEffortId}
              referenceEffortId={referenceEffortId}
              playbackSeconds={playbackSeconds}
              unitSystem={unitSystem}
              onHoverEffort={onHoverEffort}
              onTogglePinnedEffort={onTogglePinnedEffort}
              onRemoveEffort={onRemoveEffort}
            />
          </div>

          <div className="border-t border-base-300 bg-base-100/95">
            <ComparisonChart
              routePoints={routePoints}
              routeDistanceMeters={routeDistanceMeters}
              selectedRows={selectedRows}
              referenceEffortId={referenceEffortId}
              playbackSeconds={playbackSeconds}
              unitSystem={unitSystem}
            />
          </div>

          <div className="flex flex-wrap items-center gap-3 border-t border-base-300 bg-base-100 px-4 py-3">
            <button
              type="button"
              className="btn btn-sm btn-circle shrink-0 border-0 bg-orange-500 text-white hover:bg-orange-600"
              disabled={selectedRows.length === 0 || playbackLimitSeconds <= 0}
              aria-label={
                isPlaying
                  ? "Pause comparison playback"
                  : playbackSeconds >= playbackLimitSeconds
                    ? "Replay comparison playback"
                    : "Play comparison playback"
              }
              onClick={() => {
                if (playbackSeconds >= playbackLimitSeconds) {
                  onPlaybackSecondsChange(0);
                }
                onPlayingChange(!isPlaying);
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
              className="range range-primary min-w-[14rem] flex-1"
              disabled={selectedRows.length === 0 || playbackLimitSeconds <= 0}
              aria-label="Playback timeline"
              onChange={(event) => {
                onPlaybackSecondsChange(Number(event.target.value));
                onPlayingChange(false);
              }}
            />

            <div className="join">
              {PLAYBACK_PACE_OPTIONS.map((option) => (
                <button
                  key={option.key}
                  type="button"
                  className={`join-item btn btn-sm ${playbackPace === option.key ? "btn-neutral" : "btn-ghost"}`}
                  aria-pressed={playbackPace === option.key}
                  onClick={() => {
                    onPlaybackPaceChange(option.key);
                  }}
                >
                  {option.label}
                </button>
              ))}
            </div>

            <span className="badge badge-outline min-w-[11.5rem] justify-center whitespace-nowrap text-center">
              {formatDuration(Math.round(playbackSeconds))} /{" "}
              {formatDuration(playbackLimitSeconds)}
            </span>

            <span className="badge badge-ghost whitespace-nowrap">
              {PLAYBACK_PACE_OPTIONS.find(
                (option) => option.key === playbackPace,
              )?.label ?? "Auto"}{" "}
              {formatDuration(Math.round(targetPlaybackDurationSeconds))} target
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}
