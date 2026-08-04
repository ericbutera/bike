"use client";

import { faArrowLeft, faArrowRight } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useMemo, useState } from "react";
import toast from "react-hot-toast";
import {
  formatActivityTimestamp,
  formatDistance,
  formatDuration,
  type UnitSystem,
} from "../lib/activityFormatting";
import {
  useCreateSegmentFromActivity,
  useUpdateSegmentFromActivity,
  type Activity,
  type ActivityRoutePoint,
  type Segment,
} from "../lib/queries";
import {
  buildInitialSegmentSelection,
  clampEndIndex,
  clampStartIndex,
  hasSegmentBuilderRoute,
  segmentSelectionDistanceMeters,
  segmentSelectionDurationSeconds,
  shiftEndIndex,
  shiftStartIndex,
  sliceSegmentRoutePoints,
  type SegmentBuilderSelection,
} from "../lib/segmentBuilder";
import { useUnitPreferences } from "../lib/unitPreferences";
import MapLibreRouteMap from "./MapLibreRouteMap";
import { ErrorCard, LoadingCard } from "./ui/QueryState";

const EMPTY_ROUTE_POINTS: ActivityRoutePoint[] = [];

function BuilderStat({
  label,
  value,
  detail,
}: {
  label: string;
  value: string;
  detail?: string;
}) {
  return (
    <div className="rounded-box border border-base-300 bg-base-200 px-4 py-3">
      <div className="text-[0.7rem] font-semibold uppercase tracking-[0.16em] text-base-content/50">
        {label}
      </div>
      <div className="mt-1 text-lg font-semibold text-base-content">
        {value}
      </div>
      {detail ? (
        <div className="mt-1 text-sm text-base-content/65">{detail}</div>
      ) : null}
    </div>
  );
}

function SelectionControl({
  label,
  toneClassName,
  value,
  min,
  max,
  routePointCount,
  point,
  unitSystem,
  onChange,
  onShift,
}: {
  label: string;
  toneClassName: string;
  value: number;
  min: number;
  max: number;
  routePointCount: number;
  point: ActivityRoutePoint | null;
  unitSystem: UnitSystem;
  onChange: (value: number) => void;
  onShift: (delta: number) => void;
}) {
  return (
    <div className="rounded-box border border-base-300 bg-base-200 p-4">
      <div className="flex items-start justify-between gap-4">
        <div>
          <div className="flex items-center gap-2">
            <span className={`h-2.5 w-2.5 rounded-full ${toneClassName}`} />
            <span className="text-sm font-semibold text-base-content">
              {label}
            </span>
          </div>
          <div className="mt-2 text-sm text-base-content/70">
            {formatDuration(point?.elapsed_seconds ?? null)} into the ride
          </div>
          <div className="mt-1 text-sm text-base-content/55">
            {formatDistance(point?.distance_meters ?? null, unitSystem)} from
            the activity start
          </div>
        </div>

        <div className="join">
          <button
            type="button"
            className="join-item btn btn-sm btn-square btn-ghost"
            disabled={value <= min}
            aria-label={`Move ${label.toLowerCase()} backward one point`}
            onClick={() => {
              onShift(-1);
            }}
          >
            <FontAwesomeIcon icon={faArrowLeft} className="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            className="join-item btn btn-sm btn-square btn-ghost"
            disabled={value >= max}
            aria-label={`Move ${label.toLowerCase()} forward one point`}
            onClick={() => {
              onShift(1);
            }}
          >
            <FontAwesomeIcon icon={faArrowRight} className="h-3.5 w-3.5" />
          </button>
        </div>
      </div>

      <input
        type="range"
        min={min}
        max={max}
        step={1}
        value={Math.min(Math.max(value, min), max)}
        className="range range-primary mt-4"
        aria-label={label}
        onChange={(event) => {
          onChange(Number(event.target.value));
        }}
      />

      <div className="mt-2 flex items-center justify-between text-xs text-base-content/55">
        <span>Point {value + 1}</span>
        <span>{routePointCount} total points</span>
      </div>
    </div>
  );
}

type SegmentBuilderWorkspaceProps = {
  segment: Segment | null;
  activity: Activity | null;
  isLoading: boolean;
  unavailableMessage?: string | null;
};

