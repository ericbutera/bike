"use client";

import { useMemo, type KeyboardEvent } from "react";
import {
  Area,
  CartesianGrid,
  ComposedChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { buildActivityClimbs, type ActivityClimb } from "../../lib/activityClimbs";
import {
  FEET_PER_METER,
  formatDistance,
  formatDuration,
  formatElevation,
  METERS_PER_MILE,
  type UnitSystem,
} from "../../lib/activityFormatting";
import { useActivity } from "../../lib/queries";
import MetricCard from "../MetricCard";

const CLIMB_LIST_VISIBLE_ROW_COUNT = 15;
const CLIMB_LIST_MAX_HEIGHT = "40rem";

function formatGradePercent(value: number | null | undefined) {
  if (value == null || !Number.isFinite(value)) {
    return "--";
  }

  return `${value.toFixed(1)}%`;
}

function distanceUnit(unitSystem: UnitSystem) {
  return unitSystem === "imperial" ? "mi" : "km";
}

function distanceValue(valueMeters: number, unitSystem: UnitSystem) {
  const value =
    unitSystem === "imperial"
      ? valueMeters / METERS_PER_MILE
      : valueMeters / 1000;
  const formatted = value >= 100 ? value.toFixed(0) : value.toFixed(1);

  return formatted.replace(/\.0$/, "");
}

function elevationChartValue(valueMeters: number, unitSystem: UnitSystem) {
  return unitSystem === "imperial" ? valueMeters * FEET_PER_METER : valueMeters;
}

function formatDistanceRange(
  startDistanceMeters: number,
  endDistanceMeters: number,
  unitSystem: UnitSystem,
) {
  return `${distanceValue(startDistanceMeters, unitSystem)} - ${distanceValue(endDistanceMeters, unitSystem)} ${distanceUnit(unitSystem)}`;
}

function formatClimbCategory(category: ActivityClimb["category"]) {
  if (category == null) {
    return "--";
  }

  return category.toString();
}

function buildClimbElevationRows(climb: ActivityClimb, unitSystem: UnitSystem) {
  return climb.routePoints.flatMap((point) => {
    if (
      point.distance_meters == null ||
      point.elevation_meters == null ||
      !Number.isFinite(point.distance_meters) ||
      !Number.isFinite(point.elevation_meters)
    ) {
      return [];
    }

    return [
      {
        distance:
          unitSystem === "imperial"
            ? (point.distance_meters - climb.startDistanceMeters) /
              METERS_PER_MILE
            : (point.distance_meters - climb.startDistanceMeters) / 1000,
        elevation: elevationChartValue(point.elevation_meters, unitSystem),
      },
    ];
  });
}

function ClimbElevationTooltip({
  active,
  payload,
  label,
  unitSystem,
}: {
  active?: boolean;
  payload?: Array<{
    value?: number;
  }>;
  label?: number;
  unitSystem: UnitSystem;
}) {
  if (!active || !payload?.length) {
    return null;
  }

  const elevation = payload[0]?.value;

  return (
    <div className="rounded-box border border-base-300 bg-base-100 px-3 py-2 text-xs shadow-lg">
      <div className="font-semibold text-base-content">
        {typeof label === "number"
          ? `${label.toFixed(1)} ${distanceUnit(unitSystem)}`
          : "--"}
      </div>
      <div className="mt-1 text-base-content/70">
        {unitSystem === "imperial"
          ? `${Math.round(elevation ?? 0)} ft`
          : `${Math.round(elevation ?? 0)} m`}
      </div>
    </div>
  );
}

export default function ActivityClimbsCard({
  activityId,
  selectedClimbId,
  unitSystem,
  onSelectClimb,
  onZoomOutMap,
}: {
  activityId: number | string;
  selectedClimbId: string | null;
  unitSystem: UnitSystem;
  onSelectClimb: (climbId: string) => void;
  onZoomOutMap: () => void;
}) {
  const activityQuery = useActivity(activityId);
  const activityClimbs = useMemo(
    () => buildActivityClimbs(activityQuery.data?.route_points),
    [activityQuery.data?.route_points],
  );
  const selectedClimb =
    activityClimbs.find((climb) => climb.id === selectedClimbId) ?? null;
  const elevationRows = selectedClimb
    ? buildClimbElevationRows(selectedClimb, unitSystem)
    : [];
  const shouldLimitClimbList =
    activityClimbs.length > CLIMB_LIST_VISIBLE_ROW_COUNT;

  function handleRowKeyDown(
    event: KeyboardEvent<HTMLTableRowElement>,
    climbId: string,
  ) {
    if (event.key !== "Enter" && event.key !== " ") {
      return;
    }

    event.preventDefault();
    onSelectClimb(climbId);
  }

  return (
    <div id="activity-climbs-card" className="card bg-base-100 shadow-xl">
      <div className="card-body">
        <div className="mb-3 flex items-center justify-between gap-3 text-xs font-medium uppercase tracking-[0.24em] text-base-content/50">
          <h2>Climbs</h2>
          <span className="badge badge-outline">
            {activityClimbs.length} climb
            {activityClimbs.length === 1 ? "" : "s"}
          </span>
        </div>

        {activityClimbs.length > 0 ? (
          <>
            <div
              data-testid="activity-climbs-table-scroll"
              className={`overflow-x-auto rounded-box border border-base-300 ${
                shouldLimitClimbList ? "overflow-y-auto" : ""
              }`}
              style={
                shouldLimitClimbList
                  ? { maxHeight: CLIMB_LIST_MAX_HEIGHT }
                  : undefined
              }
            >
              <table className="table table-sm">
                <thead
                  className={
                    shouldLimitClimbList ? "sticky top-0 z-10 bg-base-100" : ""
                  }
                >
                  <tr className="h-10">
                    <th>Climb</th>
                    <th>Distance</th>
                    <th>Elevation</th>
                    <th>Grade</th>
                  </tr>
                </thead>
                <tbody>
                  {activityClimbs.map((climb) => {
                    const isSelected = climb.id === selectedClimbId;

                    return (
                      <tr
                        key={climb.id}
                        role="button"
                        tabIndex={0}
                        aria-label={`Show climb ${climb.sequence} details`}
                        aria-pressed={isSelected}
                        className={`h-10 cursor-pointer transition hover:bg-base-200 focus:bg-base-200 focus:outline-none ${
                          isSelected ? "bg-primary/10" : ""
                        }`}
                        onClick={() => onSelectClimb(climb.id)}
                        onKeyDown={(event) => handleRowKeyDown(event, climb.id)}
                      >
                        <th
                          scope="row"
                          className="whitespace-nowrap font-semibold"
                        >
                          Climb {climb.sequence}
                        </th>
                        <td className="whitespace-nowrap">
                          {formatDistance(climb.distanceMeters, unitSystem)}
                        </td>
                        <td className="whitespace-nowrap">
                          {formatElevation(
                            climb.elevationGainMeters,
                            unitSystem,
                          )}
                        </td>
                        <td className="whitespace-nowrap">
                          {formatGradePercent(climb.avgGradePercent)}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>

            {selectedClimb ? (
              <div className="mt-6 border-t border-base-300 pt-6">
                <div className="flex flex-wrap items-start justify-between gap-3">
                  <h3 className="text-lg font-semibold text-base-content">
                    Climb:{" "}
                    {formatDistanceRange(
                      selectedClimb.startDistanceMeters,
                      selectedClimb.endDistanceMeters,
                      unitSystem,
                    )}
                  </h3>
                  <button
                    type="button"
                    className="btn btn-outline btn-sm"
                    onClick={onZoomOutMap}
                  >
                    Zoom out map
                  </button>
                </div>

                <div className="mt-5 grid gap-x-6 gap-y-5 sm:grid-cols-2 lg:grid-cols-4">
                  <MetricCard
                    variant="plain"
                    label="distance"
                    value={formatDistance(
                      selectedClimb.distanceMeters,
                      unitSystem,
                    )}
                  />
                  <MetricCard
                    variant="plain"
                    label="elevation gain"
                    value={formatElevation(
                      selectedClimb.elevationGainMeters,
                      unitSystem,
                    )}
                  />
                  <MetricCard
                    variant="plain"
                    label="elevation loss"
                    value={formatElevation(
                      selectedClimb.elevationLossMeters,
                      unitSystem,
                    )}
                  />
                  <MetricCard
                    variant="plain"
                    label="category"
                    value={formatClimbCategory(selectedClimb.category)}
                  />
                  <MetricCard
                    variant="plain"
                    label="max grade"
                    value={formatGradePercent(selectedClimb.maxGradePercent)}
                  />
                  <MetricCard
                    variant="plain"
                    label="avg grade"
                    value={formatGradePercent(selectedClimb.avgGradePercent)}
                  />
                  <MetricCard
                    variant="plain"
                    label="estimated"
                    value={formatDuration(selectedClimb.durationSeconds)}
                  />
                </div>

                {elevationRows.length > 1 ? (
                  <div
                    role="img"
                    aria-label={`Climb ${selectedClimb.sequence} elevation profile`}
                    className="mt-6 overflow-hidden rounded-box border border-base-300 bg-base-200 p-3"
                  >
                    <div className="h-[180px] w-full">
                      <ResponsiveContainer
                        width="100%"
                        height="100%"
                        minWidth={320}
                        minHeight={180}
                      >
                        <ComposedChart
                          data={elevationRows}
                          margin={{ top: 8, right: 8, bottom: 8, left: 0 }}
                        >
                          <CartesianGrid
                            vertical={false}
                            stroke="var(--color-base-content)"
                            strokeOpacity={0.1}
                          />
                          <XAxis
                            axisLine={false}
                            dataKey="distance"
                            tick={{
                              fill: "var(--color-base-content)",
                              fontSize: 10,
                            }}
                            tickFormatter={(value: number) => value.toFixed(1)}
                            tickLine={false}
                            type="number"
                          />
                          <YAxis hide domain={["dataMin", "dataMax"]} />
                          <Tooltip
                            content={
                              <ClimbElevationTooltip unitSystem={unitSystem} />
                            }
                          />
                          <Area
                            type="linear"
                            dataKey="elevation"
                            stroke="var(--color-warning)"
                            fill="var(--color-warning)"
                            fillOpacity={0.16}
                            strokeWidth={2}
                            dot={false}
                            connectNulls
                          />
                        </ComposedChart>
                      </ResponsiveContainer>
                    </div>
                  </div>
                ) : null}
              </div>
            ) : null}
          </>
        ) : (
          <div className="alert mt-5">
            <span>No sustained climbs found.</span>
          </div>
        )}
      </div>
    </div>
  );
}
