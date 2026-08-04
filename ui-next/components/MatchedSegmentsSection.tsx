"use client";

import {
  faChevronDown,
  faChevronUp,
  faCrown,
  faMedal,
  faRocket,
  faStar,
  faTrophy,
} from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import Link from "next/link";
import { useEffect, useMemo, useState, type KeyboardEvent } from "react";
import { CartesianGrid, Line, LineChart, XAxis, YAxis } from "recharts";
import { formatDuration, formatHeartRate } from "../lib/activityFormatting";
import {
  type ActivityRoutePoint,
  type ActivitySegmentEffort,
  useActivity,
  useSegments,
  useUpdateSegment,
} from "../lib/queries";
import {
  primarySegmentAchievement,
  type SegmentAchievement,
  type SegmentAchievementKind,
} from "../lib/segmentAchievements";
import {
  segmentOverlayPoints,
  type MatchedSegmentGroup,
  useMatchedSegmentGroups,
} from "./activity-detail/matchedSegments";

type SegmentAttemptChartPoint = {
  effort: ActivitySegmentEffort;
  segmentTitle: string;
  runNumber: number;
  runLabel: string;
  durationSeconds: number;
  maxHeartRate: number | null;
  overallRank: number | null | undefined;
  personalRank: number | null | undefined;
  personalBestDurationSeconds: number | null | undefined;
  isFastestOfDay: boolean;
};

type SegmentAttemptTooltipState = {
  attempt: SegmentAttemptChartPoint;
  x: number;
  y: number;
};

function formatOverallRank(rank: number | null | undefined) {
  return rank != null ? `#${rank} overall` : "No global rank";
}

function formatPersonalRank(rank: number | null | undefined) {
  return rank != null ? `#${rank} all-time` : "No PR history";
}

function formatPrDelta(
  durationSeconds: number,
  personalBestDurationSeconds: number | null | undefined,
) {
  if (personalBestDurationSeconds == null) {
    return null;
  }

  const deltaSeconds = durationSeconds - personalBestDurationSeconds;

  if (deltaSeconds === 0) {
    return "At PR";
  }

  const magnitude = formatDuration(Math.abs(deltaSeconds));

  if (deltaSeconds > 0) {
    return `${magnitude} off PR`;
  }

  return `${magnitude} faster than prior PR`;
}

function achievementIcon(kind: SegmentAchievementKind) {
  switch (kind) {
    case "kom":
      return faCrown;
    case "top-10":
      return faTrophy;
    case "pr":
      return faMedal;
    case "fastest":
      return faRocket;
  }
}

function achievementBadgeClassName(kind: SegmentAchievementKind) {
  switch (kind) {
    case "kom":
      return "badge badge-warning badge-outline gap-1";
    case "top-10":
      return "badge badge-info badge-outline gap-1";
    case "pr":
      return "badge badge-primary badge-outline gap-1";
    case "fastest":
      return "badge badge-success badge-outline gap-1";
  }
}

function achievementMarkerFill(kind: SegmentAchievementKind) {
  switch (kind) {
    case "kom":
      return "var(--color-warning)";
    case "top-10":
      return "var(--color-info)";
    case "pr":
      return "var(--color-primary)";
    case "fastest":
      return "var(--color-success)";
  }
}

function SegmentAchievementBadge({
  achievement,
}: {
  achievement: SegmentAchievement;
}) {
  return (
    <span className={achievementBadgeClassName(achievement.kind)}>
      <FontAwesomeIcon
        icon={achievementIcon(achievement.kind)}
        className="h-3 w-3"
      />
      <span>{achievement.longLabel}</span>
    </span>
  );
}

function isSameSegmentEffort(
  left: ActivitySegmentEffort,
  right: ActivitySegmentEffort,
) {
  return (
    left.segment_id === right.segment_id &&
    left.effort_index === right.effort_index &&
    left.start_route_point_index === right.start_route_point_index &&
    left.end_route_point_index === right.end_route_point_index
  );
}

