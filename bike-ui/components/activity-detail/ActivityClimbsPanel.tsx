"use client";

import ResponsiveChart from "../ui/ResponsiveChart";

import { type KeyboardEvent } from "react";
import {
  Area,
  CartesianGrid,
  ComposedChart,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { type ActivityClimb } from "../../lib/activityClimbs";
import {
  FEET_PER_METER,
  formatDistance,
  formatDuration,
  formatElevation,
  METERS_PER_MILE,
  type UnitSystem,
} from "../../lib/activityFormatting";
import MetricCard from "../MetricCard";
import { useKeyedState } from "../../lib/useKeyedState";
import ViewRouteOnMap from "../ViewRouteOnMap";

const CLIMB_LIST_VISIBLE_ROW_COUNT = 20;
const CLIMB_LIST_MAX_HEIGHT = `calc(${(CLIMB_LIST_VISIBLE_ROW_COUNT + 1) * 2.5}rem + 2px)`;

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
  return `${distanceValue(startDistanceMeters, unitSystem)} - ${distanceValue(
    endDistanceMeters,
    unitSystem,
  )} ${distanceUnit(unitSystem)}`;
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

export default function ActivityClimbsPanel({
  activityId,
  climbs: activityClimbs,
  unitSystem,
  active,
}: {
  activityId: number | string;
  climbs: ActivityClimb[];
  unitSystem: UnitSystem;
  active: boolean;
}) {
  const [selectedClimbId, setSelectedClimbId] = useKeyedState<string | null>(
    String(activityId),
    null,
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
    setSelectedClimbId(climbId);
  }

  return (
    <section id="activity-climbs-card" aria-label="Climbs">
      <h2 className="sr-only">Climbs</h2>

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
                  <th>
                    <span className="sr-only">Map</span>
                  </th>
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
                      onClick={() => setSelectedClimbId(climb.id)}
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
                        {formatElevation(climb.elevationGainMeters, unitSystem)}
                      </td>
                      <td className="whitespace-nowrap">
                        {formatGradePercent(climb.avgGradePercent)}
                      </td>
                      <td className="text-right">
                        <ViewRouteOnMap
                          title={`Climb ${climb.sequence}`}
                          routePoints={climb.routePoints}
                        />
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>

          {selectedClimb ? (
            <section
              className="mt-6 border-t border-base-300 pt-6"
              aria-label={`Climb ${selectedClimb.sequence} details`}
            >
              <h3 className="text-lg font-semibold text-base-content">
                Climb {selectedClimb.sequence}
              </h3>
              <p className="mt-1 text-sm text-base-content/60">
                Activity distance:{" "}
                {formatDistanceRange(
                  selectedClimb.startDistanceMeters,
                  selectedClimb.endDistanceMeters,
                  unitSystem,
                )}
              </p>

              <div className="mt-4 grid grid-cols-2 gap-x-6 gap-y-4 lg:grid-cols-4">
                <MetricCard
                  variant="plain"
                  size="sm"
                  label="distance"
                  value={formatDistance(
                    selectedClimb.distanceMeters,
                    unitSystem,
                  )}
                />
                <MetricCard
                  variant="plain"
                  size="sm"
                  label="elevation gain"
                  value={formatElevation(
                    selectedClimb.elevationGainMeters,
                    unitSystem,
                  )}
                />
                <MetricCard
                  variant="plain"
                  size="sm"
                  label="elevation loss"
                  value={formatElevation(
                    selectedClimb.elevationLossMeters,
                    unitSystem,
                  )}
                />
                <MetricCard
                  variant="plain"
                  size="sm"
                  label="category"
                  value={formatClimbCategory(selectedClimb.category)}
                />
                <MetricCard
                  variant="plain"
                  size="sm"
                  label="max grade"
                  value={formatGradePercent(selectedClimb.maxGradePercent)}
                />
                <MetricCard
                  variant="plain"
                  size="sm"
                  label="avg grade"
                  value={formatGradePercent(selectedClimb.avgGradePercent)}
                />
                <MetricCard
                  variant="plain"
                  size="sm"
                  label="estimated"
                  value={formatDuration(selectedClimb.durationSeconds)}
                />
              </div>

              {active && elevationRows.length > 1 ? (
                <div
                  role="img"
                  aria-label={`Climb ${selectedClimb.sequence} elevation profile`}
                  className="mt-6 overflow-hidden rounded-box border border-base-300 bg-base-200 p-3"
                >
                  <div className="h-[180px] w-full">
                    <ResponsiveChart
                      width="100%"
                      height="100%"
                      minWidth={0}
                      minHeight={180}
                      initialDimension={{ width: 320, height: 180 }}
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
                          domain={["dataMin", "dataMax"]}
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
                    </ResponsiveChart>
                  </div>
                </div>
              ) : null}
            </section>
          ) : null}
        </>
      ) : (
        <div className="alert mt-5">
          <span>No sustained climbs found.</span>
        </div>
      )}
    </section>
  );
}
