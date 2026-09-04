"use client";

import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import {
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  Legend,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import {
  formatActivityTimestamp,
  formatDuration,
  formatSpeed,
} from "../../lib/activityFormatting";
import {
  type Segment,
  type ActivityRoutePoint,
  type SegmentAnalysisEffortSummary,
  type SegmentAnalysisSection,
  type SegmentAnalysisSectionEffort,
  useSegmentEffortAnalysis,
} from "../../lib/queries";
import { interpolateRoutePointByProgress } from "../../lib/segmentDetail";
import type { RouteMovingMarker, RouteOverlay } from "../RouteMapTypes";
import MapLibreRouteMap from "../MapLibreRouteMap";
import { AppCard, CardHeader } from "../ui/Card";
import InfoTooltip from "../ui/InfoTooltip";
import { LoadingSpinner } from "../ui/QueryState";

const SPLIT_COUNT_OPTIONS = [5, 10, 20] as const;
const ANALYSIS_MAP_FIT_BOUNDS_PADDING = 24;
const ANALYSIS_MAP_FIT_BOUNDS_MAX_ZOOM = 18;
const ANALYSIS_HELP_TEXT =
  "Splits are normalized by distance along the segment, then the selected ride is compared with the fastest observed split for each section.";

type ChartRow = {
  section: string;
  sectionIndex: number;
  selectedAvailable: number;
};

type ChartInteractionState = {
  activePayload?: Array<{
    payload?: {
      sectionIndex?: unknown;
    };
  }>;
};

type ChartTooltipEntry = {
  payload?: ChartRow;
  value?: number | string | null;
};

function formatSeconds(value: number) {
  return `${value.toFixed(1)}s`;
}

function effortLabel(effort: SegmentAnalysisEffortSummary) {
  return `${formatDuration(effort.duration_seconds)} - ${effort.activity_title}`;
}

function sectionLabel(section: SegmentAnalysisSection) {
  return `${section.start_progress_percent}-${section.end_progress_percent}%`;
}

function sectionByIndex(
  sections: SegmentAnalysisSection[],
  sectionIndex?: number | null,
) {
  return sections.find((section) => section.section_index === sectionIndex);
}

function effortById(
  efforts: SegmentAnalysisEffortSummary[],
  effortId: number | null,
) {
  return efforts.find((effort) => effort.effort_id === effortId) ?? null;
}

function SegmentAnalysisChartTooltip({
  active,
  payload,
  sections,
  selectedSectionEffortBySection,
}: {
  active?: boolean;
  payload?: ChartTooltipEntry[];
  sections: SegmentAnalysisSection[];
  selectedSectionEffortBySection: Map<
    number,
    SegmentAnalysisSectionEffort | null
  >;
}) {
  if (!active) {
    return null;
  }

  const sectionIndex = payload?.[0]?.payload?.sectionIndex;
  const section = sectionByIndex(sections, sectionIndex);

  if (!section) {
    return null;
  }

  const selectedSectionEffort = selectedSectionEffortBySection.get(
    section.section_index,
  );

  return (
    <div className="max-w-sm border border-base-300 bg-base-100 px-3 py-3 text-sm shadow-lg">
      <div className="font-semibold">
        S{section.section_index}{" "}
        <span className="text-base-content/60">{sectionLabel(section)}</span>
      </div>
      <div className="mt-2 grid grid-cols-3 gap-2 text-xs">
        <div>
          <div className="text-base-content/50">Selected</div>
          <div className="font-semibold">
            {selectedSectionEffort
              ? formatSeconds(selectedSectionEffort.split_seconds)
              : "--"}
          </div>
        </div>
        <div>
          <div className="text-base-content/50">Best</div>
          <div className="font-semibold">
            {formatSeconds(section.best_split_seconds)}
          </div>
        </div>
        <div>
          <div className="text-base-content/50">Available</div>
          <div className="font-semibold text-success">
            {selectedSectionEffort
              ? formatSeconds(selectedSectionEffort.delta_from_best_seconds)
              : "--"}
          </div>
        </div>
      </div>
      <div className="mt-3 border-t border-base-300 pt-2">
        <div className="text-xs font-medium uppercase text-base-content/50">
          Top splits
        </div>
        <div className="mt-1 grid gap-1">
          {section.top_efforts.map((effort, index) => (
            <div
              key={effort.effort_id}
              className="grid grid-cols-[1.5rem_3rem_minmax(0,1fr)] gap-2 text-xs"
            >
              <span className="text-base-content/50">#{index + 1}</span>
              <span className="font-semibold">
                {formatSeconds(effort.split_seconds)}
              </span>
              <span className="truncate" title={effort.activity_title}>
                {effort.activity_title} ·{" "}
                {formatActivityTimestamp(effort.activity_started_at)}
              </span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

function chartSectionIndex(state: unknown) {
  const interaction = state as ChartInteractionState | null;
  const sectionIndex = interaction?.activePayload?.[0]?.payload?.sectionIndex;

  return typeof sectionIndex === "number" && Number.isFinite(sectionIndex)
    ? sectionIndex
    : null;
}

function routePointProgress(
  routePoints: ActivityRoutePoint[],
  pointIndex: number,
) {
  const pointDistance = routePoints[pointIndex]?.distance_meters;
  const firstDistance = routePoints[0]?.distance_meters;
  const lastDistance = routePoints.at(-1)?.distance_meters;

  if (
    typeof pointDistance === "number" &&
    typeof firstDistance === "number" &&
    typeof lastDistance === "number" &&
    lastDistance > firstDistance
  ) {
    return (pointDistance - firstDistance) / (lastDistance - firstDistance);
  }

  return routePoints.length > 1 ? pointIndex / (routePoints.length - 1) : 0;
}

function sectionRoutePoints(
  routePoints: ActivityRoutePoint[],
  section: SegmentAnalysisSection,
) {
  const startProgress = section.start_progress_percent / 100;
  const endProgress = section.end_progress_percent / 100;
  const startPoint = interpolateRoutePointByProgress(
    routePoints,
    startProgress,
  );
  const endPoint = interpolateRoutePointByProgress(routePoints, endProgress);

  if (!startPoint || !endPoint) {
    return [];
  }

  const pointsBetween = routePoints.filter((_point, index) => {
    const progress = routePointProgress(routePoints, index);
    return progress > startProgress && progress < endProgress;
  });

  return [startPoint, ...pointsBetween, endPoint];
}

export default function SegmentEffortAnalysisSection({
  segment,
}: {
  segment: Segment;
}) {
  const [splitCount, setSplitCount] = useState<number>(10);
  const [selectedEffortId, setSelectedEffortId] = useState<number | null>(null);
  const [selectedSectionIndex, setSelectedSectionIndex] = useState<
    number | null
  >(null);
  const [hoveredSectionIndex, setHoveredSectionIndex] = useState<number | null>(
    null,
  );
  const analysisQuery = useSegmentEffortAnalysis(segment.id, { splitCount });
  const analysis = analysisQuery.data;
  const efforts = analysis?.efforts ?? [];
  const referenceEffort = analysis?.reference_effort ?? null;
  const selectedEffort =
    effortById(efforts, selectedEffortId) ?? referenceEffort;
  const selectedSectionEffortBySection = useMemo(() => {
    const pairs =
      analysis?.sections.map<[number, SegmentAnalysisSectionEffort | null]>(
        (section) => [
          section.section_index,
          section.efforts.find(
            (effort) => effort.effort_id === selectedEffort?.effort_id,
          ) ?? null,
        ],
      ) ?? [];

    return new Map<number, SegmentAnalysisSectionEffort | null>(pairs);
  }, [analysis?.sections, selectedEffort?.effort_id]);
  const chartData =
    analysis?.sections.map<ChartRow>((section) => {
      const selectedSectionEffort = selectedSectionEffortBySection.get(
        section.section_index,
      );

      return {
        section: `S${section.section_index}`,
        sectionIndex: section.section_index,
        selectedAvailable: selectedSectionEffort?.delta_from_best_seconds ?? 0,
      };
    }) ?? [];
  const selectedAvailableSeconds = analysis?.sections.reduce(
    (total, section) =>
      total +
      (selectedSectionEffortBySection.get(section.section_index)
        ?.delta_from_best_seconds ?? 0),
    0,
  );
  const activeSectionIndex = hoveredSectionIndex ?? selectedSectionIndex;
  const sectionOverlays = useMemo<RouteOverlay[]>(() => {
    const routePoints = analysis?.route_points ?? [];

    return (
      analysis?.sections.map((section) => {
        const isActive = section.section_index === activeSectionIndex;
        return {
          id: `analysis-section-${section.section_index}`,
          label: `S${section.section_index}: ${sectionLabel(section)}`,
          points: sectionRoutePoints(routePoints, section),
          color: isActive ? "#f97316" : "#2563eb",
          weight: isActive ? 9 : 5,
          onClick: () => {
            setSelectedSectionIndex(section.section_index);
          },
          onMouseEnter: () => {
            setHoveredSectionIndex(section.section_index);
          },
          onMouseLeave: () => {
            setHoveredSectionIndex(null);
          },
        } satisfies RouteOverlay;
      }) ?? []
    );
  }, [activeSectionIndex, analysis?.route_points, analysis?.sections]);
  const sectionMarkers = useMemo<RouteMovingMarker[]>(() => {
    const routePoints = analysis?.route_points ?? [];

    return (
      analysis?.sections
        .map((section) => {
          const midpointProgress =
            (section.start_progress_percent + section.end_progress_percent) /
            200;
          const point = interpolateRoutePointByProgress(
            routePoints,
            midpointProgress,
          );

          return {
            id: `analysis-section-${section.section_index}-marker`,
            point,
            color:
              section.section_index === activeSectionIndex
                ? "#f97316"
                : "#1d4ed8",
            opacity: 0.96,
            label: `S${section.section_index}`,
          } satisfies RouteMovingMarker;
        })
        .filter((marker) => marker.point != null) ?? []
    );
  }, [activeSectionIndex, analysis?.route_points, analysis?.sections]);

  useEffect(() => {
    if (!analysis) {
      return;
    }

    if (
      selectedEffortId == null ||
      !analysis.efforts.some((effort) => effort.effort_id === selectedEffortId)
    ) {
      setSelectedEffortId(analysis.reference_effort.effort_id);
    }
  }, [analysis, selectedEffortId]);

  return (
    <AppCard bodyClassName="gap-5">
      <CardHeader
        title="Effort Analysis"
        titleExtras={
          <InfoTooltip
            label="Effort analysis details"
            tip={ANALYSIS_HELP_TEXT}
          />
        }
      />

      <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-end">
        <div className="grid gap-3">
          <label className="form-control max-w-xl">
            <div className="label">
              <span className="label-text font-medium">Selected ride</span>
            </div>
            <select
              className="select select-bordered"
              value={selectedEffort?.effort_id ?? ""}
              disabled={efforts.length === 0}
              onChange={(event) => {
                setSelectedEffortId(Number(event.target.value));
              }}
            >
              {efforts.map((effort) => (
                <option key={effort.effort_id} value={effort.effort_id}>
                  {effort.effort_id === referenceEffort?.effort_id
                    ? `PR - ${effortLabel(effort)}`
                    : effortLabel(effort)}
                </option>
              ))}
            </select>
          </label>
        </div>

        <div className="join">
          {SPLIT_COUNT_OPTIONS.map((option) => (
            <button
              key={option}
              type="button"
              className={`join-item btn btn-sm ${
                splitCount === option ? "btn-neutral" : "btn-ghost"
              }`}
              onClick={() => {
                setSplitCount(option);
              }}
            >
              {option}
            </button>
          ))}
        </div>
      </div>

      {analysisQuery.isLoading ? (
        <div className="flex min-h-40 items-center justify-center">
          <LoadingSpinner size="md" aria-label="Loading segment analysis" />
        </div>
      ) : !analysis || analysis.sections.length === 0 ? (
        <div className="alert">
          <span>Not enough route data to analyze this segment yet.</span>
        </div>
      ) : (
        <>
          <div className="grid gap-3 sm:grid-cols-3">
            <div className="rounded border border-base-300 bg-base-200/60 p-3">
              <div className="text-xs font-medium uppercase text-base-content/50">
                Selected Ride
              </div>
              <div className="mt-1 font-semibold">
                {selectedEffort
                  ? formatDuration(selectedEffort.duration_seconds)
                  : "--"}
              </div>
              {selectedEffort ? (
                <Link
                  href={`/activities/${selectedEffort.activity_id}`}
                  className="link-hover link mt-1 block truncate text-sm"
                >
                  {selectedEffort.activity_title}
                </Link>
              ) : null}
            </div>
            <div className="rounded border border-base-300 bg-base-200/60 p-3">
              <div className="text-xs font-medium uppercase text-base-content/50">
                Theoretical Best
              </div>
              <div className="mt-1 font-semibold">
                {formatDuration(
                  Math.round(analysis.theoretical_best_duration_seconds),
                )}
              </div>
              <div className="mt-1 text-sm text-success">
                {selectedAvailableSeconds != null
                  ? formatSeconds(selectedAvailableSeconds)
                  : "--"}{" "}
                available from selected ride
              </div>
            </div>
            <div className="rounded border border-base-300 bg-base-200/60 p-3">
              <div className="text-xs font-medium uppercase text-base-content/50">
                Time Available
              </div>
              <div className="mt-1 font-semibold">
                {selectedAvailableSeconds != null
                  ? formatSeconds(selectedAvailableSeconds)
                  : "--"}
              </div>
              <div className="mt-1 truncate text-sm text-base-content/65">
                Against fastest section splits
              </div>
            </div>
          </div>

          <MapLibreRouteMap
            routePoints={analysis.route_points}
            overlays={sectionOverlays}
            movingMarkers={sectionMarkers}
            fitBoundsPoints={analysis.route_points}
            fitBoundsKey={`${analysis.segment_id}-${analysis.split_count}`}
            fitBoundsPadding={ANALYSIS_MAP_FIT_BOUNDS_PADDING}
            fitBoundsMaxZoom={ANALYSIS_MAP_FIT_BOUNDS_MAX_ZOOM}
            ariaLabel={`${analysis.segment_title} analysis sections map`}
            emptyMessage="No segment route available for section mapping."
            className="h-[28rem] w-full overflow-hidden rounded-box border border-base-300 bg-base-300"
            showRouteEndpoints
            showLayerPicker
            showZoomControls
          />

          <div className="h-72 w-full">
            <ResponsiveContainer width="100%" height="100%">
              <BarChart
                data={chartData}
                onMouseMove={(state) => {
                  setHoveredSectionIndex(chartSectionIndex(state));
                }}
                onMouseLeave={() => {
                  setHoveredSectionIndex(null);
                }}
                onClick={(state) => {
                  const sectionIndex = chartSectionIndex(state);
                  if (sectionIndex != null) {
                    setSelectedSectionIndex(sectionIndex);
                  }
                }}
              >
                <CartesianGrid strokeDasharray="4 4" />
                <XAxis dataKey="section" />
                <YAxis tickFormatter={(value) => `${value}s`} />
                <Tooltip
                  content={
                    <SegmentAnalysisChartTooltip
                      sections={analysis.sections}
                      selectedSectionEffortBySection={
                        selectedSectionEffortBySection
                      }
                    />
                  }
                />
                <Legend />
                <Bar
                  dataKey="selectedAvailable"
                  name="Time available"
                  fill="#2563eb"
                >
                  {chartData.map((row) => (
                    <Cell
                      key={`selected-${row.sectionIndex}`}
                      fill={
                        row.sectionIndex === activeSectionIndex
                          ? "#f97316"
                          : "#2563eb"
                      }
                      cursor="pointer"
                    />
                  ))}
                </Bar>
              </BarChart>
            </ResponsiveContainer>
          </div>

          <div className="overflow-x-auto">
            <table className="table table-zebra">
              <thead>
                <tr>
                  <th>Section</th>
                  <th>Selected split</th>
                  <th>Best</th>
                  <th>Available</th>
                  <th>Best ride</th>
                  <th>Speed</th>
                </tr>
              </thead>
              <tbody>
                {analysis.sections.map((section) => {
                  const selectedSectionEffort =
                    selectedSectionEffortBySection.get(section.section_index);

                  return (
                    <tr
                      key={section.section_index}
                      className={
                        section.section_index === activeSectionIndex
                          ? "cursor-pointer bg-primary/10"
                          : "cursor-pointer"
                      }
                      onMouseEnter={() => {
                        setHoveredSectionIndex(section.section_index);
                      }}
                      onMouseLeave={() => {
                        setHoveredSectionIndex(null);
                      }}
                      onClick={() => {
                        setSelectedSectionIndex(section.section_index);
                      }}
                    >
                      <td className="font-semibold">
                        S{section.section_index}
                        <span className="ml-2 text-xs font-normal text-base-content/55">
                          {sectionLabel(section)}
                        </span>
                      </td>
                      <td>
                        {selectedSectionEffort
                          ? formatSeconds(selectedSectionEffort.split_seconds)
                          : "--"}
                      </td>
                      <td>{formatSeconds(section.best_split_seconds)}</td>
                      <td className="text-success">
                        {selectedSectionEffort
                          ? formatSeconds(
                              selectedSectionEffort.delta_from_best_seconds,
                            )
                          : "--"}
                      </td>
                      <td>
                        <Link
                          href={`/activities/${section.best_activity_id}`}
                          className="link-hover link font-medium"
                        >
                          {section.best_activity_title}
                        </Link>
                      </td>
                      <td>
                        {formatSpeed(
                          selectedSectionEffort?.average_speed_mps ?? null,
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </>
      )}
    </AppCard>
  );
}