function maxHeartRateForSegmentEffort(
  routePoints: ActivityRoutePoint[] | null | undefined,
  segmentEffort: ActivitySegmentEffort,
) {
  const heartRateValues = segmentOverlayPoints(
    routePoints,
    segmentEffort,
  ).flatMap((point) =>
    point.heart_rate_bpm == null || Number.isNaN(point.heart_rate_bpm)
      ? []
      : [point.heart_rate_bpm],
  );

  if (heartRateValues.length === 0) {
    return null;
  }

  return Math.round(Math.max(...heartRateValues));
}

function segmentHistoricalAchievements(segmentGroup: MatchedSegmentGroup) {
  return primarySegmentAchievement({
    overallRank: segmentGroup.bestEffort.overall_rank,
    personalRank: segmentGroup.bestEffort.personal_rank,
  });
}

function AttemptAchievementBadges({
  attempt,
}: {
  attempt: SegmentAttemptChartPoint;
}) {
  const achievement = primarySegmentAchievement({
    overallRank: attempt.overallRank,
    personalRank: attempt.personalRank,
    isFastestOfDay: attempt.isFastestOfDay,
  });

  return achievement ? (
    <SegmentAchievementBadge achievement={achievement} />
  ) : null;
}

function iconPaths(
  icon: typeof faCrown | typeof faMedal | typeof faRocket | typeof faTrophy,
) {
  const pathData = icon.icon[4];
  return Array.isArray(pathData) ? pathData : [pathData];
}

function ChartMarkerIcon({
  icon,
  cx,
  cy,
  fill,
  size,
  stroke,
  strokeWidth = 0,
  offsetX = 0,
  offsetY = 0,
}: {
  icon: typeof faCrown | typeof faMedal | typeof faRocket | typeof faTrophy;
  cx: number;
  cy: number;
  fill: string;
  size: number;
  stroke?: string;
  strokeWidth?: number;
  offsetX?: number;
  offsetY?: number;
}) {
  const [iconWidth, iconHeight] = icon.icon;
  const paths = iconPaths(icon);

  return (
    <svg
      x={cx - size / 2 + offsetX}
      y={cy - size / 2 + offsetY}
      width={size}
      height={size}
      viewBox={`0 0 ${iconWidth} ${iconHeight}`}
      overflow="visible"
      pointerEvents="none"
    >
      {paths.map((path, index) => (
        <path
          key={`${icon.iconName}-${index}`}
          d={path}
          fill={fill}
          stroke={stroke}
          strokeWidth={strokeWidth}
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      ))}
    </svg>
  );
}

function SegmentAttemptTooltipContent({
  attempt,
}: {
  attempt: SegmentAttemptChartPoint;
}) {
  const prDelta = formatPrDelta(
    attempt.durationSeconds,
    attempt.personalBestDurationSeconds,
  );

  return (
    <div className="rounded-box border border-base-300 bg-base-100 px-3 py-3 shadow-lg">
      <p className="text-sm font-semibold text-base-content">
        {`Run ${attempt.runNumber} · ${formatDuration(attempt.durationSeconds)}`}
      </p>
      <p className="mt-1 text-sm text-base-content/70">
        {`Leaderboard ${formatOverallRank(attempt.overallRank)} · Max heart rate ${formatHeartRate(attempt.maxHeartRate)}`}
      </p>
      <p className="mt-1 text-xs text-base-content/60">
        {`Personal rank ${formatPersonalRank(attempt.personalRank)}`}
      </p>
      {prDelta ? (
        <p className="mt-1 text-xs font-medium text-base-content/75">
          {prDelta}
        </p>
      ) : null}
      <div className="mt-2 flex flex-wrap gap-2">
        <AttemptAchievementBadges attempt={attempt} />
      </div>
    </div>
  );
}