export default function SegmentBuilderWorkspace({
  segment,
  activity,
  isLoading,
  unavailableMessage = null,
}: SegmentBuilderWorkspaceProps) {
  const router = useRouter();
  const { unitSystem } = useUnitPreferences();
  const createSegmentMutation = useCreateSegmentFromActivity();
  const updateSegmentFromActivityMutation = useUpdateSegmentFromActivity();
  const routePoints = activity?.route_points ?? EMPTY_ROUTE_POINTS;
  const hasRoute = hasSegmentBuilderRoute(routePoints);
  const builderSource = segment?.builder_source ?? null;
  const isEditingExistingSegment = builderSource != null;
  const [selection, setSelection] = useState<SegmentBuilderSelection>(() =>
    buildInitialSegmentSelection(routePoints),
  );
  const [segmentName, setSegmentName] = useState("");

  useEffect(() => {
    if (!activity) {
      setSelection(buildInitialSegmentSelection(undefined));
      setSegmentName(segment?.title ?? "");
      return;
    }

    if (
      builderSource &&
      builderSource.activity_id === activity.id &&
      hasSegmentBuilderRoute(routePoints)
    ) {
      const nextStartIndex = clampStartIndex(
        routePoints,
        builderSource.start_route_point_index,
        builderSource.end_route_point_index,
      );
      const nextEndIndex = clampEndIndex(
        routePoints,
        nextStartIndex,
        builderSource.end_route_point_index,
      );

      setSelection({
        startIndex: nextStartIndex,
        endIndex: nextEndIndex,
      });
      setSegmentName(segment?.title ?? "");
      return;
    }

    setSelection(buildInitialSegmentSelection(activity.route_points));
    setSegmentName(segment?.title ?? "");
  }, [activity, builderSource, routePoints, segment?.title]);

  const startIndex = clampStartIndex(
    routePoints,
    selection.startIndex,
    selection.endIndex,
  );
  const endIndex = clampEndIndex(routePoints, startIndex, selection.endIndex);
  const normalizedSelection = useMemo(
    () => ({ startIndex, endIndex }),
    [endIndex, startIndex],
  );
  const selectedRoutePoints = useMemo(
    () => sliceSegmentRoutePoints(routePoints, normalizedSelection),
    [normalizedSelection, routePoints],
  );
  const startPoint = routePoints[startIndex] ?? null;
  const endPoint = routePoints[endIndex] ?? null;
  const selectedDistance = segmentSelectionDistanceMeters(
    routePoints,
    normalizedSelection,
  );
  const selectedDuration = segmentSelectionDurationSeconds(
    routePoints,
    normalizedSelection,
  );
  const overlayPoints =
    selectedRoutePoints.length >= 2 ? selectedRoutePoints : [];
  const isSaving =
    createSegmentMutation.isPending ||
    updateSegmentFromActivityMutation.isPending;

  const mapMarkers = [
    {
      id: "segment-start",
      point: startPoint,
      color: "#16a34a",
      label: "S",
      opacity: 1,
    },
    {
      id: "segment-end",
      point: endPoint,
      color: "#dc2626",
      label: "E",
      opacity: 1,
    },
  ];

  async function handleSave() {
    if (!activity) {
      return;
    }

    const title = segmentName.trim();

    if (!title) {
      toast.error("Give the segment a name before saving.");
      return;
    }

    if (!hasRoute) {
      toast.error(
        "This activity does not have enough route data to build a segment.",
      );
      return;
    }

    try {
      const savedSegment =
        isEditingExistingSegment && segment
          ? await updateSegmentFromActivityMutation.updateAsync({
              id: segment.id,
              activityId: activity.id,
              title,
              startRoutePointIndex: normalizedSelection.startIndex,
              endRoutePointIndex: normalizedSelection.endIndex,
            })
          : await createSegmentMutation.createAsync({
              activityId: activity.id,
              title,
              startRoutePointIndex: normalizedSelection.startIndex,
              endRoutePointIndex: normalizedSelection.endIndex,
            });

      toast.success(
        savedSegment.processing_task_id
          ? `Saved ${savedSegment.title}. Segment matching queued as task ${savedSegment.processing_task_id}.`
          : `Saved ${savedSegment.title}. Segment matching queued.`,
      );
      router.push(`/segments/${savedSegment.id}`);
    } catch {
      // Mutation errors are surfaced by the app-level React Query handler.
    }
  }

  if (isLoading) {
    return (
      <LoadingCard
        size="lg"
        className="border border-base-300"
        bodyClassName="items-center justify-center py-16"
      />
    );
  }

  if (unavailableMessage) {
    return (
      <ErrorCard
        error={null}
        fallback={unavailableMessage}
        className="border border-base-300"
      />
    );
  }

  if (!activity) {
    return (
      <section className="card border border-dashed border-base-300 bg-base-100 shadow-xl">
        <div className="card-body py-16 text-center">
          <h2 className="text-2xl font-semibold text-base-content">
            Open the builder from a ride
          </h2>
        </div>
      </section>
    );
  }

  return (
    <section className="card border border-base-300 bg-base-100 shadow-xl">
      <div className="card-body gap-6">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <div className="flex flex-wrap items-center gap-2">
              <p className="text-sm text-base-content/60">Segment workspace</p>
              {isEditingExistingSegment ? (
                <span className="badge badge-outline">
                  Editing saved segment
                </span>
              ) : null}
            </div>
            <h2 className="text-2xl font-semibold text-base-content">
              {activity.title}
            </h2>
            <p className="mt-2 text-sm text-base-content/70">
              {formatActivityTimestamp(activity.started_at)}
              {activity.location ? ` · ${activity.location}` : ""}
            </p>
          </div>

          <Link
            href={`/activities/${activity.id}`}
            className="btn btn-ghost btn-sm"
          >
            Open full ride detail
          </Link>
        </div>

        {!hasRoute ? (
          <div className="alert alert-warning">
            <span>
              This activity does not have enough route geometry yet. Regenerate
              or re-import it before building a segment from it.
            </span>
          </div>
        ) : (
          <>
            <div className="rounded-box border border-base-300 bg-base-200 p-4">
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div>
                  <h3 className="text-lg font-semibold text-base-content">
                    Route crop
                  </h3>
                  <p className="mt-1 text-sm text-base-content/70">
                    Drag the start and end sliders to crop the route, then use
                    the arrow controls for point-level adjustments.
                  </p>
                </div>
                <span className="badge badge-outline whitespace-nowrap">
                  {overlayPoints.length} selected points
                </span>
              </div>

              <div className="mt-4 grid gap-4 lg:grid-cols-2">
                <SelectionControl
                  label="Start point"
                  toneClassName="bg-success"
                  value={startIndex}
                  min={0}
                  max={Math.max(0, routePoints.length - 2)}
                  routePointCount={routePoints.length}
                  point={startPoint}
                  unitSystem={unitSystem}
                  onChange={(nextValue) => {
                    setSelection((current) => ({
                      startIndex: clampStartIndex(
                        routePoints,
                        nextValue,
                        current.endIndex,
                      ),
                      endIndex: current.endIndex,
                    }));
                  }}
                  onShift={(delta) => {
                    setSelection((current) =>
                      shiftStartIndex(routePoints, current, delta),
                    );
                  }}
                />

                <SelectionControl
                  label="End point"
                  toneClassName="bg-error"
                  value={endIndex}
                  min={Math.min(routePoints.length - 1, startIndex + 1)}
                  max={Math.max(1, routePoints.length - 1)}
                  routePointCount={routePoints.length}
                  point={endPoint}
                  unitSystem={unitSystem}
                  onChange={(nextValue) => {
                    setSelection((current) => ({
                      startIndex: current.startIndex,
                      endIndex: clampEndIndex(
                        routePoints,
                        current.startIndex,
                        nextValue,
                      ),
                    }));
                  }}
                  onShift={(delta) => {
                    setSelection((current) =>
                      shiftEndIndex(routePoints, current, delta),
                    );
                  }}
                />
              </div>
            </div>

            <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
              <BuilderStat
                label="Selected distance"
                value={formatDistance(selectedDistance, unitSystem)}
              />
              <BuilderStat
                label="Selected duration"
                value={formatDuration(selectedDuration)}
              />
              <BuilderStat
                label="Start position"
                value={formatDuration(startPoint?.elapsed_seconds ?? null)}
                detail={formatDistance(
                  startPoint?.distance_meters ?? null,
                  unitSystem,
                )}
              />
              <BuilderStat
                label="End position"
                value={formatDuration(endPoint?.elapsed_seconds ?? null)}
                detail={formatDistance(
                  endPoint?.distance_meters ?? null,
                  unitSystem,
                )}
              />
            </div>

            <div className="overflow-hidden rounded-box border border-base-300 bg-base-200">
              <MapLibreRouteMap
                routePoints={routePoints}
                overlays={
                  overlayPoints.length >= 2
                    ? [
                        {
                          id: "segment-selection",
                          points: overlayPoints,
                          color: "#f97316",
                          weight: 7,
                        },
                      ]
                    : undefined
                }
                movingMarkers={mapMarkers}
                ariaLabel="Segment builder map"
                emptyMessage="This ride does not have route geometry yet."
                className="h-[28rem] w-full rounded-none border-0"
                showZoomControls
                showLayerPicker
                fitBoundsPadding={48}
                fitBoundsMaxZoom={16}
                showRouteEndpoints={false}
              />
            </div>

            <div className="rounded-box border border-base-300 bg-base-200 p-4">
              <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-end">
                <label className="form-control w-full">
                  <div className="label px-0 pb-2">
                    <span className="label-text font-semibold text-base-content">
                      Segment name
                    </span>
                  </div>
                  <input
                    type="text"
                    value={segmentName}
                    className="input input-bordered w-full bg-base-100"
                    placeholder="Name this segment"
                    onChange={(event) => {
                      setSegmentName(event.target.value);
                    }}
                  />
                </label>

                <button
                  type="button"
                  className="btn btn-primary lg:min-w-[12rem]"
                  disabled={isSaving || segmentName.trim().length === 0}
                  onClick={() => {
                    void handleSave();
                  }}
                >
                  {isSaving
                    ? "Saving..."
                    : isEditingExistingSegment
                      ? "Save changes"
                      : "Save segment"}
                </button>
              </div>
            </div>
          </>
        )}
      </div>
    </section>
  );
}