function SegmentAttemptDot({
  active = false,
  cx,
  cy,
  payload,
  toneColor,
  isHighlighted,
  onDismiss,
  onSelect,
  ...interactionProps
}: {
  active?: boolean;
  cx?: number;
  cy?: number;
  payload?: SegmentAttemptChartPoint;
  toneColor: string;
  isHighlighted: boolean;
  onDismiss: () => void;
  onSelect: (state: SegmentAttemptTooltipState) => void;
  [key: string]: unknown;
}) {
  if (cx == null || cy == null || !payload) {
    return null;
  }

  const achievement = primarySegmentAchievement({
    overallRank: payload.overallRank,
    personalRank: payload.personalRank,
    isFastestOfDay: payload.isFastestOfDay,
  });
  const showKomMarker = achievement?.kind === "kom";
  const showTopMarker = achievement?.kind === "top-10";
  const showPrMarker = achievement?.kind === "pr";
  const showFastestMarker = achievement?.kind === "fastest";
  const hitRadius =
    showKomMarker || showTopMarker || showPrMarker || showFastestMarker
      ? 14
      : 11;
  const highlightRadius =
    showKomMarker || showTopMarker || showPrMarker
      ? 8.5
      : showFastestMarker
        ? 7.5
        : 5.25;

  return (
    <g
      {...interactionProps}
      role="button"
      tabIndex={0}
      aria-label={`${payload.segmentTitle} run ${payload.runNumber} point`}
      className="cursor-pointer"
      onBlur={() => {
        onDismiss();
      }}
      onClick={() => {
        onSelect({ attempt: payload, x: cx, y: cy });
      }}
      onFocus={() => {
        onSelect({ attempt: payload, x: cx, y: cy });
      }}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onSelect({ attempt: payload, x: cx, y: cy });
        }
      }}
      onMouseEnter={() => {
        onSelect({ attempt: payload, x: cx, y: cy });
      }}
      onMouseLeave={() => {
        onDismiss();
      }}
    >
      <circle cx={cx} cy={cy} r={hitRadius} fill="transparent" />
      {isHighlighted ? (
        <circle
          cx={cx}
          cy={cy}
          r={highlightRadius}
          fill="none"
          stroke="var(--color-base-content)"
          strokeOpacity={0.35}
          strokeWidth={1.25}
        />
      ) : null}

      {showKomMarker ? (
        <ChartMarkerIcon
          icon={faCrown}
          cx={cx}
          cy={cy}
          fill={achievementMarkerFill("kom")}
          size={active || isHighlighted ? 20 : 18}
          stroke="var(--color-base-100)"
          strokeWidth={24}
        />
      ) : null}

      {showTopMarker ? (
        <ChartMarkerIcon
          icon={faTrophy}
          cx={cx}
          cy={cy}
          fill={achievementMarkerFill("top-10")}
          size={active || isHighlighted ? 18 : 16}
          stroke="var(--color-base-100)"
          strokeWidth={24}
        />
      ) : null}

      {showPrMarker ? (
        <ChartMarkerIcon
          icon={faMedal}
          cx={cx}
          cy={cy}
          fill={achievementMarkerFill("pr")}
          size={active || isHighlighted ? 20 : 18}
          stroke="var(--color-base-100)"
          strokeWidth={24}
        />
      ) : null}

      {showFastestMarker ? (
        <ChartMarkerIcon
          icon={faRocket}
          cx={cx}
          cy={cy}
          fill="var(--color-success)"
          size={active || isHighlighted ? 18 : 16}
          stroke="var(--color-base-100)"
          strokeWidth={26}
        />
      ) : null}

      {!showKomMarker && !showPrMarker && !showFastestMarker ? (
        <circle
          cx={cx}
          cy={cy}
          r={active || isHighlighted ? 4.25 : 3.5}
          fill={toneColor}
          stroke="var(--color-base-100)"
          strokeWidth={1}
        />
      ) : null}
    </g>
  );
}

function SegmentAttemptsChart({
  segmentGroup,
  routePoints,
}: {
  segmentGroup: MatchedSegmentGroup;
  routePoints: ActivityRoutePoint[] | null | undefined;
}) {
  const [tooltipState, setTooltipState] =
    useState<SegmentAttemptTooltipState | null>(null);
  const chartWidth = 520;
  const chartHeight = 120;
  const attempts: SegmentAttemptChartPoint[] = [...segmentGroup.efforts]
    .sort(
      (left, right) =>
        left.effort_index - right.effort_index ||
        left.start_route_point_index - right.start_route_point_index ||
        left.end_route_point_index - right.end_route_point_index,
    )
    .map((effort) => ({
      effort,
      segmentTitle: segmentGroup.segmentTitle,
      runNumber: effort.effort_index,
      runLabel: `Run ${effort.effort_index}`,
      durationSeconds: effort.duration_seconds,
      maxHeartRate: maxHeartRateForSegmentEffort(routePoints, effort),
      overallRank: effort.overall_rank,
      personalRank: effort.personal_rank,
      personalBestDurationSeconds: effort.personal_best_duration_seconds,
      isFastestOfDay: isSameSegmentEffort(effort, segmentGroup.bestEffort),
    }));

  if (attempts.length === 0) {
    return null;
  }

  const minDuration = Math.min(
    ...attempts.map((attempt) => attempt.durationSeconds),
  );
  const maxDuration = Math.max(
    ...attempts.map((attempt) => attempt.durationSeconds),
  );
  const durationSpread = Math.max(maxDuration - minDuration, 1);
  const chartPadding = Math.max(4, Math.round(durationSpread * 0.12));
  const yAxisDomain: [number, number] = [
    Math.max(0, minDuration - chartPadding),
    maxDuration + chartPadding,
  ];
  const xAxisTicks = attempts.map((attempt) => attempt.runNumber);
  const xAxisDomain: [number, number] = [
    Math.min(...xAxisTicks),
    Math.max(...xAxisTicks),
  ];

  return (
    <div
      role="img"
      aria-label={`${segmentGroup.segmentTitle} attempts chart`}
      className="relative overflow-visible rounded-box border border-base-300 bg-base-100 p-2"
    >
      {tooltipState ? (
        <div
          className="pointer-events-none absolute z-10 w-max max-w-64 -translate-x-1/2 -translate-y-[calc(100%+0.75rem)]"
          style={{
            left: `${(tooltipState.x / chartWidth) * 100}%`,
            top: `${(tooltipState.y / chartHeight) * 100}%`,
          }}
        >
          <SegmentAttemptTooltipContent attempt={tooltipState.attempt} />
        </div>
      ) : null}
      <LineChart
        width={chartWidth}
        height={chartHeight}
        data={attempts}
        margin={{ top: 10, right: 14, bottom: 4, left: 4 }}
        style={{ width: "100%", height: "auto" }}
        onMouseLeave={() => {
          setTooltipState(null);
        }}
      >
        <CartesianGrid
          vertical={false}
          stroke="var(--color-base-content)"
          strokeOpacity={0.12}
        />
        <XAxis
          axisLine={false}
          allowDecimals={false}
          dataKey="runNumber"
          domain={xAxisDomain}
          tickFormatter={(value: number) => `Run ${value}`}
          tick={{ fill: "var(--color-base-content)", fontSize: 8 }}
          tickLine={false}
          ticks={xAxisTicks}
          type="number"
        />
        <YAxis
          axisLine={false}
          domain={yAxisDomain}
          label={{
            angle: -90,
            fill: "var(--color-base-content)",
            fontSize: 8,
            position: "insideLeft",
            style: { opacity: 0.65 },
            value: "Time",
          }}
          tick={{ fill: "var(--color-base-content)", fontSize: 8 }}
          tickFormatter={(value: number) => formatDuration(Math.round(value))}
          tickLine={false}
          tickMargin={6}
          width={56}
        />
        <Line
          type="linear"
          dataKey="durationSeconds"
          stroke={segmentGroup.tone.mapColor}
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth={2}
          dot={(dotProps) => (
            <SegmentAttemptDot
              active={false}
              cx={dotProps.cx}
              cy={dotProps.cy}
              isHighlighted={
                tooltipState?.attempt.runNumber ===
                (dotProps.payload as SegmentAttemptChartPoint).runNumber
              }
              onDismiss={() => {
                setTooltipState(null);
              }}
              payload={dotProps.payload as SegmentAttemptChartPoint}
              onSelect={setTooltipState}
              toneColor={segmentGroup.tone.mapColor}
            />
          )}
          activeDot={false}
        />
      </LineChart>
    </div>
  );
}

export default function MatchedSegmentsSection({
  activityId,
  selectedSegmentId,
  onToggleSegmentMatch,
}: {
  activityId: number | string;
  selectedSegmentId: number | null;
  onToggleSegmentMatch: (segmentId: number) => void;
}) {
  const [expandedSegmentIds, setExpandedSegmentIds] = useState<number[]>([]);
  const activityQuery = useActivity(activityId);
  const activity = activityQuery.data;
  const routePoints = activity?.route_points;
  const segmentGroups = useMatchedSegmentGroups(activity);
  const segmentsQuery = useSegments();
  const updateSegmentMutation = useUpdateSegment();
  const starredSegmentIds = useMemo(
    () =>
      new Set(
        (segmentsQuery.data ?? [])
          .filter((segment) => segment.starred)
          .map((segment) => segment.id),
      ),
    [segmentsQuery.data],
  );
  const starredSegmentIdsKey = useMemo(
    () =>
      Array.from(starredSegmentIds)
        .sort((left, right) => left - right)
        .join(","),
    [starredSegmentIds],
  );

  useEffect(() => {
    if (starredSegmentIds.size === 0) {
      return;
    }

    setExpandedSegmentIds((current) => {
      const next = new Set(current);

      for (const segmentId of starredSegmentIds) {
        next.add(segmentId);
      }

      return next.size === current.length ? current : Array.from(next);
    });
  }, [starredSegmentIds, starredSegmentIdsKey]);

  useEffect(() => {
    if (selectedSegmentId == null) {
      return;
    }

    setExpandedSegmentIds((current) =>
      current.includes(selectedSegmentId)
        ? current
        : [...current, selectedSegmentId],
    );
  }, [selectedSegmentId]);

  async function toggleSegmentStar(segmentId: number, starred: boolean) {
    try {
      await updateSegmentMutation.updateAsync({ id: segmentId, starred });
      if (starred) {
        setExpandedSegmentIds((current) =>
          current.includes(segmentId) ? current : [...current, segmentId],
        );
      }
    } catch {
      // The mutation exposes error state where segment controls are rendered.
    }
  }

  function toggleSegmentMatch(segmentId: number) {
    setExpandedSegmentIds((current) =>
      current.includes(segmentId)
        ? current.filter((entry) => entry !== segmentId)
        : [...current, segmentId],
    );
    onToggleSegmentMatch(segmentId);
  }

  function handleToggleRow(
    segmentId: number,
    isStarred: boolean,
    event?: KeyboardEvent<HTMLDivElement>,
  ) {
    if (isStarred) {
      return;
    }

    if (event && event.key !== "Enter" && event.key !== " ") {
      return;
    }

    event?.preventDefault();
    toggleSegmentMatch(segmentId);
  }

  return (
    <div className="card bg-base-100 shadow-xl">
      <div className="card-body">
        {segmentGroups.length > 0 && (
          <ul className="list overflow-hidden bg-base-100">
            <li className="text-xs opacity-60 uppercase">Matched segments</li>

            {segmentGroups.map((segmentGroup) => {
              const segmentHref = `/segments/${segmentGroup.segmentId}`;
              const segmentAchievement =
                segmentHistoricalAchievements(segmentGroup);
              const isSelected = selectedSegmentId === segmentGroup.segmentId;
              const isStarred = starredSegmentIds.has(segmentGroup.segmentId);
              const isExpanded =
                isStarred ||
                isSelected ||
                expandedSegmentIds.includes(segmentGroup.segmentId);

              return (
                <li
                  id={segmentGroup.anchorId}
                  key={segmentGroup.segmentId}
                  className={`list-row grid-cols-1 w-full p-0 transition ${isSelected ? `${segmentGroup.tone.highlightClassName} bg-base-200/60` : "bg-base-100"} border-b border-base-300 last:border-b-0`}
                >
                  <div
                    className={`collapse min-w-0 w-full rounded-none ring-1 ring-inset transition ${isSelected ? segmentGroup.tone.highlightClassName : "ring-transparent"} ${isExpanded ? "collapse-open" : "collapse-close"}`}
                  >
                    <div
                      role="button"
                      tabIndex={0}
                      className="collapse-title grid grid-cols-[auto_minmax(0,1fr)_auto] items-start gap-3 px-3 py-3 pr-3 sm:px-4 sm:pr-4"
                      aria-expanded={isExpanded}
                      aria-controls={`${segmentGroup.anchorId}-details`}
                      onClick={() => {
                        if (isStarred) {
                          return;
                        }

                        toggleSegmentMatch(segmentGroup.segmentId);
                      }}
                      onKeyDown={(event) => {
                        handleToggleRow(
                          segmentGroup.segmentId,
                          isStarred,
                          event,
                        );
                      }}
                    >
                      <button
                        type="button"
                        className={`btn btn-ghost btn-sm btn-square self-center ${isStarred ? "text-warning" : "text-base-content/45"}`}
                        aria-label={`${isStarred ? "Unstar" : "Star"} ${segmentGroup.segmentTitle}`}
                        aria-pressed={isStarred}
                        disabled={updateSegmentMutation.isPending}
                        onClick={(event) => {
                          event.stopPropagation();
                          void toggleSegmentStar(
                            segmentGroup.segmentId,
                            !isStarred,
                          );
                        }}
                      >
                        <FontAwesomeIcon
                          icon={faStar}
                          className="h-3.5 w-3.5"
                        />
                      </button>

                      <div className="min-w-0">
                        <div className="min-w-0">
                          <Link
                            href={segmentHref}
                            className="truncate text-base font-semibold text-base-content transition hover:text-primary"
                            onClick={(event) => {
                              event.stopPropagation();
                            }}
                          >
                            {segmentGroup.segmentTitle}
                          </Link>
                        </div>
                        <p className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-base-content/60">
                          <span
                            aria-hidden
                            className={`inline-block h-2 w-2 rounded-full ${segmentGroup.tone.dotClassName}`}
                          />
                          <span>
                            {segmentGroup.efforts.length} run
                            {segmentGroup.efforts.length === 1 ? "" : "s"}
                          </span>
                          <span>
                            {formatOverallRank(segmentGroup.bestOverallRank)}
                          </span>
                        </p>
                      </div>

                      <div className="flex items-center self-center justify-end">
                        <button
                          type="button"
                          className="btn btn-ghost btn-sm btn-square"
                          aria-label={
                            isStarred
                              ? "Starred stays open"
                              : isExpanded
                                ? "Hide time & runs"
                                : "Show time & runs"
                          }
                          aria-expanded={isExpanded}
                          aria-controls={`${segmentGroup.anchorId}-details`}
                          disabled={isStarred}
                          onClick={(event) => {
                            event.stopPropagation();

                            if (isStarred) {
                              return;
                            }

                            toggleSegmentMatch(segmentGroup.segmentId);
                          }}
                        >
                          <FontAwesomeIcon
                            icon={isExpanded ? faChevronUp : faChevronDown}
                            className="h-3.5 w-3.5"
                          />
                        </button>
                      </div>
                    </div>

                    {isExpanded ? (
                      <div
                        id={`${segmentGroup.anchorId}-details`}
                        className="collapse-content list-col-wrap px-3 pb-4 sm:px-4"
                      >
                        <div className="grid gap-3">
                          <div className="flex flex-wrap gap-2">
                            <span className="badge badge-outline">
                              Best{" "}
                              {formatDuration(
                                segmentGroup.bestEffort.duration_seconds,
                              )}
                            </span>
                            <span className="badge badge-outline">
                              Leaderboard{" "}
                              {formatOverallRank(segmentGroup.bestOverallRank)}
                            </span>
                            <span className="badge badge-outline">
                              Peak HR{" "}
                              {formatHeartRate(segmentGroup.peakHeartRate)}
                            </span>
                            {segmentAchievement ? (
                              <SegmentAchievementBadge
                                achievement={segmentAchievement}
                              />
                            ) : null}
                          </div>

                          <SegmentAttemptsChart
                            segmentGroup={segmentGroup}
                            routePoints={routePoints}
                          />
                        </div>
                      </div>
                    ) : null}
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </div>
  );
}
